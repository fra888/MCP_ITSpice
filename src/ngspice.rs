use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_short, c_void};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use libloading::{Library, Symbol};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NgComplex {
    pub cx_real: f64,
    pub cx_imag: f64,
}

#[repr(C)]
#[derive(Debug)]
pub struct VectorInfo {
    pub v_name: *mut c_char,
    pub v_type: c_int,
    pub v_flags: c_short,
    pub v_realdata: *mut f64,
    pub v_compdata: *mut NgComplex,
    pub v_length: c_int,
}

pub type SendCharFn = extern "C" fn(*mut c_char, c_int, *mut c_void) -> c_int;
pub type SendStatFn = extern "C" fn(*mut c_char, c_int, *mut c_void) -> c_int;
pub type ControlledExitFn = extern "C" fn(c_int, bool, bool, c_int, *mut c_void) -> c_int;

type NgSpiceInitFn = unsafe extern "C" fn(
    Option<SendCharFn>,
    Option<SendStatFn>,
    Option<ControlledExitFn>,
    *const c_void,
    *const c_void,
    *const c_void,
    *mut c_void,
) -> c_int;
type NgSpiceCircFn = unsafe extern "C" fn(*mut *mut c_char) -> c_int;
type NgSpiceCommandFn = unsafe extern "C" fn(*mut c_char) -> c_int;
type NgSpiceCurPlotFn = unsafe extern "C" fn() -> *mut c_char;
type NgSpiceAllVecsFn = unsafe extern "C" fn(*mut c_char) -> *mut *mut c_char;
type NgGetVecInfoFn = unsafe extern "C" fn(*mut c_char) -> *mut VectorInfo;
type NgSpiceRunningFn = unsafe extern "C" fn() -> bool;

#[derive(Debug, Clone)]
pub struct TraceData {
    pub name: String,
    pub real: Vec<f64>,
    pub imag: Option<Vec<f64>>,
}

#[derive(Debug, Clone)]
pub struct SimulationResult {
    pub plot_name: String,
    pub x_name: String,
    pub x_values: Vec<f64>,
    pub traces: HashMap<String, TraceData>,
    pub log: Vec<String>,
    pub success: bool,
    pub error: Option<String>,
}

struct SharedState {
    logs: Vec<String>,
    status: String,
    exited: bool,
}

extern "C" fn cb_send_char(msg: *mut c_char, _id: c_int, user_data: *mut c_void) -> c_int {
    if !msg.is_null() && !user_data.is_null() {
        let s = unsafe { CStr::from_ptr(msg) }.to_string_lossy().into_owned();
        let state = unsafe { &*(user_data as *const Mutex<SharedState>) };
        if let Ok(mut guard) = state.lock() {
            for line in s.lines() {
                let trimmed = line.trim_end();
                if !trimmed.is_empty() {
                    guard.logs.push(trimmed.to_string());
                }
            }
        }
    }
    0
}

extern "C" fn cb_send_stat(msg: *mut c_char, _id: c_int, user_data: *mut c_void) -> c_int {
    if !msg.is_null() && !user_data.is_null() {
        let s = unsafe { CStr::from_ptr(msg) }.to_string_lossy().into_owned();
        let state = unsafe { &*(user_data as *const Mutex<SharedState>) };
        if let Ok(mut guard) = state.lock() {
            guard.status = s;
        }
    }
    0
}

extern "C" fn cb_controlled_exit(
    status: c_int,
    _immediate: bool,
    _quit: bool,
    _id: c_int,
    user_data: *mut c_void,
) -> c_int {
    if !user_data.is_null() {
        let state = unsafe { &*(user_data as *const Mutex<SharedState>) };
        if let Ok(mut guard) = state.lock() {
            guard.exited = true;
            guard
                .logs
                .push(format!("ngspice requested exit (status {status})"));
        }
    }
    0
}

