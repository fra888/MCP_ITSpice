use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};

use egui::{Color32, FontId, RichText, ScrollArea, Ui};

use crate::model::NetlistResult;
use crate::ngspice::SimulationResult;

const PYTHON_REPL_DRIVER: &str = r#"
import sys, json, math, statistics, code
try:
    import numpy as np
    HAS_NP = True
except Exception:
    np = None
    HAS_NP = False

SIM_PATH = sys.argv[1]
OUT_PATH = sys.argv[2]
ns = {
    "json": json,
    "math": math,
    "statistics": statistics,
    "np": np,
    "sim": {},
    "x": [],
    "x_name": "",
    "traces": {},
    "netlist": [],
}

def reload():
    try:
        with open(SIM_PATH, "r", encoding="utf-8") as f:
            data = json.load(f)
        ns["sim"] = data
        ns["x_name"] = data.get("x_name", "")
        ns["netlist"] = data.get("netlist", [])
        xv = data.get("x", [])
        ns["x"] = np.array(xv, dtype=float) if HAS_NP else xv
        raw_tr = data.get("traces", {})
        if HAS_NP:
            ns["traces"] = {k: np.array(v, dtype=float) for k, v in raw_tr.items()}
        else:
            ns["traces"] = raw_tr
        return len(ns["traces"])
    except Exception as e:
        print(f"reload() error: {e}", file=sys.stderr)
        return 0

def tr(name):
    return ns["traces"][name]

def rms(sig):
    if HAS_NP:
        a = np.asarray(sig, dtype=float)
        return float(np.sqrt(np.mean(a * a))) if len(a) else 0.0
    return math.sqrt(sum(v * v for v in sig) / len(sig)) if sig else 0.0

def avg(sig):
    if HAS_NP:
        a = np.asarray(sig, dtype=float)
        return float(np.mean(a)) if len(a) else 0.0
    return (sum(sig) / len(sig)) if sig else 0.0

def pp(sig):
    return float(max(sig) - min(sig)) if len(sig) else 0.0

def info():
    tkeys = sorted(ns["traces"].keys())
    print(f"x_name={ns['x_name']!r}, points={len(ns['x'])}, traces={tkeys}")

def plot(name, y, x=None, x_unit=None, y_unit=None, new_graph=False, graph=None):
    vals = [float(v) for v in y]
    ns["traces"][name] = np.array(vals, dtype=float) if HAS_NP else vals
    obj = {"name": str(name), "y": vals, "new_graph": bool(new_graph)}
    if x is not None:
        obj["x"] = [float(v) for v in x]
    if x_unit is not None:
        obj["x_unit"] = str(x_unit)
    if y_unit is not None:
        obj["y_unit"] = str(y_unit)
    if graph is not None:
        obj["graph"] = int(graph)
    print("__ITSPICE_OUT_JSON__:" + json.dumps(obj))
    sys.stdout.flush()
    target = "new plot" if new_graph else (f"graph {graph}" if graph is not None else "integrated plot")
    unit_info = f" (x_unit={x_unit})" if x_unit else ""
    print(f"Wrote waveform '{name}' ({len(vals)} points) to {target}{unit_info}")

def new_plot(name, y, x=None, x_unit=None, y_unit=None):
    return plot(name, y, x=x, x_unit=x_unit, y_unit=y_unit, new_graph=True)

def new_graph():
    payload = json.dumps({"name": "", "y": [], "new_graph": True})
    print("__ITSPICE_OUT_JSON__:" + payload)
    sys.stdout.flush()
    print("Created new plot pane")

def set_xaxis(x_vals, unit="Hz", graph=None, y_unit=None):
    obj = {
        "name": "",
        "y": [],
        "new_graph": False,
        "x": [float(v) for v in x_vals],
        "x_unit": str(unit),
    }
    if graph is not None:
        obj["graph"] = int(graph)
    if y_unit is not None:
        obj["y_unit"] = str(y_unit)
    print("__ITSPICE_OUT_JSON__:" + json.dumps(obj))
    sys.stdout.flush()
    g_label = f"graph {graph}" if graph is not None else "focused graph"
    print(f"Overrode X-axis on {g_label}: {len(obj['x'])} points, unit='{unit}'")

def run_script(path):
    with open(path, "r", encoding="utf-8") as f:
        src = f.read()
    exec(compile(src, path, "exec"), ns)

