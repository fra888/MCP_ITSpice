use std::io::{self, BufRead, Write};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use crate::model::{ComponentKind, Schematic};
use crate::ngspice::{run_simulation_with_settings, SimulationResult};
use crate::settings::{AppSettings, ModelIndex};

pub struct McpSharedState {
    pub schematic: Schematic,
    pub last_sim: Option<SimulationResult>,
    pub revision: u64,
    pub zoom_fit_requested: bool,
    pub egui_ctx: Option<egui::Context>,
}

impl McpSharedState {
    pub fn new() -> Self {
        Self {
            schematic: Schematic::default_ltspice_demo(),
            last_sim: None,
            revision: 0,
            zoom_fit_requested: false,
            egui_ctx: None,
        }
    }

    pub fn notify_gui(&mut self) {
        self.revision += 1;
        if let Some(ctx) = &self.egui_ctx {
            ctx.request_repaint();
        }
    }
}

pub struct McpServer {
    shared: Arc<Mutex<McpSharedState>>,
}

impl McpServer {
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Mutex::new(McpSharedState::new())),
        }
    }

    pub fn with_shared(shared: Arc<Mutex<McpSharedState>>) -> Self {
        Self { shared }
    }

    pub fn run_stdio(&mut self) -> io::Result<()> {
        let stdin = io::stdin();
        let stdout = io::stdout();
        let mut out = io::BufWriter::new(stdout.lock());

        for line in stdin.lock().lines() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let Ok(req) = serde_json::from_str::<Value>(trimmed) else {
                let err_resp = json!({
                    "jsonrpc": "2.0",
                    "id": Value::Null,
                    "error": { "code": -32700, "message": "Parse error" }
                });
                writeln!(out, "{err_resp}")?;
                out.flush()?;
                continue;
            };

            if let Some(resp) = self.handle_message(&req) {
                writeln!(out, "{resp}")?;
                out.flush()?;
            }
        }
        Ok(())
    }

    pub fn handle_message(&mut self, req: &Value) -> Option<Value> {
        let id = req.get("id").cloned();
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = req.get("params").cloned().unwrap_or(Value::Null);

        // Notifications have no `id` and expect no response.
        if id.is_none() || id.as_ref() == Some(&Value::Null) && method.starts_with("notifications/") {
            return None;
        }

        let id = id?;
        let resp = match method {
            "initialize" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "MCP_ITSpice",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }
            }),
            "ping" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {}
            }),
            "tools/list" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "tools": tool_definitions()
                }
            }),
            "tools/call" => {
                let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                let (text, is_error) = self.call_tool(name, &args);
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": text }],
                        "isError": is_error
                    }
                })
            }
            _ => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32601,
                    "message": format!("Method not found: {method}")
                }
            }),
        };
        Some(resp)
    }

    fn call_tool(&mut self, name: &str, args: &Value) -> (String, bool) {
        let mut st = self.shared.lock().unwrap();
        match name {
            "get_schematic" => {
                let net = st.schematic.to_spice_netlist();
                let payload = json!({
                    "name": st.schematic.name,
                    "components": st.schematic.components,
                    "wires": st.schematic.wires,
                    "directives": st.schematic.directives,
                    "netlist": net.lines
                });
                (serde_json::to_string_pretty(&payload).unwrap_or_default(), false)
            }
            "new_schematic" => {
                let load_demo = args
                    .get("load_demo")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let name = args
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Draft1.asc");
                st.schematic = if load_demo {
                    Schematic::default_ltspice_demo()
                } else {
                    Schematic::new_empty(name)
                };
                st.last_sim = None;
                st.zoom_fit_requested = true;
                st.notify_gui();
                (format!("Initialized schematic '{}'", st.schematic.name), false)
            }
            "load_schematic" => {
                let Some(path) = args.get("path").and_then(|v| v.as_str()) else {
                    return ("Missing required argument: path".into(), true);
                };
                match std::fs::read_to_string(path) {
                    Ok(content) => {
                        let fname = std::path::Path::new(path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("Schematic.asc");
                        st.schematic = if path.ends_with(".json") {
                            serde_json::from_str(&content)
                                .unwrap_or_else(|_| Schematic::from_asc(fname, &content))
                        } else {
                            Schematic::from_asc(fname, &content)
                        };
                        st.last_sim = None;
                        st.zoom_fit_requested = true;
                        st.notify_gui();
                        (
                            format!(
                                "Loaded '{}' ({} components, {} wires)",
                                st.schematic.name,
                                st.schematic.components.len(),
                                st.schematic.wires.len()
                            ),
                            false,
                        )
                    }
                    Err(e) => (format!("Failed to read '{path}': {e}"), true),
                }
            }
            "save_schematic" => {
                let Some(path) = args.get("path").and_then(|v| v.as_str()) else {
                    return ("Missing required argument: path".into(), true);
                };
                let content = if path.ends_with(".json") {
                    serde_json::to_string_pretty(&st.schematic).unwrap_or_default()
                } else {
                    st.schematic.to_asc()
                };
                match std::fs::write(path, content) {
                    Ok(()) => (format!("Saved schematic to '{path}'"), false),
                    Err(e) => (format!("Failed to write '{path}': {e}"), true),
                }
            }
            "add_component" => {
                let Some(kind_str) = args.get("kind").and_then(|v| v.as_str()) else {
                    return ("Missing required argument: kind".into(), true);
                };
                let Some(kind) = parse_component_kind(kind_str) else {
                    return (format!("Unknown component kind: '{kind_str}'"), true);
                };
                let x = args.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let y = args.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let refdes = args
                    .get("refdes")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| st.schematic.next_refdes(kind));
                let value = args
                    .get("value")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| kind.default_value().to_string());
                let rotation = args.get("rotation").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let mirror = args.get("mirror").and_then(|v| v.as_bool()).unwrap_or(false);

                let id = st
                    .schematic
                    .add_component(kind, refdes.clone(), value.clone(), x, y);
                if let Some(c) = st.schematic.components.iter_mut().find(|c| c.id == id) {
                    c.rotation = rotation;
                    c.mirror = mirror;
                }
                st.schematic.split_wires_for_component(id);
                st.notify_gui();
                (
                    json!({ "id": id, "refdes": refdes, "value": value, "x": x, "y": y }).to_string(),
                    false,
                )
            }
            "edit_component" => {
                let target_id = args.get("id").and_then(|v| v.as_u64());
                let target_ref = args.get("refdes").and_then(|v| v.as_str());
                let comp = st.schematic.components.iter_mut().find(|c| {
                    target_id.is_some_and(|id| c.id == id)
                        || target_ref.is_some_and(|r| c.refdes.eq_ignore_ascii_case(r))
                });
                let Some(c) = comp else {
                    return ("Component not found".into(), true);
                };
                if let Some(v) = args.get("value").and_then(|v| v.as_str()) {
                    c.value = v.to_string();
                }
                if let Some(v) = args.get("new_refdes").and_then(|v| v.as_str()) {
                    c.refdes = v.to_string();
                }
                if let Some(x) = args.get("x").and_then(|v| v.as_i64()) {
                    c.x = x as i32;
                }
                if let Some(y) = args.get("y").and_then(|v| v.as_i64()) {
                    c.y = y as i32;
                }
                if let Some(rot) = args.get("rotation").and_then(|v| v.as_i64()) {
                    c.rotation = rot as i32;
                }
                if let Some(m) = args.get("mirror").and_then(|v| v.as_bool()) {
                    c.mirror = m;
                }
                let out = serde_json::to_string(c).unwrap_or_default();
                st.notify_gui();
                (out, false)
            }
            "remove_component" => {
                let target_id = args.get("id").and_then(|v| v.as_u64());
                let target_ref = args.get("refdes").and_then(|v| v.as_str());
                let before = st.schematic.components.len();
                st.schematic.components.retain(|c| {
                    !(target_id.is_some_and(|id| c.id == id)
                        || target_ref.is_some_and(|r| c.refdes.eq_ignore_ascii_case(r)))
                });
                let removed = before - st.schematic.components.len();
                if removed > 0 {
                    st.notify_gui();
                }
                (format!("Removed {removed} component(s)"), removed == 0)
            }
            "add_wire" => {
                let x1 = args.get("x1").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let y1 = args.get("y1").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let x2 = args.get("x2").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let y2 = args.get("y2").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let mut ids = Vec::new();
                if x1 != x2 && y1 != y2 {
                    ids.push(st.schematic.add_wire(x1, y1, x2, y1));
                    ids.push(st.schematic.add_wire(x2, y1, x2, y2));
                } else {
                    ids.push(st.schematic.add_wire(x1, y1, x2, y2));
                }
                st.notify_gui();
                (json!({ "wire_ids": ids }).to_string(), false)
            }
            "set_directive" => {
                let Some(text) = args.get("text").and_then(|v| v.as_str()) else {
                    return ("Missing required argument: text".into(), true);
                };
                let x = args.get("x").and_then(|v| v.as_i64()).unwrap_or(8) as i32;
                let y = args.get("y").and_then(|v| v.as_i64()).unwrap_or(24) as i32;
                let replace_analysis = args
                    .get("replace_analysis")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                if replace_analysis && is_analysis_directive(text) {
                    if let Some(d) = st
                        .schematic
                        .directives
                        .iter_mut()
                        .find(|d| is_analysis_directive(&d.text))
                    {
                        d.text = text.to_string();
                        st.notify_gui();
                        return (format!("Updated directive: {text}"), false);
                    }
                }
                let id = st.schematic.add_directive(x, y, text.to_string());
                st.notify_gui();
                (json!({ "id": id, "text": text }).to_string(), false)
            }
            "run_simulation" => {
                let settings = AppSettings::load();
                let dirs = settings.all_model_dirs();
                let index = ModelIndex::scan(&dirs);
                let net = st.schematic.to_spice_netlist_with_models(Some(&index));
                drop(st);
                self.execute_netlist(&net.lines, &dirs)
            }
            "run_spice_netlist" => {
                let Some(netlist_str) = args.get("netlist").and_then(|v| v.as_str()) else {
                    return ("Missing required argument: netlist".into(), true);
                };
                let settings = AppSettings::load();
                let dirs = settings.all_model_dirs();
                let lines: Vec<String> = netlist_str.lines().map(str::to_string).collect();
                drop(st);
                self.execute_netlist(&lines, &dirs)
            }
            "get_trace" => {
                let Some(trace_name) = args.get("trace").and_then(|v| v.as_str()) else {
                    return ("Missing required argument: trace".into(), true);
                };
                let max_points = args
                    .get("max_points")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(200)
                    .clamp(10, 5000) as usize;
                let Some(sim) = &st.last_sim else {
                    return ("No simulation has been run yet. Call run_simulation first.".into(), true);
                };
                let Some(tr) = sim.traces.get(trace_name).or_else(|| {
                    sim.traces
                        .iter()
                        .find(|(k, _)| k.eq_ignore_ascii_case(trace_name))
                        .map(|(_, v)| v)
                }) else {
                    let avail: Vec<&String> = sim.traces.keys().collect();
                    return (
                        format!("Trace '{trace_name}' not found. Available: {avail:?}"),
                        true,
                    );
                };
                let n = sim.x_values.len().min(tr.real.len());
                let step = (n / max_points).max(1);
                let points: Vec<Value> = (0..n)
                    .step_by(step)
                    .map(|i| json!([sim.x_values[i], tr.real[i]]))
                    .collect();
                let out = json!({
                    "trace": tr.name,
                    "x_name": sim.x_name,
                    "total_points": n,
                    "returned_points": points.len(),
                    "points": points
                });
                (serde_json::to_string_pretty(&out).unwrap_or_default(), false)
            }
            _ => (format!("Unknown tool: {name}"), true),
        }
    }

    fn execute_netlist(
        &mut self,
        lines: &[String],
        search_paths: &[std::path::PathBuf],
    ) -> (String, bool) {
        let settings = AppSettings::load();
        let res = run_simulation_with_settings(lines, search_paths, &settings.ngspice_executable);
        if !res.success {
            let err = res.error.clone().unwrap_or_else(|| "Simulation failed".into());
            let payload = json!({
                "success": false,
                "error": err,
                "log": res.log
            });
            let mut st = self.shared.lock().unwrap();
            st.last_sim = Some(res);
            st.revision += 1;
            return (serde_json::to_string_pretty(&payload).unwrap_or_default(), true);
        }

        let mut trace_summaries = serde_json::Map::new();
        let mut keys: Vec<&String> = res.traces.keys().collect();
        keys.sort();
        for k in keys {
            let tr = &res.traces[k];
            let min_v = tr.real.iter().copied().fold(f64::INFINITY, f64::min);
            let max_v = tr.real.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let last_v = tr.real.last().copied().unwrap_or(0.0);
            trace_summaries.insert(
                k.clone(),
                json!({ "min": min_v, "max": max_v, "final": last_v }),
            );
        }

        let payload = json!({
            "success": true,
            "plot_name": res.plot_name,
            "x_name": res.x_name,
            "num_points": res.x_values.len(),
            "traces": trace_summaries,
            "log_tail": res.log.iter().rev().take(10).rev().cloned().collect::<Vec<_>>()
        });
        let mut st = self.shared.lock().unwrap();
        st.last_sim = Some(res);
        st.revision += 1;
        (serde_json::to_string_pretty(&payload).unwrap_or_default(), false)
    }
}