pub struct NgSpiceEngine {
    _lib: Library,
    state: Arc<Mutex<SharedState>>,
    fn_circ: NgSpiceCircFn,
    fn_command: NgSpiceCommandFn,
    fn_cur_plot: NgSpiceCurPlotFn,
    fn_all_vecs: NgSpiceAllVecsFn,
    fn_get_vec_info: NgGetVecInfoFn,
    fn_running: NgSpiceRunningFn,
}

unsafe impl Send for NgSpiceEngine {}

static GLOBAL_ENGINE: OnceLock<Result<Mutex<NgSpiceEngine>, String>> = OnceLock::new();

impl NgSpiceEngine {
    pub fn global() -> Result<&'static Mutex<Self>, String> {
        let res = GLOBAL_ENGINE.get_or_init(|| Self::load().map(Mutex::new));
        match res {
            Ok(m) => Ok(m),
            Err(e) => Err(e.clone()),
        }
    }

    pub fn load() -> Result<Self, String> {
        let candidates = [
            "libngspice.so.0",
            "libngspice.so",
            "/lib/x86_64-linux-gnu/libngspice.so.0",
            "/usr/lib/x86_64-linux-gnu/libngspice.so.0",
            "/usr/local/lib/libngspice.so.0",
        ];

        let mut last_err = String::new();
        let mut loaded_lib = None;
        for path in candidates {
            match unsafe { Library::new(path) } {
                Ok(lib) => {
                    loaded_lib = Some(lib);
                    break;
                }
                Err(e) => {
                    last_err = format!("{path}: {e}");
                }
            }
        }

        let lib = loaded_lib.ok_or_else(|| {
            format!("Could not load libngspice shared library ({last_err})")
        })?;

        unsafe {
            let fn_init: Symbol<NgSpiceInitFn> = lib
                .get(b"ngSpice_Init\0")
                .map_err(|e| format!("Missing ngSpice_Init: {e}"))?;
            let fn_circ: NgSpiceCircFn = *lib
                .get::<NgSpiceCircFn>(b"ngSpice_Circ\0")
                .map_err(|e| format!("Missing ngSpice_Circ: {e}"))?;
            let fn_command: NgSpiceCommandFn = *lib
                .get::<NgSpiceCommandFn>(b"ngSpice_Command\0")
                .map_err(|e| format!("Missing ngSpice_Command: {e}"))?;
            let fn_cur_plot: NgSpiceCurPlotFn = *lib
                .get::<NgSpiceCurPlotFn>(b"ngSpice_CurPlot\0")
                .map_err(|e| format!("Missing ngSpice_CurPlot: {e}"))?;
            let fn_all_vecs: NgSpiceAllVecsFn = *lib
                .get::<NgSpiceAllVecsFn>(b"ngSpice_AllVecs\0")
                .map_err(|e| format!("Missing ngSpice_AllVecs: {e}"))?;
            let fn_get_vec_info: NgGetVecInfoFn = *lib
                .get::<NgGetVecInfoFn>(b"ngGet_Vec_Info\0")
                .map_err(|e| format!("Missing ngGet_Vec_Info: {e}"))?;
            let fn_running: NgSpiceRunningFn = *lib
                .get::<NgSpiceRunningFn>(b"ngSpice_running\0")
                .map_err(|e| format!("Missing ngSpice_running: {e}"))?;

            let state = Arc::new(Mutex::new(SharedState {
                logs: Vec::new(),
                status: String::new(),
                exited: false,
            }));

            let user_ptr = Arc::as_ptr(&state) as *mut c_void;
            let rc = fn_init(
                Some(cb_send_char),
                Some(cb_send_stat),
                Some(cb_controlled_exit),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                user_ptr,
            );
            if rc != 0 {
                return Err(format!("ngSpice_Init failed with code {rc}"));
            }

            Ok(Self {
                _lib: lib,
                state,
                fn_circ,
                fn_command,
                fn_cur_plot,
                fn_all_vecs,
                fn_get_vec_info,
                fn_running,
            })
        }
    }

    pub fn command(&self, cmd: &str) -> Result<i32, String> {
        let cstr = CString::new(cmd).map_err(|e| format!("Invalid command string: {e}"))?;
        let rc = unsafe { (self.fn_command)(cstr.as_ptr() as *mut c_char) };
        Ok(rc)
    }

    pub fn is_running(&self) -> bool {
        unsafe { (self.fn_running)() }
    }

    pub fn halt(&self) {
        let _ = self.command("bg_halt");
    }

    pub fn run_netlist(&self, netlist_lines: &[String]) -> SimulationResult {
        self.run_netlist_with_paths(netlist_lines, &[])
    }

    pub fn run_netlist_with_paths(
        &self,
        netlist_lines: &[String],
        search_paths: &[PathBuf],
    ) -> SimulationResult {
        if let Ok(mut guard) = self.state.lock() {
            guard.logs.clear();
            guard.status.clear();
            guard.exited = false;
        }

        // Clear previous simulation state in ngspice.
        let _ = self.command("destroy all");
        let _ = self.command("remcirc");

        // Configure ngspice sourcepath with default + user model directories
        if !search_paths.is_empty() {
            let mut sp = String::from("set sourcepath = ( .");
            for p in search_paths {
                let s = p.display().to_string();
                if !s.trim().is_empty() {
                    sp.push_str(&format!(" \"{}\"", s.replace('"', "")));
                }
            }
            sp.push_str(" )");
            let _ = self.command(&sp);
        }

        let mut cstrings: Vec<CString> = Vec::with_capacity(netlist_lines.len());
        for line in netlist_lines {
            match CString::new(line.as_str()) {
                Ok(cs) => cstrings.push(cs),
                Err(e) => {
                    return SimulationResult {
                        plot_name: String::new(),
                        x_name: String::new(),
                        x_values: Vec::new(),
                        traces: HashMap::new(),
                        log: vec![format!("Invalid netlist line '{line}': {e}")],
                        success: false,
                        error: Some(format!("Invalid netlist line: {e}")),
                    };
                }
            }
        }

        let mut ptrs: Vec<*mut c_char> = cstrings
            .iter()
            .map(|cs| cs.as_ptr() as *mut c_char)
            .collect();
        ptrs.push(std::ptr::null_mut());

        let rc_circ = unsafe { (self.fn_circ)(ptrs.as_mut_ptr()) };
        if rc_circ != 0 {
            let logs = self.take_logs();
            return SimulationResult {
                plot_name: String::new(),
                x_name: String::new(),
                x_values: Vec::new(),
                traces: HashMap::new(),
                log: logs.clone(),
                success: false,
                error: Some(format!(
                    "ngSpice_Circ failed (code {rc_circ}). {}",
                    logs.last().cloned().unwrap_or_default()
                )),
            };
        }

        let rc_run = match self.command("run") {
            Ok(rc) => rc,
            Err(e) => {
                return SimulationResult {
                    plot_name: String::new(),
                    x_name: String::new(),
                    x_values: Vec::new(),
                    traces: HashMap::new(),
                    log: self.take_logs(),
                    success: false,
                    error: Some(e),
                };
            }
        };

        let logs = self.take_logs();
        let exited = self
            .state
            .lock()
            .map(|g| g.exited)
            .unwrap_or(false);

        if rc_run != 0 || exited {
            return SimulationResult {
                plot_name: String::new(),
                x_name: String::new(),
                x_values: Vec::new(),
                traces: HashMap::new(),
                log: logs.clone(),
                success: false,
                error: Some(
                    logs.iter()
                        .rev()
                        .find(|l| {
                            let low = l.to_lowercase();
                            low.contains("error") || low.contains("fatal") || low.contains("failed")
                        })
                        .cloned()
                        .unwrap_or_else(|| format!("Simulation failed (code {rc_run})")),
                ),
            };
        }

        self.collect_current_plot(logs)
    }

    fn take_logs(&self) -> Vec<String> {
        self.state
            .lock()
            .map(|g| g.logs.clone())
            .unwrap_or_default()
    }

    fn collect_current_plot(&self, logs: Vec<String>) -> SimulationResult {
        let cur_plot_ptr = unsafe { (self.fn_cur_plot)() };
        if cur_plot_ptr.is_null() {
            return SimulationResult {
                plot_name: String::new(),
                x_name: String::new(),
                x_values: Vec::new(),
                traces: HashMap::new(),
                log: logs,
                success: false,
                error: Some("No simulation plot produced".to_string()),
            };
        }

        let plot_name = unsafe { CStr::from_ptr(cur_plot_ptr) }
            .to_string_lossy()
            .into_owned();

        if plot_name == "const" {
            return SimulationResult {
                plot_name,
                x_name: String::new(),
                x_values: Vec::new(),
                traces: HashMap::new(),
                log: logs,
                success: false,
                error: Some("Simulation produced no active plot (check analysis directive)".to_string()),
            };
        }

        let vecs_ptr = unsafe { (self.fn_all_vecs)(cur_plot_ptr) };
        if vecs_ptr.is_null() {
            return SimulationResult {
                plot_name,
                x_name: String::new(),
                x_values: Vec::new(),
                traces: HashMap::new(),
                log: logs,
                success: false,
                error: Some("Failed to enumerate plot vectors".to_string()),
            };
        }

        let mut vec_names = Vec::new();
        let mut idx = 0isize;
        loop {
            let p = unsafe { *vecs_ptr.offset(idx) };
            if p.is_null() {
                break;
            }
            let name = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
            vec_names.push(name);
            idx += 1;
        }

        let mut traces = HashMap::new();
        let mut x_name = String::new();
        let mut x_values = Vec::new();

        for name in vec_names {
            let full_name = format!("{plot_name}.{name}");
            let Ok(cname) = CString::new(full_name.as_str()) else {
                continue;
            };
            let info_ptr = unsafe { (self.fn_get_vec_info)(cname.as_ptr() as *mut c_char) };
            if info_ptr.is_null() {
                continue;
            }
            let info = unsafe { &*info_ptr };
            let len = info.v_length.max(0) as usize;
            if len == 0 {
                continue;
            }

            let (real, imag) = if !info.v_realdata.is_null() {
                let slice = unsafe { std::slice::from_raw_parts(info.v_realdata, len) };
                (slice.to_vec(), None)
            } else if !info.v_compdata.is_null() {
                let slice = unsafe { std::slice::from_raw_parts(info.v_compdata, len) };
                let r: Vec<f64> = slice.iter().map(|c| c.cx_real).collect();
                let im: Vec<f64> = slice.iter().map(|c| c.cx_imag).collect();
                (r, Some(im))
            } else {
                continue;
            };

            let lower = name.to_lowercase();
            let is_scale = lower == "time"
                || lower == "frequency"
                || lower == "v-sweep"
                || lower == "i-sweep"
                || lower == "temp-sweep";

            if is_scale && x_values.is_empty() {
                x_name = name.clone();
                x_values = real.clone();
            }

            let formatted_name = format_trace_name(&name);
            traces.insert(
                formatted_name.clone(),
                TraceData {
                    name: formatted_name,
                    real,
                    imag,
                },
            );
        }

        if x_values.is_empty() {
            if let Some(first) = traces.values().next() {
                x_name = "index".to_string();
                x_values = (0..first.real.len()).map(|i| i as f64).collect();
            }
        }

        if x_values.is_empty() {
            let err = logs
                .iter()
                .rev()
                .find(|l| {
                    let low = l.to_lowercase();
                    low.contains("error") || low.contains("aborted") || low.contains("too small")
                })
                .cloned()
                .unwrap_or_else(|| "Simulation produced 0 data points".to_string());
            return SimulationResult {
                plot_name,
                x_name,
                x_values,
                traces,
                log: logs,
                success: false,
                error: Some(err),
            };
        }

        SimulationResult {
            plot_name,
            x_name,
            x_values,
            traces,
            log: logs,
            success: true,
            error: None,
        }
    }
}