def run_code(src):
    try:
        try:
            code_obj = compile(src, "<mcp>", "eval")
            res = eval(code_obj, ns)
            if res is not None:
                print(repr(res))
        except SyntaxError:
            exec(compile(src, "<mcp>", "exec"), ns)
    except Exception:
        console.showtraceback()

ns.update({
    "reload": reload,
    "tr": tr,
    "rms": rms,
    "avg": avg,
    "pp": pp,
    "info": info,
    "plot": plot,
    "new_plot": new_plot,
    "new_graph": new_graph,
    "set_xaxis": set_xaxis,
    "write_waveform": plot,
    "add_trace": plot,
    "run_script": run_script,
    "run_code": run_code,
})
n_tr = reload()
ver = sys.version.split()[0]
print(f"Python {ver} ready (numpy={'yes' if HAS_NP else 'no'}, {n_tr} traces loaded). Helpers: tr(name), rms(s), avg(s), pp(s), plot(name, y), new_plot(name, y, x=..., x_unit='Hz'), set_xaxis(x, unit='Hz', graph=None), new_graph(), reload(), info()")
sys.stdout.flush()

class TrackingConsole(code.InteractiveConsole):
    def __init__(self, locals=None):
        super().__init__(locals=locals)
        self.had_error = False

    def showsyntaxerror(self, filename=None):
        self.had_error = True
        super().showsyntaxerror(filename)

    def showtraceback(self):
        self.had_error = True
        super().showtraceback()

console = TrackingConsole(locals=ns)
for line in sys.stdin:
    console.had_error = False
    more = console.push(line.rstrip("\r\n"))
    if more:
        print("__ITSPICE_CMD_MORE__")
    elif console.had_error:
        print("__ITSPICE_CMD_ERR__")
    else:
        print("__ITSPICE_CMD_OK__")
    sys.stdout.flush()
    sys.stderr.flush()
"#;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TermLineKind {
    Command,
    Stdout,
    Stderr,
    System,
}

pub struct PythonTerminalState {
    pub input: String,
    pub history: Vec<String>,
    pub history_idx: Option<usize>,
    pub lines: Vec<(String, TermLineKind)>,
    pub sim_json_path: PathBuf,
    pub out_json_path: PathBuf,
    pub python_exe: String,
    pub recording: bool,
    pub recorded_commands: Vec<String>,
    pub loaded_script: Option<PathBuf>,
    pending_commands: std::collections::VecDeque<Option<String>>,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    rx: Option<Receiver<(String, TermLineKind)>>,
}

impl Default for PythonTerminalState {
    fn default() -> Self {
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let pid = std::process::id();
        Self {
            input: String::new(),
            history: Vec::new(),
            history_idx: None,
            lines: Vec::new(),
            sim_json_path: std::env::temp_dir().join(format!("mcp_itspice_py_sim_{pid}_{id}.json")),
            out_json_path: std::env::temp_dir().join(format!("mcp_itspice_py_out_{pid}_{id}.json")),
            python_exe: crate::settings::detect_python_path(),
            recording: false,
            recorded_commands: Vec::new(),
            loaded_script: None,
            pending_commands: std::collections::VecDeque::new(),
            child: None,
            stdin: None,
            rx: None,
        }
    }
}

impl Drop for PythonTerminalState {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Debug, Clone)]
pub struct PyPlotCommand {
    pub name: String,
    pub y: Vec<f64>,
    pub new_graph: bool,
    pub graph_idx: Option<usize>,
    pub x_values: Option<Vec<f64>>,
    pub x_unit: Option<String>,
    pub y_unit: Option<String>,
}

fn parse_py_plot_cmd(json_str: &str) -> Option<PyPlotCommand> {
    let val = serde_json::from_str::<serde_json::Value>(json_str).ok()?;
    let name = val
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let new_graph = val
        .get("new_graph")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let graph_idx = val
        .get("graph")
        .and_then(|v| v.as_u64())
        .map(|g| g as usize);
    let y: Vec<f64> = val
        .get("y")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|x| x.as_f64()).collect())
        .unwrap_or_default();
    let x_values = val
        .get("x")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|x| x.as_f64()).collect());
    let x_unit = val
        .get("x_unit")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let y_unit = val
        .get("y_unit")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    Some(PyPlotCommand {
        name,
        y,
        new_graph,
        graph_idx,
        x_values,
        x_unit,
        y_unit,
    })
}