fn is_analysis_directive(text: &str) -> bool {
    let low = text.trim().to_ascii_lowercase();
    low.starts_with(".tran")
        || low.starts_with(".ac")
        || low.starts_with(".dc")
        || low.starts_with(".noise")
        || low.starts_with(".tf")
        || low == ".op"
}

fn parse_component_kind(s: &str) -> Option<ComponentKind> {
    match s.trim().to_ascii_lowercase().as_str() {
        "v" | "voltage" | "voltagesource" => Some(ComponentKind::VoltageSource),
        "i" | "current" | "currentsource" => Some(ComponentKind::CurrentSource),
        "r" | "res" | "resistor" => Some(ComponentKind::Resistor),
        "c" | "cap" | "capacitor" => Some(ComponentKind::Capacitor),
        "l" | "ind" | "inductor" => Some(ComponentKind::Inductor),
        "d" | "diode" => Some(ComponentKind::Diode),
        "nmos" => Some(ComponentKind::Nmos),
        "pmos" => Some(ComponentKind::Pmos),
        "npn" => Some(ComponentKind::Npn),
        "pnp" => Some(ComponentKind::Pnp),
        "opamp" | "u" => Some(ComponentKind::OpAmp),
        "gnd" | "ground" | "0" => Some(ComponentKind::Ground),
        "net" | "netlabel" | "label" => Some(ComponentKind::NetLabel),
        _ => None,
    }
}