pub fn run_simulation_with_settings(
    netlist_lines: &[String],
    search_paths: &[PathBuf],
    ngspice_path: &str,
) -> SimulationResult {
    let trimmed = ngspice_path.trim();
    let use_cli = !trimmed.is_empty()
        && !trimmed.contains("libngspice")
        && !trimmed.ends_with(".so")
        && !trimmed.contains(".so.")
        && (trimmed != "ngspice" || std::path::Path::new("/usr/bin/ngspice").is_file());

    if use_cli && std::path::Path::new(trimmed).is_file() {
        return run_ngspice_cli(trimmed, netlist_lines, search_paths);
    }

    match NgSpiceEngine::global() {
        Ok(engine_mutex) => match engine_mutex.lock() {
            Ok(engine) => engine.run_netlist_with_paths(netlist_lines, search_paths),
            Err(e) => SimulationResult {
                plot_name: String::new(),
                x_name: String::new(),
                x_values: Vec::new(),
                traces: HashMap::new(),
                log: vec![format!("ngspice mutex poisoned: {e}")],
                success: false,
                error: Some(format!("ngspice mutex poisoned: {e}")),
            },
        },
        Err(err) => SimulationResult {
            plot_name: String::new(),
            x_name: String::new(),
            x_values: Vec::new(),
            traces: HashMap::new(),
            log: vec![err.clone()],
            success: false,
            error: Some(err),
        },
    }
}