impl PythonTerminalState {
    pub fn is_running(&self) -> bool {
        self.child.is_some()
    }

    pub fn stop(&mut self) {
        self.stdin = None;
        self.rx = None;
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    pub fn sync_sim_data(&self, sim: Option<&SimulationResult>, netlist: &NetlistResult) {
        let mut traces_map = serde_json::Map::new();
        let (plot_name, x_name, x_vals) = if let Some(s) = sim.filter(|s| s.success) {
            for (k, v) in &s.traces {
                traces_map.insert(k.clone(), serde_json::json!(v.real));
            }
            (s.plot_name.clone(), s.x_name.clone(), s.x_values.clone())
        } else {
            (String::new(), String::new(), Vec::new())
        };
        let payload = serde_json::json!({
            "plot_name": plot_name,
            "x_name": x_name,
            "x": x_vals,
            "traces": traces_map,
            "netlist": netlist.lines,
        });
        if let Ok(json_str) = serde_json::to_string(&payload) {
            let _ = std::fs::write(&self.sim_json_path, json_str);
        }
    }

    pub fn ensure_started(&mut self, sim: Option<&SimulationResult>, netlist: &NetlistResult) {
        if self.child.is_some() {
            return;
        }
        self.sync_sim_data(sim, netlist);

        let mut cmd = Command::new(&self.python_exe);
        cmd.arg("-u")
            .arg("-c")
            .arg(PYTHON_REPL_DRIVER)
            .arg(&self.sim_json_path)
            .arg(&self.out_json_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }

        match cmd.spawn() {
            Ok(mut child) => {
                let stdin = child.stdin.take();
                let stdout = child.stdout.take();
                let stderr = child.stderr.take();
                let (tx, rx) = mpsc::channel();

                if let Some(out) = stdout {
                    let tx_out = tx.clone();
                    std::thread::spawn(move || {
                        let reader = BufReader::new(out);
                        for line in reader.lines().map_while(Result::ok) {
                            let _ = tx_out.send((line, TermLineKind::Stdout));
                        }
                    });
                }
                if let Some(err) = stderr {
                    let tx_err = tx;
                    std::thread::spawn(move || {
                        let reader = BufReader::new(err);
                        for line in reader.lines().map_while(Result::ok) {
                            let _ = tx_err.send((line, TermLineKind::Stderr));
                        }
                    });
                }

                self.child = Some(child);
                self.stdin = stdin;
                self.rx = Some(rx);
            }
            Err(e) => {
                self.lines.push((
                    format!("Failed to start '{}': {e}", self.python_exe),
                    TermLineKind::Stderr,
                ));
            }
        }
    }

    pub fn poll_output(&mut self) -> Vec<PyPlotCommand> {
        let mut new_waveforms = Vec::new();
        if let Some(rx) = &self.rx {
            while let Ok((line, kind)) = rx.try_recv() {
                if kind == TermLineKind::Stdout {
                    let trimmed = line.trim();
                    if trimmed == "__ITSPICE_CMD_OK__" {
                        if let Some(Some(cmd)) = self.pending_commands.pop_front() {
                            if self.recording {
                                self.recorded_commands.push(cmd);
                            }
                        }
                        continue;
                    } else if trimmed == "__ITSPICE_CMD_ERR__" {
                        let _ = self.pending_commands.pop_front();
                        continue;
                    } else if trimmed == "__ITSPICE_CMD_MORE__" {
                        if let Some(Some(cmd)) = self.pending_commands.pop_front() {
                            if self.recording {
                                self.recorded_commands.push(cmd);
                            }
                        }
                        continue;
                    } else if let Some(json_str) = trimmed.strip_prefix("__ITSPICE_OUT_JSON__:") {
                        if let Some(cmd) = parse_py_plot_cmd(json_str) {
                            new_waveforms.push(cmd);
                        }
                        continue;
                    }
                }
                self.lines.push((line, kind));
            }
            if self.lines.len() > 2000 {
                let excess = self.lines.len() - 2000;
                self.lines.drain(0..excess);
            }
        }
        new_waveforms
    }

    pub fn send_command(&mut self, cmd_str: &str, echo: bool) {
        if echo {
            self.lines
                .push((format!(">>> {cmd_str}"), TermLineKind::Command));
            self.pending_commands.push_back(Some(cmd_str.to_string()));
        } else {
            self.pending_commands.push_back(None);
        }
        if let Some(stdin) = &mut self.stdin {
            if let Err(e) = writeln!(stdin, "{cmd_str}").and_then(|_| stdin.flush()) {
                self.lines
                    .push((format!("Python write error: {e}"), TermLineKind::Stderr));
                self.stop();
            }
        }
    }

    pub fn play_script(&mut self, path: &std::path::Path) {
        let escaped = path
            .display()
            .to_string()
            .replace('\\', "\\\\")
            .replace('\'', "\\'");
        self.lines.push((
            format!("Running script: {}", path.display()),
            TermLineKind::System,
        ));
        self.send_command(&format!("run_script('{escaped}')"), false);
    }

    /// Execute Python code synchronously (for MCP) and return `(stdout, stderr, had_error, plots)`.
    pub fn execute_blocking(
        &mut self,
        code: &str,
        sim: Option<&SimulationResult>,
        netlist: &NetlistResult,
    ) -> (String, String, bool, Vec<PyPlotCommand>) {
        self.ensure_started(sim, netlist);
        self.sync_sim_data(sim, netlist);
        let _ = self.poll_output(); // drain any prior banner lines

        let mut stdout_buf = String::new();
        let mut stderr_buf = String::new();
        let mut plots = Vec::new();
        let mut had_error = false;

        // First reload latest sim data, then run the user's code
        let code_json = serde_json::to_string(code).unwrap_or_else(|_| "\"\"".to_string());
        let cmd = format!("reload(); run_code({code_json})");
        self.send_command(&cmd, false);

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while std::time::Instant::now() < deadline {
            if let Some(rx) = &self.rx {
                match rx.recv_timeout(std::time::Duration::from_millis(50)) {
                    Ok((line, kind)) => {
                        if kind == TermLineKind::Stdout {
                            let trimmed = line.trim();
                            if trimmed == "__ITSPICE_CMD_OK__" {
                                break;
                            } else if trimmed == "__ITSPICE_CMD_ERR__" {
                                had_error = true;
                                break;
                            } else if trimmed == "__ITSPICE_CMD_MORE__" {
                                continue;
                            } else if let Some(json_str) =
                                trimmed.strip_prefix("__ITSPICE_OUT_JSON__:")
                            {
                                if let Some(cmd) = parse_py_plot_cmd(json_str) {
                                    plots.push(cmd);
                                }
                                continue;
                            }
                            if !stdout_buf.is_empty() {
                                stdout_buf.push('\n');
                            }
                            stdout_buf.push_str(&line);
                        } else if kind == TermLineKind::Stderr {
                            if !stderr_buf.is_empty() {
                                stderr_buf.push('\n');
                            }
                            stderr_buf.push_str(&line);
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        had_error = true;
                        stderr_buf.push_str("Python process disconnected");
                        break;
                    }
                }
            } else {
                had_error = true;
                break;
            }
        }

        (stdout_buf, stderr_buf, had_error, plots)
    }
}

pub fn draw_python_terminal(
    ui: &mut Ui,
    state: &mut PythonTerminalState,
    sim: Option<&SimulationResult>,
    netlist: &NetlistResult,
) -> Vec<PyPlotCommand> {
    state.ensure_started(sim, netlist);
    let new_waveforms = state.poll_output();

    if state.is_running() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(80));
    }

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("Python Analysis Terminal ({})", state.python_exe))
                .strong()
                .color(Color32::from_rgb(130, 200, 255)),
        );
        ui.separator();
        if ui.small_button("info()").clicked() {
            state.send_command("info()", true);
        }
        if ui.small_button("Sync Sim").clicked() {
            state.sync_sim_data(sim, netlist);
            state.send_command("reload()", true);
        }
        if ui.small_button("Clear").clicked() {
            state.lines.clear();
        }
        if ui.small_button("Restart").clicked() {
            state.stop();
            state.lines.push(("Restarting Python...".into(), TermLineKind::System));
            state.ensure_started(sim, netlist);
        }
        ui.separator();
        if state.recording {
            let stop_btn = egui::Button::new(
                RichText::new(format!("■ Stop ({})", state.recorded_commands.len()))
                    .color(Color32::WHITE),
            )
            .fill(Color32::from_rgb(185, 45, 45));
            if ui.add(stop_btn).clicked() {
                state.recording = false;
                state.lines.push((
                    format!(
                        "Recording stopped ({} non-failing command(s) captured).",
                        state.recorded_commands.len()
                    ),
                    TermLineKind::System,
                ));
            }
        } else {
            let rec_btn = egui::Button::new(
                RichText::new("● Record").color(Color32::from_rgb(245, 110, 110)),
            );
            if ui.add(rec_btn).clicked() {
                state.recorded_commands.clear();
                state.recording = true;
                state.lines.push((
                    "Recording started (all non-failing commands will be captured).".into(),
                    TermLineKind::System,
                ));
            }
        }
        if !state.recorded_commands.is_empty() {
            if ui
                .small_button(format!("Save Script ({})...", state.recorded_commands.len()))
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Python Script", &["py"])
                    .set_file_name("analysis_script.py")
                    .save_file()
                {
                    let body = format!("{}\n", state.recorded_commands.join("\n"));
                    match std::fs::write(&path, body) {
                        Ok(()) => state.lines.push((
                            format!("Saved script to {}", path.display()),
                            TermLineKind::System,
                        )),
                        Err(e) => state.lines.push((
                            format!("Failed to save script: {e}"),
                            TermLineKind::Stderr,
                        )),
                    }
                }
            }
        }
        ui.separator();
        if ui.small_button("Load & Play...").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Python Script", &["py"])
                .pick_file()
            {
                state.loaded_script = Some(path.clone());
                state.play_script(&path);
            }
        }
        if let Some(script_path) = state.loaded_script.clone() {
            let fname = script_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("script.py");
            if ui.small_button(format!("▶ Play ({fname})")).clicked() {
                state.play_script(&script_path);
            }
        }
    });
    ui.separator();

    let available_h = (ui.available_height() - 30.0).max(60.0);
    egui::Frame::none()
        .fill(Color32::from_rgb(18, 20, 24))
        .inner_margin(6.0)
        .show(ui, |ui| {
            ScrollArea::vertical()
                .max_height(available_h)
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for (txt, kind) in &state.lines {
                        let color = match kind {
                            TermLineKind::Command => Color32::from_rgb(100, 210, 255),
                            TermLineKind::Stdout => Color32::from_rgb(225, 225, 225),
                            TermLineKind::Stderr => Color32::from_rgb(255, 105, 105),
                            TermLineKind::System => Color32::from_rgb(255, 210, 100),
                        };
                        ui.label(RichText::new(txt).font(FontId::monospace(12.0)).color(color));
                    }
                });
        });

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(">>>")
                .font(FontId::monospace(12.5))
                .color(Color32::from_rgb(100, 210, 255)),
        );
        let resp = ui.add_sized(
            [ui.available_width() - 60.0, 22.0],
            egui::TextEdit::singleline(&mut state.input)
                .font(FontId::monospace(12.5))
                .hint_text("e.g. info(), rms(tr('V(out)')), np.fft.rfft(tr('V(out)'))"),
        );

        if resp.has_focus() {
            if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) && !state.history.is_empty() {
                let next_idx = match state.history_idx {
                    None => state.history.len() - 1,
                    Some(idx) => idx.saturating_sub(1),
                };
                state.history_idx = Some(next_idx);
                state.input = state.history[next_idx].clone();
            } else if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                if let Some(idx) = state.history_idx {
                    if idx + 1 < state.history.len() {
                        state.history_idx = Some(idx + 1);
                        state.input = state.history[idx + 1].clone();
                    } else {
                        state.history_idx = None;
                        state.input.clear();
                    }
                }
            }
        }

        let enter_pressed =
            resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let run_clicked = ui.button("Run").clicked();
        if enter_pressed || run_clicked {
            let cmd = state.input.trim_end_matches(['\r', '\n']).to_string();
            if !cmd.is_empty() {
                if state.history.last().is_none_or(|last| last != &cmd) {
                    state.history.push(cmd.clone());
                }
                state.history_idx = None;
                state.send_command(&cmd, true);
                state.input.clear();
            }
            resp.request_focus();
        }
    });

    new_waveforms
}
