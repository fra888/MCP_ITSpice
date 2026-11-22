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

def plot(name, y, new_graph=False):
    vals = [float(v) for v in y]
    ns["traces"][name] = np.array(vals, dtype=float) if HAS_NP else vals
    with open(OUT_PATH, "w", encoding="utf-8") as f:
        json.dump({"name": str(name), "y": vals, "new_graph": bool(new_graph)}, f)
    print("__ITSPICE_OUT__")
    sys.stdout.flush()
    target = "new plot" if new_graph else "integrated plot"
    print(f"Wrote waveform '{name}' ({len(vals)} points) to {target}")

def new_plot(name, y):
    return plot(name, y, new_graph=True)

def new_graph():
    with open(OUT_PATH, "w", encoding="utf-8") as f:
        json.dump({"name": "", "y": [], "new_graph": True}, f)
    print("__ITSPICE_OUT__")
    sys.stdout.flush()
    print("Created new plot pane")

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
    "write_waveform": plot,
    "add_trace": plot,
})
n_tr = reload()
ver = sys.version.split()[0]
print(f"Python {ver} ready (numpy={'yes' if HAS_NP else 'no'}, {n_tr} traces loaded). Helpers: tr(name), rms(s), avg(s), pp(s), plot(name, y), new_plot(name, y), new_graph(), reload(), info()")
sys.stdout.flush()

console = code.InteractiveConsole(locals=ns)
for line in sys.stdin:
    console.push(line.rstrip("\r\n"))
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
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    rx: Option<Receiver<(String, TermLineKind)>>,
}

impl Default for PythonTerminalState {
    fn default() -> Self {
        Self {
            input: String::new(),
            history: Vec::new(),
            history_idx: None,
            lines: Vec::new(),
            sim_json_path: std::env::temp_dir().join("mcp_itspice_py_sim.json"),
            out_json_path: std::env::temp_dir().join("mcp_itspice_py_out.json"),
            python_exe: "python3".to_string(),
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

pub struct PyPlotCommand {
    pub name: String,
    pub y: Vec<f64>,
    pub new_graph: bool,
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
                if kind == TermLineKind::Stdout && line.trim() == "__ITSPICE_OUT__" {
                    if let Ok(raw) = std::fs::read_to_string(&self.out_json_path) {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) {
                            let name = val
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let new_graph = val
                                .get("new_graph")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            let y: Vec<f64> = val
                                .get("y")
                                .and_then(|v| v.as_array())
                                .map(|arr| arr.iter().filter_map(|x| x.as_f64()).collect())
                                .unwrap_or_default();
                            new_waveforms.push(PyPlotCommand { name, y, new_graph });
                        }
                    }
                    continue;
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
        }
        if let Some(stdin) = &mut self.stdin {
            if let Err(e) = writeln!(stdin, "{cmd_str}").and_then(|_| stdin.flush()) {
                self.lines
                    .push((format!("Python write error: {e}"), TermLineKind::Stderr));
                self.stop();
            }
        }
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