fn run_ngspice_cli(
    exe: &str,
    netlist_lines: &[String],
    search_paths: &[PathBuf],
) -> SimulationResult {
    let tmp = std::env::temp_dir();
    let cir_path = tmp.join("mcp_itspice_run.cir");
    let raw_path = tmp.join("mcp_itspice_run.raw");

    let mut content = String::new();
    for l in netlist_lines {
        if l.trim().eq_ignore_ascii_case(".end") {
            continue;
        }
        content.push_str(l);
        content.push('\n');
    }
    content.push_str(".control\nset filetype=ascii\n");
    if !search_paths.is_empty() {
        content.push_str("set sourcepath = ( .");
        for p in search_paths {
            content.push_str(&format!(" \"{}\"", p.display()));
        }
        content.push_str(" )\n");
    }
    content.push_str(&format!(
        "run\nwrite {} all\nquit\n.endc\n.end\n",
        raw_path.display()
    ));

    if let Err(e) = std::fs::write(&cir_path, content) {
        return SimulationResult {
            plot_name: String::new(),
            x_name: String::new(),
            x_values: Vec::new(),
            traces: HashMap::new(),
            log: vec![format!("Failed to write temp netlist: {e}")],
            success: false,
            error: Some(format!("Failed to write temp netlist: {e}")),
        };
    }

    let output = match std::process::Command::new(exe)
        .arg("-b")
        .arg(&cir_path)
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            return SimulationResult {
                plot_name: String::new(),
                x_name: String::new(),
                x_values: Vec::new(),
                traces: HashMap::new(),
                log: vec![format!("Failed to execute '{exe}': {e}")],
                success: false,
                error: Some(format!("Failed to execute '{exe}': {e}")),
            };
        }
    };

    let mut logs: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .chain(String::from_utf8_lossy(&output.stderr).lines())
        .map(str::to_string)
        .collect();

    let Ok(raw_text) = std::fs::read_to_string(&raw_path) else {
        return SimulationResult {
            plot_name: String::new(),
            x_name: String::new(),
            x_values: Vec::new(),
            traces: HashMap::new(),
            log: logs,
            success: false,
            error: Some("ngspice CLI did not produce a raw output file".into()),
        };
    };

    parse_ascii_raw(&raw_text, &mut logs)
}