fn tool_definitions() -> Value {
    json!([
        {
            "name": "get_schematic",
            "description": "Get the current schematic components, wires, directives, and generated SPICE netlist.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "new_schematic",
            "description": "Create a new empty schematic or reset to the built-in demo circuit.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "load_demo": { "type": "boolean" }
                }
            }
        },
        {
            "name": "load_schematic",
            "description": "Load a schematic from an ASCII .asc or MCP_ITSpice .json file.",
            "inputSchema": {
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }
        },
        {
            "name": "save_schematic",
            "description": "Save the current schematic to an ASCII .asc or MCP_ITSpice .json file.",
            "inputSchema": {
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }
        },
        {
            "name": "add_component",
            "description": "Add a component to the schematic (automatically cuts wires between 2-pin pads).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "kind": { "type": "string", "description": "resistor, capacitor, inductor, diode, voltage, current, nmos, pmos, npn, pnp, opamp, ground, netlabel" },
                    "refdes": { "type": "string" },
                    "value": { "type": "string" },
                    "x": { "type": "integer" },
                    "y": { "type": "integer" },
                    "rotation": { "type": "integer", "enum": [0, 90, 180, 270] },
                    "mirror": { "type": "boolean" }
                },
                "required": ["kind", "x", "y"]
            }
        },
        {
            "name": "edit_component",
            "description": "Edit an existing component's value, refdes, position, or orientation.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "integer" },
                    "refdes": { "type": "string" },
                    "new_refdes": { "type": "string" },
                    "value": { "type": "string" },
                    "x": { "type": "integer" },
                    "y": { "type": "integer" },
                    "rotation": { "type": "integer" },
                    "mirror": { "type": "boolean" }
                }
            }
        },
        {
            "name": "remove_component",
            "description": "Remove a component by id or refdes.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "integer" },
                    "refdes": { "type": "string" }
                }
            }
        },
        {
            "name": "add_wire",
            "description": "Add an orthogonal wire between (x1, y1) and (x2, y2) in grid units.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "x1": { "type": "integer" },
                    "y1": { "type": "integer" },
                    "x2": { "type": "integer" },
                    "y2": { "type": "integer" }
                },
                "required": ["x1", "y1", "x2", "y2"]
            }
        },
        {
            "name": "set_directive",
            "description": "Set or add a SPICE directive (e.g. '.tran 10m', '.ac dec 10 1 1Meg', '.op').",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": { "type": "string" },
                    "x": { "type": "integer" },
                    "y": { "type": "integer" },
                    "replace_analysis": { "type": "boolean" }
                },
                "required": ["text"]
            }
        },
        {
            "name": "run_simulation",
            "description": "Compile the current schematic to a SPICE netlist and run it with ngspice.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "run_spice_netlist",
            "description": "Run a raw SPICE netlist directly in ngspice and return trace summaries.",
            "inputSchema": {
                "type": "object",
                "properties": { "netlist": { "type": "string" } },
                "required": ["netlist"]
            }
        },
        {
            "name": "get_trace",
            "description": "Fetch sampled [x, y] data points for a trace (e.g. 'V(n001)', 'I(v1)') from the last simulation.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "trace": { "type": "string" },
                    "max_points": { "type": "integer" }
                },
                "required": ["trace"]
            }
        }
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_init_list_and_simulate() {
        let mut srv = McpServer::new();
        let init = srv
            .handle_message(&json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {}
            }))
            .unwrap();
        assert_eq!(init["result"]["serverInfo"]["name"], "MCP_ITSpice");

        let list = srv
            .handle_message(&json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/list"
            }))
            .unwrap();
        assert!(list["result"]["tools"].as_array().unwrap().len() >= 10);

        let sim = srv
            .handle_message(&json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {
                    "name": "run_simulation",
                    "arguments": {}
                }
            }))
            .unwrap();
        assert_eq!(sim["result"]["isError"], false);
        let text = sim["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"success\": true"));
    }
}