fn parse_ascii_raw(raw: &str, logs: &mut Vec<String>) -> SimulationResult {
    let mut plot_name = "tran1".to_string();
    let mut var_names: Vec<String> = Vec::new();
    let mut num_vars = 0usize;
    let mut num_points = 0usize;
    let mut in_vars = false;
    let mut in_vals = false;
    let mut values_flat: Vec<f64> = Vec::new();

    for line in raw.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if let Some(rest) = t.strip_prefix("Plotname:") {
            plot_name = rest.trim().to_string();
            in_vars = false;
        } else if let Some(rest) = t.strip_prefix("No. Variables:") {
            num_vars = rest.trim().parse().unwrap_or(0);
        } else if let Some(rest) = t.strip_prefix("No. Points:") {
            num_points = rest.trim().parse().unwrap_or(0);
        } else if t == "Variables:" {
            in_vars = true;
            in_vals = false;
        } else if t == "Values:" {
            in_vars = false;
            in_vals = true;
        } else if in_vars {
            let parts: Vec<&str> = t.split_whitespace().collect();
            if parts.len() >= 2 {
                var_names.push(parts[1].to_string());
            }
        } else if in_vals {
            let mut toks = t.split_whitespace();
            if let Some(first) = toks.next() {
                // First line of each point has `<index>\t<val0>`
                if let Some(second) = toks.next() {
                    if let Ok(v) = second.split(',').next().unwrap_or(second).parse::<f64>() {
                        values_flat.push(v);
                    }
                } else if let Ok(v) = first.split(',').next().unwrap_or(first).parse::<f64>() {
                    values_flat.push(v);
                }
            }
        }
    }

    if num_vars == 0 || num_points == 0 || var_names.len() < num_vars {
        logs.push("Failed to parse ASCII rawfile".into());
        return SimulationResult {
            plot_name,
            x_name: String::new(),
            x_values: Vec::new(),
            traces: HashMap::new(),
            log: logs.clone(),
            success: false,
            error: Some("Failed to parse ngspice ASCII raw output".into()),
        };
    }

    let mut cols: Vec<Vec<f64>> = vec![Vec::with_capacity(num_points); num_vars];
    for p in 0..num_points {
        for v in 0..num_vars {
            let idx = p * num_vars + v;
            if let Some(&val) = values_flat.get(idx) {
                cols[v].push(val);
            }
        }
    }

    let x_name = var_names[0].clone();
    let x_values = cols[0].clone();
    let mut traces = HashMap::new();
    for (v, name) in var_names.iter().enumerate() {
        let formatted = format_trace_name(name);
        traces.insert(
            formatted.clone(),
            TraceData {
                name: formatted,
                real: cols[v].clone(),
                imag: None,
            },
        );
    }

    SimulationResult {
        plot_name,
        x_name,
        x_values,
        traces,
        log: logs.clone(),
        success: true,
        error: None,
    }
}

fn format_sense_source(comp_tag: &str) -> String {
    // Multi-terminal pin sense sources: d_<m>, g_<m>, s_<m>, c_<q>, b_<q>, e_<q>, inp_<u>, inn_<u>, vp_<u>, vn_<u>, out_<u>, p<k>_<x>
    for (prefix, fn_prefix) in [
        ("d_", "Id"),
        ("g_", "Ig"),
        ("s_", "Is"),
        ("c_", "Ic"),
        ("b_", "Ib"),
        ("e_", "Ie"),
    ] {
        if let Some(comp) = comp_tag.strip_prefix(prefix) {
            return format!("{fn_prefix}({comp})");
        }
    }
    for (prefix, pin_label) in [
        ("inp_", "in+"),
        ("inn_", "in-"),
        ("vp_", "v+"),
        ("vn_", "v-"),
        ("out_", "out"),
    ] {
        if let Some(comp) = comp_tag.strip_prefix(prefix) {
            return format!("I({comp}:{pin_label})");
        }
    }
    if let Some(rest) = comp_tag.strip_prefix('p') {
        if let Some((num_str, comp)) = rest.split_once('_') {
            if !num_str.is_empty() && num_str.chars().all(|c| c.is_ascii_digit()) {
                return format!("I({comp}:{num_str})");
            }
        }
    }
    format!("I({comp_tag})")
}

fn format_trace_name(raw: &str) -> String {
    let lower = raw.to_lowercase();
    if lower.starts_with("v(") || lower.starts_with("i(") {
        if let Some(open) = raw.find('(') {
            let prefix = raw[..open].to_ascii_uppercase();
            let rest = &raw[open..];
            if prefix == "I" {
                let inner = rest.trim_start_matches('(').trim_end_matches(')');
                let inner_low = inner.to_ascii_lowercase();
                if let Some(comp) = inner_low.strip_prefix("v_sense_") {
                    return format_sense_source(comp);
                }
                if let Some(at_inner) = inner_low.strip_prefix('@') {
                    if let Some(bracket) = at_inner.find('[') {
                        let dev = &at_inner[..bracket];
                        return format!("I({dev})");
                    }
                }
            }
            return format!("{prefix}{rest}");
        }
    }
    if let Some(inner) = lower.strip_prefix('@') {
        if let Some(bracket) = inner.find('[') {
            let dev = &inner[..bracket];
            let param = &inner[bracket..];
            if param == "[i]" || param == "[id]" {
                return format!("I({dev})");
            }
        }
    }
    if lower.ends_with("#branch") {
        let base = &raw[..raw.len() - "#branch".len()];
        let base_lower = base.to_ascii_lowercase();
        if let Some(comp) = base_lower.strip_prefix("v_sense_") {
            return format_sense_source(comp);
        }
        return format!("I({base})");
    }
    if lower == "time" || lower == "frequency" || lower == "v-sweep" || lower == "i-sweep" {
        return lower;
    }
    format!("V({raw})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ngspice_rc_transient() {
        let engine_mutex = NgSpiceEngine::global().expect("libngspice should load");
        let engine = engine_mutex.lock().unwrap();
        let netlist = vec![
            "* RC test".to_string(),
            "V1 in 0 PULSE(0 5 0 1u 1u 10 20)".to_string(),
            "R1 in out 100k".to_string(),
            "C1 out 0 10u".to_string(),
            ".save all @r1[i] @c1[i]".to_string(),
            ".tran 10m 5".to_string(),
            ".end".to_string(),
        ];
        let res = engine.run_netlist(&netlist);
        assert!(res.success, "sim failed: {:?} logs: {:?}", res.error, res.log);
        assert!(!res.x_values.is_empty(), "expected time points");
        let v_out = res.traces.get("V(out)").expect("V(out) trace should exist");
        let last = *v_out.real.last().unwrap();
        assert!(last > 4.9 && last <= 5.01, "expected ~4.96V, got {last}");
        assert!(res.traces.contains_key("I(c1)"), "I(c1) trace should exist");
        assert!(res.traces.contains_key("I(r1)"), "I(r1) trace should exist");
        assert_eq!(format_trace_name("i(@c1[i])"), "I(c1)");
        assert_eq!(format_trace_name("i(@r1[i])"), "I(r1)");
        assert_eq!(format_trace_name("i(v_sense_c1)"), "I(c1)");
        assert_eq!(format_trace_name("i(v_sense_d_m1)"), "Id(m1)");
        assert_eq!(format_trace_name("v_sense_g_m1#branch"), "Ig(m1)");
        assert_eq!(format_trace_name("i(v_sense_s_m1)"), "Is(m1)");
        assert_eq!(format_trace_name("i(v_sense_c_q1)"), "Ic(q1)");
        assert_eq!(format_trace_name("v_sense_b_q1#branch"), "Ib(q1)");
        assert_eq!(format_trace_name("i(v_sense_e_q1)"), "Ie(q1)");
        assert_eq!(format_trace_name("i(v_sense_out_u1)"), "I(u1:out)");
        assert_eq!(format_trace_name("v_sense_p2_x1#branch"), "I(x1:2)");
    }
}
