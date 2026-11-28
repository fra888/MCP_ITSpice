use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};

use crate::analysis_dialog::AnalysisDialogState;
use crate::capacitor_dialog::{CapacitorDialogAction, CapacitorDialogState};
use crate::hdl_dialog::{HdlDialogAction, HdlDialogState};
use crate::icons::{icon_button, ToolbarIcon};
use crate::mcp::McpSharedState;
use crate::model::{ComponentKind, NetlistResult, Schematic};
use crate::mosfet_dialog::MosfetDialogState;
use crate::ngspice::{run_simulation_with_settings, SimulationResult};
use crate::python_terminal::{draw_python_terminal, PythonTerminalState};
use crate::resistor_dialog::{ResistorDialogAction, ResistorDialogState};
use crate::schematic_ui::{draw_schematic_pane, EditorTool, SchematicAction, SchematicViewState};
use crate::settings::{detect_ngspice_path, detect_python_path, AppSettings, ModelIndex};
use crate::source_dialog::SourceDialogState;
use crate::symbol_editor::{CustomSymbol, SymbolEditorAction, SymbolEditorState};
use crate::waveform_ui::{draw_waveform_pane, WaveformViewState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayoutMode {
    SplitHorizontal,
    SplitVertical,
    SchematicOnly,
    WaveformOnly,
    PythonTerminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActivePane {
    Schematic,
    Waveform,
}

#[derive(Debug, Clone)]
enum EditModal {
    Component {
        id: u64,
        refdes: String,
        value: String,
        rotation: i32,
        mirror: bool,
    },
    Directive {
        id: Option<u64>,
        x: i32,
        y: i32,
        text: String,
    },
    NetLabel {
        x: i32,
        y: i32,
        text: String,
    },
}

pub struct ItSpiceApp {
    schematic: Schematic,
    current_file: Option<PathBuf>,
    netlist: NetlistResult,
    undo_stack: Vec<Schematic>,
    redo_stack: Vec<Schematic>,

    tool: EditorTool,
    schematic_view: SchematicViewState,
    waveform_view: WaveformViewState,
    layout_mode: LayoutMode,
    active_pane: ActivePane,
    split_ratio: f32,

    sim_result: Option<SimulationResult>,
    sim_rx: Option<Receiver<SimulationResult>>,
    sim_running: bool,
    sim_dirty: bool,
    ngspice_error: Option<String>,

    edit_modal: Option<EditModal>,
    source_dialog: Option<SourceDialogState>,
    capacitor_dialog: Option<CapacitorDialogState>,
    resistor_dialog: Option<ResistorDialogState>,
    mosfet_dialog: Option<MosfetDialogState>,
    analysis_dialog: Option<AnalysisDialogState>,
    show_component_picker: bool,
    show_netlist_window: bool,
    show_log_window: bool,
    show_add_trace_window: bool,
    show_settings_window: bool,
    python_terminal: PythonTerminalState,
    settings: AppSettings,
    model_index: ModelIndex,
    symbol_editor: SymbolEditorState,
    hdl_dialog: HdlDialogState,
    custom_symbols: Vec<CustomSymbol>,
    new_default_dir_input: String,
    new_user_dir_input: String,
    status_msg: String,
    first_frame: bool,
    mcp_shared: Option<Arc<Mutex<McpSharedState>>>,
    last_mcp_revision: u64,
}

impl ItSpiceApp {
    pub fn new(cc: &eframe::CreationContext<'_>, mcp_shared: Option<Arc<Mutex<McpSharedState>>>) -> Self {
        let _ = cc;
        let schematic = if let Some(shared) = &mcp_shared {
            let mut st = shared.lock().unwrap();
            st.egui_ctx = Some(cc.egui_ctx.clone());
            st.schematic.clone()
        } else {
            Schematic::default_ltspice_demo()
        };
        let settings = AppSettings::load();
        let model_index = ModelIndex::scan(&settings.all_model_dirs());
        let netlist = schematic.to_spice_netlist_with_models(Some(&model_index));
        let mut app = Self {
            schematic,
            current_file: None,
            netlist,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            tool: EditorTool::SelectOrProbe,
            schematic_view: SchematicViewState::default(),
            waveform_view: WaveformViewState::default(),
            layout_mode: LayoutMode::SplitHorizontal,
            active_pane: ActivePane::Schematic,
            split_ratio: 0.48,
            sim_result: None,
            sim_rx: None,
            sim_running: false,
            sim_dirty: false,
            ngspice_error: None,
            edit_modal: None,
            source_dialog: None,
            capacitor_dialog: None,
            resistor_dialog: None,
            mosfet_dialog: None,
            analysis_dialog: None,
            show_component_picker: false,
            show_netlist_window: false,
            show_log_window: false,
            show_add_trace_window: false,
            show_settings_window: false,
            python_terminal: {
                let mut pt = PythonTerminalState::default();
                pt.python_exe = settings.python_executable.clone();
                pt
            },
            settings,
            model_index,
            symbol_editor: SymbolEditorState::default(),
            hdl_dialog: HdlDialogState::default(),
            custom_symbols: Vec::new(),
            new_default_dir_input: String::new(),
            new_user_dir_input: String::new(),
            status_msg: if mcp_shared.is_some() {
                "Ready (MCP live sync active)".into()
            } else {
                "Ready".into()
            },
            first_frame: true,
            mcp_shared,
            last_mcp_revision: 0,
        };
        if app.mcp_shared.is_none() {
            app.run_simulation(true);
        }
        app
    }

    fn refresh_netlist(&mut self) {
        self.netlist = self
            .schematic
            .to_spice_netlist_with_models(Some(&self.model_index));
        self.sim_dirty = true;
    }

    fn run_simulation(&mut self, auto_select_demo_traces: bool) {
        if self.sim_running {
            return;
        }
        self.refresh_netlist();
        self.sim_dirty = false;
        let lines = self.netlist.lines.clone();
        let search_paths = self.settings.all_model_dirs();
        let ngspice_exe = self.settings.ngspice_executable.clone();
        let (tx, rx) = mpsc::channel();
        self.sim_rx = Some(rx);
        self.sim_running = true;
        self.status_msg = "Simulating with ngspice...".into();

        if auto_select_demo_traces && self.waveform_view.active_traces.is_empty() {
            // Pre-select interesting nodes (e.g. M1 gate & C2/R3 tank) if they exist
            let mut candidates: Vec<String> = self
                .netlist
                .point_to_net
                .values()
                .filter(|n| *n != "0" && *n != "v5")
                .map(|n| format!("V({n})"))
                .collect();
            candidates.sort();
            candidates.dedup();
            for t in candidates.into_iter().take(2) {
                self.waveform_view.active_traces.push(t);
            }
        }

        thread::spawn(move || {
            let result = run_simulation_with_settings(&lines, &search_paths, &ngspice_exe);
            let _ = tx.send(result);
        });
    }

    fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack.push(self.schematic.clone());
            self.schematic = prev;
            self.refresh_netlist();
        }
    }

    fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(self.schematic.clone());
            self.schematic = next;
            self.refresh_netlist();
        }
    }

    fn open_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("SPICE Schematic / MCP_ITSpice", &["asc", "json"])
            .pick_file()
        {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Schematic.asc");
                let sch = if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    serde_json::from_str(&content)
                        .unwrap_or_else(|_| Schematic::from_asc(name, &content))
                } else {
                    Schematic::from_asc(name, &content)
                };
                self.undo_stack.push(self.schematic.clone());
                self.schematic = sch;
                self.current_file = Some(path);
                self.refresh_netlist();
                self.sim_result = None;
                self.waveform_view.active_traces.clear();
                self.first_frame = true;
                self.status_msg = format!("Opened {}", self.schematic.name);
            }
        }
    }

    fn save_file(&mut self, save_as: bool) {
        let target = if save_as || self.current_file.is_none() {
            rfd::FileDialog::new()
                .set_file_name(&self.schematic.name)
                .add_filter("ASCII Schematic", &["asc"])
                .add_filter("MCP_ITSpice JSON", &["json"])
                .save_file()
        } else {
            self.current_file.clone()
        };

        if let Some(path) = target {
            let content = if path.extension().and_then(|e| e.to_str()) == Some("json") {
                serde_json::to_string_pretty(&self.schematic).unwrap_or_default()
            } else {
                self.schematic.to_asc()
            };
            if std::fs::write(&path, content).is_ok() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    self.schematic.name = name.to_string();
                }
                self.current_file = Some(path);
                self.status_msg = format!("Saved {}", self.schematic.name);
            }
        }
    }
}

impl eframe::App for ItSpiceApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Sync live updates from MCP server if running in --mcp GUI mode
        let mut mcp_pending_cmds: Vec<crate::python_terminal::PyPlotCommand> = Vec::new();
        if let Some(shared) = &self.mcp_shared {
            if let Ok(mut st) = shared.lock() {
                if st.revision != self.last_mcp_revision {
                    self.schematic = st.schematic.clone();
                    self.netlist = self.schematic.to_spice_netlist();
                    if let Some(sim) = &st.last_sim {
                        self.sim_result = Some(sim.clone());
                        if self.waveform_view.active_traces.is_empty() && sim.success {
                            let mut keys: Vec<String> = sim
                                .traces
                                .keys()
                                .filter(|k| k.starts_with("V(") && *k != "V(0)")
                                .cloned()
                                .collect();
                            keys.sort();
                            for k in keys.into_iter().take(2) {
                                self.waveform_view.active_traces.push(k);
                            }
                        }
                    } else {
                        self.sim_result = None;
                        self.waveform_view.active_traces.clear();
                    }
                    mcp_pending_cmds.extend(st.pending_plot_cmds.drain(..));
                    if st.zoom_fit_requested {
                        self.first_frame = true;
                        st.zoom_fit_requested = false;
                    }
                    self.last_mcp_revision = st.revision;
                    self.status_msg = format!("MCP updated (rev {})", st.revision);
                }
            }
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        for cmd in mcp_pending_cmds {
            self.apply_py_plot_cmd(cmd);
        }

        // Global keyboard shortcuts (when not typing in a text field)
        if !ctx.wants_keyboard_input() {
            ctx.input(|i| {
                let no_mod = !i.modifiers.command && !i.modifiers.ctrl && !i.modifiers.alt;
                if i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::Z) {
                    self.undo();
                } else if (i.modifiers.command && i.key_pressed(egui::Key::Y))
                    || (i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::Z))
                {
                    self.redo();
                } else if no_mod && i.key_pressed(egui::Key::W) {
                    self.tool = EditorTool::Wire;
                } else if no_mod && i.key_pressed(egui::Key::G) {
                    self.tool = EditorTool::Place(ComponentKind::Ground);
                } else if no_mod && i.key_pressed(egui::Key::R) {
                    self.tool = EditorTool::Place(ComponentKind::Resistor);
                } else if no_mod && i.key_pressed(egui::Key::C) {
                    self.tool = EditorTool::Place(ComponentKind::Capacitor);
                } else if no_mod && i.key_pressed(egui::Key::L) {
                    self.tool = EditorTool::Place(ComponentKind::Inductor);
                } else if no_mod && i.key_pressed(egui::Key::D) {
                    self.tool = EditorTool::Place(ComponentKind::Diode);
                } else if no_mod && i.key_pressed(egui::Key::P) {
                    self.show_component_picker = true;
                } else if no_mod && i.key_pressed(egui::Key::N) {
                    self.tool = EditorTool::Place(ComponentKind::NetLabel);
                } else if no_mod && i.key_pressed(egui::Key::I) {
                    self.tool = EditorTool::ProbeCurrent;
                } else if no_mod && i.key_pressed(egui::Key::V) {
                    self.tool = EditorTool::SelectOrProbe;
                } else if no_mod && (i.key_pressed(egui::Key::F7) || i.key_pressed(egui::Key::M)) {
                    self.tool = EditorTool::Move;
                    if self.schematic_view.has_selection() {
                        let anchor = self.schematic_view.cursor_grid.or_else(|| {
                            self.schematic
                                .components
                                .iter()
                                .find(|c| self.schematic_view.selected_components.contains(&c.id))
                                .map(|c| (c.x, c.y))
                        });
                        self.schematic_view.moving_anchor = anchor;
                        self.schematic_view.moving_comp_id = None;
                    }
                } else if no_mod && (i.key_pressed(egui::Key::F8) || i.key_pressed(egui::Key::S)) {
                    self.tool = EditorTool::Drag;
                    if self.schematic_view.has_selection() {
                        let anchor = self.schematic_view.cursor_grid.or_else(|| {
                            self.schematic
                                .components
                                .iter()
                                .find(|c| self.schematic_view.selected_components.contains(&c.id))
                                .map(|c| (c.x, c.y))
                        });
                        self.schematic_view.moving_anchor = anchor;
                        self.schematic_view.moving_comp_id = None;
                    }
                }
            });
        }

        // Poll simulation completion
        if let Some(rx) = &self.sim_rx {
            if let Ok(res) = rx.try_recv() {
                self.sim_running = false;
                self.sim_rx = None;
                if res.success {
                    self.ngspice_error = None;
                    if self.waveform_view.active_traces.is_empty() {
                        let mut keys: Vec<String> = res
                            .traces
                            .keys()
                            .filter(|k| k.starts_with("V(") && *k != "V(0)")
                            .cloned()
                            .collect();
                        keys.sort();
                        for k in keys.into_iter().take(2) {
                            self.waveform_view.active_traces.push(k);
                        }
                    }
                    self.status_msg = format!(
                        "Simulation complete ({}, {} points)",
                        res.plot_name,
                        res.x_values.len()
                    );
                } else {
                    self.ngspice_error = res.error.clone();
                    self.status_msg = res
                        .error
                        .clone()
                        .unwrap_or_else(|| "Simulation failed".into());
                }
                self.sim_result = Some(res);
                if let Some(shared) = &self.mcp_shared {
                    if let Ok(mut st) = shared.lock() {
                        st.schematic = self.schematic.clone();
                        st.last_sim = self.sim_result.clone();
                        st.revision += 1;
                        self.last_mcp_revision = st.revision;
                    }
                }
                if self.python_terminal.is_running() {
                    self.python_terminal
                        .sync_sim_data(self.sim_result.as_ref(), &self.netlist);
                    self.python_terminal.send_command("reload()", false);
                }
            } else {
                ctx.request_repaint();
            }
        }

        // Top Menu Bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New Schematic").clicked() {
                        self.undo_stack.push(self.schematic.clone());
                        self.schematic = Schematic::new_empty("Draft1.asc");
                        self.current_file = None;
                        self.refresh_netlist();
                        self.sim_result = None;
                        self.waveform_view.active_traces.clear();
                        ui.close_menu();
                    }
                    if ui.button("New Symbol").clicked() {
                        self.symbol_editor.reset_new();
                        ui.close_menu();
                    }
                    if ui.button("Open...").clicked() {
                        self.open_file();
                        ui.close_menu();
                    }
                    if ui.button("Save").clicked() {
                        self.save_file(false);
                        ui.close_menu();
                    }
                    if ui.button("Save As...").clicked() {
                        self.save_file(true);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Load Demo Circuit").clicked() {
                        self.undo_stack.push(self.schematic.clone());
                        self.schematic = Schematic::default_ltspice_demo();
                        self.refresh_netlist();
                        self.waveform_view.active_traces.clear();
                        self.first_frame = true;
                        self.run_simulation(true);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Settings...").clicked() {
                        self.show_settings_window = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.menu_button("Edit", |ui| {
                    if ui.button("Undo").clicked() {
                        self.undo();
                        ui.close_menu();
                    }
                    if ui.button("Redo").clicked() {
                        self.redo();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Draw Wire (F3)").clicked() {
                        self.tool = EditorTool::Wire;
                        ui.close_menu();
                    }
                    if ui.button("Place Component (F2)").clicked() {
                        self.show_component_picker = true;
                        ui.close_menu();
                    }
                    if ui.button("SPICE Directive").clicked() {
                        self.tool = EditorTool::PlaceDirective;
                        ui.close_menu();
                    }
                });

                ui.menu_button("View", |ui| {
                    if ui.button("Zoom to Fit (Active Pane)").clicked() {
                        match self.active_pane {
                            ActivePane::Schematic => self.first_frame = true,
                            ActivePane::Waveform => self.waveform_view.reset_zoom(),
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("SPICE Netlist").clicked() {
                        self.refresh_netlist();
                        self.show_netlist_window = true;
                        ui.close_menu();
                    }
                    if ui.button("SPICE Error Log").clicked() {
                        self.show_log_window = true;
                        ui.close_menu();
                    }
                    if ui.button("Add Waveform Trace...").clicked() {
                        self.show_add_trace_window = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    let py_label = if self.layout_mode == LayoutMode::PythonTerminal {
                        "✓ Python Analysis Terminal"
                    } else {
                        "Python Analysis Terminal"
                    };
                    if ui.button(py_label).clicked() {
                        self.layout_mode = LayoutMode::PythonTerminal;
                        ui.close_menu();
                    }
                });

                ui.menu_button("Simulate", |ui| {
                    if ui.button("Run (ngspice)").clicked() {
                        self.run_simulation(false);
                        ui.close_menu();
                    }
                    if ui.button("Edit Simulation Cmd").clicked() {
                        self.open_analysis_dialog();
                        ui.close_menu();
                    }
                });

                ui.menu_button("Window", |ui| {
                    if ui.button("Tile Horizontally (Top / Bottom)").clicked() {
                        self.layout_mode = LayoutMode::SplitHorizontal;
                        ui.close_menu();
                    }
                    if ui.button("Tile Vertically (Left / Right)").clicked() {
                        self.layout_mode = LayoutMode::SplitVertical;
                        ui.close_menu();
                    }
                    if ui.button("Schematic Only").clicked() {
                        self.layout_mode = LayoutMode::SchematicOnly;
                        ui.close_menu();
                    }
                    if ui.button("Waveform Only").clicked() {
                        self.layout_mode = LayoutMode::WaveformOnly;
                        ui.close_menu();
                    }
                    if ui.button("Python Terminal").clicked() {
                        self.layout_mode = LayoutMode::PythonTerminal;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Increase Waveform Pane Size (+)").clicked() {
                        self.split_ratio = (self.split_ratio + 0.05).clamp(0.15, 0.85);
                        ui.close_menu();
                    }
                    if ui.button("Decrease Waveform Pane Size (-)").clicked() {
                        self.split_ratio = (self.split_ratio - 0.05).clamp(0.15, 0.85);
                        ui.close_menu();
                    }
                });

                ui.menu_button("Help", |ui| {
                    ui.label("MCP_ITSpice v0.1 - SPICE GUI for ngspice");
                    ui.label("Tools: W=Wire, G=Ground, R=Resistor, C=Capacitor, L=Inductor, D=Diode, P=Component, N=Net");
                    ui.label("Edit: Ctrl+Z=Undo, Ctrl+C/X/V=Copy/Cut/Paste, Ctrl+R=Rotate, E=Mirror, Esc=Cancel");
                    ui.label("Click wire/pin after simulation to probe voltage/current.");
                });
            });
        });

        // Classic LTspice Icon Toolbar
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(2.0, 2.0);

                if icon_button(ui, ToolbarIcon::New, "New Schematic", false, true) {
                    self.undo_stack.push(self.schematic.clone());
                    self.schematic = Schematic::new_empty("Draft1.asc");
                    self.current_file = None;
                    self.refresh_netlist();
                    self.sim_result = None;
                    self.waveform_view.active_traces.clear();
                }
                if icon_button(ui, ToolbarIcon::Open, "Open .asc / .json", false, true) {
                    self.open_file();
                }
                if icon_button(ui, ToolbarIcon::Save, "Save Schematic", false, true) {
                    self.save_file(false);
                }
                ui.separator();

                // Configure Analysis / Run / Halt
                if icon_button(
                    ui,
                    ToolbarIcon::ConfigureAnalysis,
                    "Configure Analysis (.tran / .ac / .dc / .op)",
                    self.analysis_dialog.is_some(),
                    true,
                ) {
                    self.open_analysis_dialog();
                }
                if icon_button(
                    ui,
                    ToolbarIcon::Run,
                    "Run ngspice Simulation",
                    false,
                    !self.sim_running,
                ) {
                    self.run_simulation(false);
                }
                if icon_button(
                    ui,
                    ToolbarIcon::Halt,
                    "Halt Simulation",
                    false,
                    self.sim_running,
                ) {
                    self.sim_running = false;
                }
                ui.separator();

                // Layout & Zoom
                if icon_button(
                    ui,
                    ToolbarIcon::Split,
                    "Split Horizontal (Top / Bottom)",
                    self.layout_mode == LayoutMode::SplitHorizontal,
                    true,
                ) {
                    self.layout_mode = LayoutMode::SplitHorizontal;
                }
                if icon_button(
                    ui,
                    ToolbarIcon::SplitVertical,
                    "Split Vertical (Left / Right)",
                    self.layout_mode == LayoutMode::SplitVertical,
                    true,
                ) {
                    self.layout_mode = LayoutMode::SplitVertical;
                }
                if icon_button(
                    ui,
                    ToolbarIcon::ZoomIn,
                    "Zoom In (Active Pane)",
                    false,
                    true,
                ) {
                    match self.active_pane {
                        ActivePane::Schematic => {
                            self.schematic_view.zoom = (self.schematic_view.zoom * 1.2).min(4.0);
                        }
                        ActivePane::Waveform => {
                            self.waveform_view.zoom_by(0.8, self.sim_result.as_ref());
                        }
                    }
                }
                if icon_button(
                    ui,
                    ToolbarIcon::ZoomOut,
                    "Zoom Out (Active Pane)",
                    false,
                    true,
                ) {
                    match self.active_pane {
                        ActivePane::Schematic => {
                            self.schematic_view.zoom = (self.schematic_view.zoom / 1.2).max(0.3);
                        }
                        ActivePane::Waveform => {
                            self.waveform_view.zoom_by(1.25, self.sim_result.as_ref());
                        }
                    }
                }
                if icon_button(
                    ui,
                    ToolbarIcon::ZoomFit,
                    "Zoom Full Extents (Active Pane)",
                    false,
                    true,
                ) {
                    match self.active_pane {
                        ActivePane::Schematic => self.first_frame = true,
                        ActivePane::Waveform => self.waveform_view.reset_zoom(),
                    }
                }
                ui.separator();

                // Drawing & Component Tools
                if icon_button(
                    ui,
                    ToolbarIcon::Probe,
                    "Select / Probe Voltage V(net)",
                    self.tool == EditorTool::SelectOrProbe,
                    true,
                ) {
                    self.tool = EditorTool::SelectOrProbe;
                }
                if icon_button(
                    ui,
                    ToolbarIcon::CurrentProbe,
                    "Probe Current I(comp)",
                    self.tool == EditorTool::ProbeCurrent,
                    true,
                ) {
                    self.tool = EditorTool::ProbeCurrent;
                }
                if icon_button(
                    ui,
                    ToolbarIcon::Wire,
                    "Draw Wire",
                    self.tool == EditorTool::Wire,
                    true,
                ) {
                    self.tool = EditorTool::Wire;
                }
                if icon_button(
                    ui,
                    ToolbarIcon::Ground,
                    "Place Ground (0)",
                    self.tool == EditorTool::Place(ComponentKind::Ground),
                    true,
                ) {
                    self.tool = EditorTool::Place(ComponentKind::Ground);
                }
                if icon_button(
                    ui,
                    ToolbarIcon::NetLabel,
                    "Place Net Label",
                    self.tool == EditorTool::Place(ComponentKind::NetLabel),
                    true,
                ) {
                    self.tool = EditorTool::Place(ComponentKind::NetLabel);
                }
                if icon_button(
                    ui,
                    ToolbarIcon::Resistor,
                    "Place Resistor",
                    self.tool == EditorTool::Place(ComponentKind::Resistor),
                    true,
                ) {
                    self.tool = EditorTool::Place(ComponentKind::Resistor);
                }
                if icon_button(
                    ui,
                    ToolbarIcon::Capacitor,
                    "Place Capacitor",
                    self.tool == EditorTool::Place(ComponentKind::Capacitor),
                    true,
                ) {
                    self.tool = EditorTool::Place(ComponentKind::Capacitor);
                }
                if icon_button(
                    ui,
                    ToolbarIcon::Inductor,
                    "Place Inductor",
                    self.tool == EditorTool::Place(ComponentKind::Inductor),
                    true,
                ) {
                    self.tool = EditorTool::Place(ComponentKind::Inductor);
                }
                if icon_button(
                    ui,
                    ToolbarIcon::Diode,
                    "Place Diode",
                    self.tool == EditorTool::Place(ComponentKind::Diode),
                    true,
                ) {
                    self.tool = EditorTool::Place(ComponentKind::Diode);
                }
                if icon_button(
                    ui,
                    ToolbarIcon::Component,
                    "Component Picker (V, OpAmp, MOSFET, BJT...)",
                    self.show_component_picker,
                    true,
                ) {
                    self.show_component_picker = true;
                }
                if ui
                    .selectable_label(self.hdl_dialog.open, "HDL")
                    .on_hover_text("Insert Verilog / VHDL Block")
                    .clicked()
                {
                    self.hdl_dialog.open_new();
                }
                if icon_button(
                    ui,
                    ToolbarIcon::Directive,
                    "Place SPICE Directive (.tran, .ac, .model...)",
                    self.tool == EditorTool::PlaceDirective,
                    true,
                ) {
                    self.tool = EditorTool::PlaceDirective;
                }
                ui.separator();

                if icon_button(
                    ui,
                    ToolbarIcon::Move,
                    "Move Component (M / F7)",
                    self.tool == EditorTool::Move,
                    true,
                ) {
                    self.tool = EditorTool::Move;
                }
                if icon_button(
                    ui,
                    ToolbarIcon::Drag,
                    "Stretch / Drag Component & Keep Wires (S / F8)",
                    self.tool == EditorTool::Drag,
                    true,
                ) {
                    self.tool = EditorTool::Drag;
                }
                if icon_button(
                    ui,
                    ToolbarIcon::Delete,
                    "Delete (Cut)",
                    self.tool == EditorTool::Delete,
                    true,
                ) {
                    self.tool = EditorTool::Delete;
                }
                if icon_button(ui, ToolbarIcon::Undo, "Undo", false, !self.undo_stack.is_empty()) {
                    self.undo();
                }
                if icon_button(ui, ToolbarIcon::Redo, "Redo", false, !self.redo_stack.is_empty()) {
                    self.redo();
                }
                if icon_button(
                    ui,
                    ToolbarIcon::AddTrace,
                    "Add Waveform Trace",
                    self.show_add_trace_window,
                    true,
                ) {
                    self.show_add_trace_window = true;
                }
            });
        });

        // Document Tab Strip
        egui::TopBottomPanel::top("doc_tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let sch_label = format!("📄 {}", self.schematic.name);
                let raw_name = self.schematic.name.replace(".asc", ".raw");
                let raw_label = format!("📈 {raw_name}");

                if ui
                    .selectable_label(
                        self.layout_mode == LayoutMode::SchematicOnly,
                        sch_label,
                    )
                    .clicked()
                {
                    self.layout_mode = LayoutMode::SchematicOnly;
                }
                if ui
                    .selectable_label(self.layout_mode == LayoutMode::WaveformOnly, raw_label)
                    .clicked()
                {
                    self.layout_mode = LayoutMode::WaveformOnly;
                }
                if ui
                    .selectable_label(
                        self.layout_mode == LayoutMode::SplitHorizontal,
                        "Split H",
                    )
                    .clicked()
                {
                    self.layout_mode = LayoutMode::SplitHorizontal;
                }
                if ui
                    .selectable_label(
                        self.layout_mode == LayoutMode::SplitVertical,
                        "Split V",
                    )
                    .clicked()
                {
                    self.layout_mode = LayoutMode::SplitVertical;
                }
                if ui
                    .selectable_label(
                        self.layout_mode == LayoutMode::PythonTerminal,
                        "🐍 Python",
                    )
                    .clicked()
                {
                    self.layout_mode = LayoutMode::PythonTerminal;
                }
                if matches!(
                    self.layout_mode,
                    LayoutMode::SplitHorizontal | LayoutMode::SplitVertical
                ) {
                    ui.separator();
                    ui.label("Pane Size:");
                    if ui
                        .small_button("-")
                        .on_hover_text("Decrease Waveform Pane Size")
                        .clicked()
                    {
                        self.split_ratio = (self.split_ratio - 0.05).clamp(0.15, 0.85);
                    }
                    ui.label(format!("{:.0}%", self.split_ratio * 100.0));
                    if ui
                        .small_button("+")
                        .on_hover_text("Increase Waveform Pane Size")
                        .clicked()
                    {
                        self.split_ratio = (self.split_ratio + 0.05).clamp(0.15, 0.85);
                    }
                }
            });
        });

        // Status Bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status_msg);
                ui.separator();
                if let Some((gx, gy)) = self.schematic_view.cursor_grid {
                    ui.label(format!("Grid: ({gx}, {gy})"));
                }
                let sel_count = self.schematic_view.selected_components.len()
                    + self.schematic_view.selected_wires.len()
                    + self.schematic_view.selected_directives.len();
                if sel_count > 0 {
                    ui.separator();
                    ui.label(format!("Selected: {sel_count} (Del=delete, Ctrl+R/E=rotate/mirror, Ctrl+C/X/V=copy/cut/paste)"));
                }
                if let Some(probe) = &self.schematic_view.hovered_probe {
                    ui.separator();
                    let color = if crate::waveform_ui::is_power_trace(probe) {
                        Color32::from_rgb(255, 140, 0)
                    } else if crate::waveform_ui::is_current_trace(probe) {
                        Color32::from_rgb(0, 120, 220)
                    } else {
                        Color32::RED
                    };
                    ui.label(RichText::new(format!("Click to probe {probe}")).color(color));
                } else if self.sim_dirty && !self.sim_running {
                    ui.separator();
                    ui.label(
                        RichText::new("Circuit modified — click Run (▶) to re-simulate and probe new nodes")
                            .color(Color32::from_rgb(180, 95, 0)),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let is_py = self.layout_mode == LayoutMode::PythonTerminal;
                    let btn_txt = if is_py { "Py Terminal (Tab)" } else { "Py Terminal" };
                    if ui.small_button(btn_txt).clicked() {
                        self.layout_mode = if is_py {
                            LayoutMode::SplitHorizontal
                        } else {
                            LayoutMode::PythonTerminal
                        };
                    }
                });
            });
        });

        // Main Central Area (Split or Single Pane)
        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                let total_rect = ui.available_rect_before_wrap();
                match self.layout_mode {
                    LayoutMode::PythonTerminal => {
                        let new_waves = egui::Frame::none()
                            .inner_margin(8.0)
                            .show(ui, |ui| {
                                draw_python_terminal(
                                    ui,
                                    &mut self.python_terminal,
                                    self.sim_result.as_ref(),
                                    &self.netlist,
                                )
                            })
                            .inner;
                        for cmd in new_waves {
                            self.apply_py_plot_cmd(cmd);
                        }
                        if let Some(shared) = &self.mcp_shared {
                            if let Ok(mut st) = shared.lock() {
                                st.last_sim = self.sim_result.clone();
                                st.gui_python_log = self
                                    .python_terminal
                                    .lines
                                    .iter()
                                    .rev()
                                    .take(200)
                                    .rev()
                                    .map(|(l, _)| l.clone())
                                    .collect();
                            }
                        }
                    }
                    LayoutMode::WaveformOnly => {
                        self.active_pane = ActivePane::Waveform;
                        draw_waveform_pane(
                            ui,
                            self.sim_result.as_ref(),
                            &mut self.waveform_view,
                        );
                    }
                    LayoutMode::SchematicOnly => {
                        self.active_pane = ActivePane::Schematic;
                        if self.first_frame {
                            self.schematic_view.zoom_to_fit(&self.schematic, total_rect);
                            self.first_frame = false;
                        }
                        self.handle_schematic_pane(ui);
                    }
                    LayoutMode::SplitHorizontal => {
                        let top_h = (total_rect.height() * self.split_ratio).max(120.0);
                        let splitter_h = 6.0;
                        let bot_h = (total_rect.height() - top_h - splitter_h).max(120.0);

                        // Top: Waveform Pane (Draft1.raw)
                        ui.allocate_ui(Vec2::new(total_rect.width(), top_h), |ui| {
                            if draw_window_header(
                                ui,
                                &self.schematic.name.replace(".asc", ".raw"),
                                self.active_pane == ActivePane::Waveform,
                            ) {
                                self.active_pane = ActivePane::Waveform;
                            }
                            if draw_waveform_pane(
                                ui,
                                self.sim_result.as_ref(),
                                &mut self.waveform_view,
                            ) {
                                self.active_pane = ActivePane::Waveform;
                            }
                        });

                        // Draggable Splitter
                        let (split_rect, split_resp) = ui.allocate_exact_size(
                            Vec2::new(total_rect.width(), splitter_h),
                            Sense::drag(),
                        );
                        ui.painter().rect_filled(
                            split_rect,
                            0.0,
                            Color32::from_rgb(180, 180, 180),
                        );
                        if split_resp.hovered() || split_resp.dragged() {
                            ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical);
                        }
                        if split_resp.dragged() {
                            let delta = split_resp.drag_delta().y / total_rect.height().max(1.0);
                            self.split_ratio = (self.split_ratio + delta).clamp(0.15, 0.85);
                        }

                        // Bottom: Schematic Pane (Draft1.asc)
                        ui.allocate_ui(Vec2::new(total_rect.width(), bot_h), |ui| {
                            if draw_window_header(
                                ui,
                                &self.schematic.name,
                                self.active_pane == ActivePane::Schematic,
                            ) {
                                self.active_pane = ActivePane::Schematic;
                            }
                            let sch_rect = ui.available_rect_before_wrap();
                            if ui.input(|i| i.pointer.any_pressed()) {
                                if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                                    if sch_rect.contains(pos) {
                                        self.active_pane = ActivePane::Schematic;
                                    }
                                }
                            }
                            if self.first_frame {
                                self.schematic_view.zoom_to_fit(&self.schematic, sch_rect);
                                self.first_frame = false;
                            }
                            self.handle_schematic_pane(ui);
                        });
                    }
                    LayoutMode::SplitVertical => {
                        let left_w = (total_rect.width() * self.split_ratio).max(160.0);
                        let splitter_w = 6.0;
                        let right_w = (total_rect.width() - left_w - splitter_w).max(160.0);

                        let left_rect = Rect::from_min_size(
                            total_rect.min,
                            Vec2::new(left_w, total_rect.height()),
                        );
                        let split_rect = Rect::from_min_size(
                            Pos2::new(total_rect.left() + left_w, total_rect.top()),
                            Vec2::new(splitter_w, total_rect.height()),
                        );
                        let right_rect = Rect::from_min_size(
                            Pos2::new(total_rect.left() + left_w + splitter_w, total_rect.top()),
                            Vec2::new(right_w, total_rect.height()),
                        );

                        // Left: Waveform Pane (Draft1.raw)
                        ui.allocate_new_ui(
                            egui::UiBuilder::new()
                                .max_rect(left_rect)
                                .layout(egui::Layout::top_down(egui::Align::Min)),
                            |ui| {
                                if draw_window_header(
                                    ui,
                                    &self.schematic.name.replace(".asc", ".raw"),
                                    self.active_pane == ActivePane::Waveform,
                                ) {
                                    self.active_pane = ActivePane::Waveform;
                                }
                                if draw_waveform_pane(
                                    ui,
                                    self.sim_result.as_ref(),
                                    &mut self.waveform_view,
                                ) {
                                    self.active_pane = ActivePane::Waveform;
                                }
                            },
                        );

                        // Draggable Vertical Splitter
                        let split_resp = ui.interact(
                            split_rect,
                            egui::Id::new("vertical_splitter"),
                            Sense::drag(),
                        );
                        ui.painter().rect_filled(
                            split_rect,
                            0.0,
                            Color32::from_rgb(180, 180, 180),
                        );
                        if split_resp.hovered() || split_resp.dragged() {
                            ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                        }
                        if split_resp.dragged() {
                            let delta = split_resp.drag_delta().x / total_rect.width().max(1.0);
                            self.split_ratio = (self.split_ratio + delta).clamp(0.15, 0.85);
                        }

                        // Right: Schematic Pane (Draft1.asc)
                        ui.allocate_new_ui(
                            egui::UiBuilder::new()
                                .max_rect(right_rect)
                                .layout(egui::Layout::top_down(egui::Align::Min)),
                            |ui| {
                                if draw_window_header(
                                    ui,
                                    &self.schematic.name,
                                    self.active_pane == ActivePane::Schematic,
                                ) {
                                    self.active_pane = ActivePane::Schematic;
                                }
                                let sch_rect = ui.available_rect_before_wrap();
                                if ui.input(|i| i.pointer.any_pressed()) {
                                    if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                                        if sch_rect.contains(pos) {
                                            self.active_pane = ActivePane::Schematic;
                                        }
                                    }
                                }
                                if self.first_frame {
                                    self.schematic_view.zoom_to_fit(&self.schematic, sch_rect);
                                    self.first_frame = false;
                                }
                                self.handle_schematic_pane(ui);
                            },
                        );
                    }
                }
            });

        // Modals & Auxiliary Windows
        self.draw_modals(ctx);
    }
}

impl ItSpiceApp {
    fn apply_py_plot_cmd(&mut self, cmd: crate::python_terminal::PyPlotCommand) {
        if cmd.new_graph {
            self.waveform_view.add_graph();
        }
        let target_graph = cmd.graph_idx.unwrap_or(self.waveform_view.focused_graph);
        let max_g = self.waveform_view.extra_graphs.len();
        let target_graph = target_graph.min(max_g);

        if cmd.x_values.is_some() || cmd.x_unit.is_some() || cmd.y_unit.is_some() {
            let x_vals = cmd.x_values.unwrap_or_else(|| {
                self.waveform_view
                    .graph_x_overrides
                    .get(&target_graph)
                    .map(|g| g.x_values.clone())
                    .unwrap_or_default()
            });
            let x_unit = cmd.x_unit.unwrap_or_else(|| {
                self.waveform_view
                    .graph_x_overrides
                    .get(&target_graph)
                    .map(|g| g.x_unit.clone())
                    .unwrap_or_else(|| "Hz".to_string())
            });
            let y_unit = cmd.y_unit.or_else(|| {
                self.waveform_view
                    .graph_x_overrides
                    .get(&target_graph)
                    .and_then(|g| g.y_unit.clone())
            });
            self.waveform_view
                .set_graph_xaxis(target_graph, x_vals, x_unit, y_unit);
        }

        if cmd.name.is_empty() {
            self.status_msg = format!("Updated waveform graph {target_graph} from Python");
            return;
        }
        let w_name = cmd.name;
        let w_vals = cmd.y;
        if let Some(sim) = self.sim_result.as_mut() {
            sim.traces.insert(
                w_name.clone(),
                crate::ngspice::TraceData {
                    name: w_name.clone(),
                    real: w_vals,
                    imag: None,
                },
            );
        } else {
            let mut traces = std::collections::HashMap::new();
            let n = w_vals.len();
            traces.insert(
                w_name.clone(),
                crate::ngspice::TraceData {
                    name: w_name.clone(),
                    real: w_vals,
                    imag: None,
                },
            );
            self.sim_result = Some(SimulationResult {
                plot_name: "python".to_string(),
                x_name: "index".to_string(),
                x_values: (0..n).map(|i| i as f64).collect(),
                traces,
                log: Vec::new(),
                success: true,
                error: None,
            });
        }
        let target = if target_graph > 0 && target_graph <= self.waveform_view.extra_graphs.len() {
            &mut self.waveform_view.extra_graphs[target_graph - 1]
        } else {
            &mut self.waveform_view.active_traces
        };
        if !target.contains(&w_name) {
            target.push(w_name.clone());
        }
        self.status_msg = format!("Added waveform '{w_name}' to graph {target_graph}");
    }

    fn handle_schematic_pane(&mut self, ui: &mut Ui) {
        let action = draw_schematic_pane(
            ui,
            &mut self.schematic,
            &mut self.schematic_view,
            &mut self.tool,
            &self.netlist,
            self.sim_result.as_ref(),
            &mut self.undo_stack,
        );
        match action {
            SchematicAction::None => {}
            SchematicAction::Modified => {
                self.redo_stack.clear();
                self.refresh_netlist();
            }
            SchematicAction::ProbeTrace(name) => {
                self.waveform_view.toggle_trace(&name);
                self.status_msg = format!("Probed {name}");
            }
            SchematicAction::ProbePower(cid) => {
                if let Some(sim) = self.sim_result.as_mut() {
                    if let Some(p_name) =
                        compute_component_power_trace(cid, &self.schematic, &self.netlist, sim)
                    {
                        self.waveform_view.toggle_trace(&p_name);
                        self.status_msg = format!("Probed {p_name}");
                    }
                }
            }
            SchematicAction::EditComponent(id) => {
                if let Some(c) = self.schematic.components.iter().find(|c| c.id == id) {
                    if matches!(
                        c.kind,
                        ComponentKind::VoltageSource | ComponentKind::CurrentSource
                    ) {
                        self.source_dialog = Some(SourceDialogState::from_component(
                            id,
                            &c.refdes,
                            &c.value,
                            c.kind == ComponentKind::CurrentSource,
                        ));
                    } else if c.kind == ComponentKind::Capacitor {
                        self.capacitor_dialog = Some(CapacitorDialogState::from_component(
                            id,
                            &c.refdes,
                            &c.value,
                            &c.params,
                        ));
                    } else if c.kind == ComponentKind::Resistor {
                        self.resistor_dialog = Some(ResistorDialogState::from_component(
                            id,
                            &c.refdes,
                            &c.value,
                            &c.params,
                        ));
                    } else if matches!(c.kind, ComponentKind::Nmos | ComponentKind::Pmos) {
                        self.mosfet_dialog = Some(MosfetDialogState::from_component(
                            id,
                            &c.refdes,
                            &c.value,
                            c.kind == ComponentKind::Pmos,
                            &c.params,
                        ));
                    } else if let Some(hdl_blk) = &c.hdl {
                        self.hdl_dialog.open_existing(id, &c.refdes, hdl_blk);
                    } else {
                        self.edit_modal = Some(EditModal::Component {
                            id,
                            refdes: c.refdes.clone(),
                            value: c.value.clone(),
                            rotation: c.rotation,
                            mirror: c.mirror,
                        });
                    }
                }
            }
            SchematicAction::EditDirective(id) => {
                if let Some(d) = self.schematic.directives.iter().find(|d| d.id == id) {
                    let low = d.text.trim().to_ascii_lowercase();
                    if low.starts_with(".tran")
                        || low.starts_with(".ac")
                        || low.starts_with(".dc")
                        || low.starts_with(".noise")
                        || low.starts_with(".tf")
                        || low == ".op"
                    {
                        self.analysis_dialog = Some(AnalysisDialogState::from_directive(
                            Some(id),
                            d.x,
                            d.y,
                            &d.text,
                        ));
                    } else {
                        self.edit_modal = Some(EditModal::Directive {
                            id: Some(id),
                            x: d.x,
                            y: d.y,
                            text: d.text.clone(),
                        });
                    }
                }
            }
            SchematicAction::RequestNewDirective(x, y) => {
                self.edit_modal = Some(EditModal::Directive {
                    id: None,
                    x,
                    y,
                    text: ".tran 10m".into(),
                });
            }
            SchematicAction::RequestNewNetLabel(x, y) => {
                self.edit_modal = Some(EditModal::NetLabel {
                    x,
                    y,
                    text: "out".into(),
                });
            }
        }
    }

    fn open_analysis_dialog(&mut self) {
        let existing = self.schematic.directives.iter().find(|d| {
            let low = d.text.trim().to_ascii_lowercase();
            low.starts_with(".tran")
                || low.starts_with(".ac")
                || low.starts_with(".dc")
                || low.starts_with(".noise")
                || low.starts_with(".tf")
                || low == ".op"
        });
        if let Some(d) = existing {
            self.analysis_dialog = Some(AnalysisDialogState::from_directive(
                Some(d.id),
                d.x,
                d.y,
                &d.text,
            ));
        } else {
            self.analysis_dialog = Some(AnalysisDialogState::from_directive(
                None,
                8,
                24,
                ".tran 20",
            ));
        }
    }

    fn draw_modals(&mut self, ctx: &egui::Context) {
        // Configure Analysis Dialog
        if let Some(mut dlg) = self.analysis_dialog.take() {
            match dlg.show(ctx) {
                Some(true) => {
                    self.undo_stack.push(self.schematic.clone());
                    let cmd = dlg.cmd_text.trim().to_string();
                    if let Some(did) = dlg.directive_id {
                        if let Some(d) = self
                            .schematic
                            .directives
                            .iter_mut()
                            .find(|d| d.id == did)
                        {
                            d.text = cmd;
                        }
                    } else {
                        self.schematic.add_directive(dlg.x, dlg.y, cmd);
                    }
                    self.refresh_netlist();
                }
                Some(false) => {}
                None => {
                    self.analysis_dialog = Some(dlg);
                }
            }
        }

        // Independent Voltage / Current Source Dialog
        if let Some(mut dlg) = self.source_dialog.take() {
            match dlg.show(ctx) {
                Some(true) => {
                    self.undo_stack.push(self.schematic.clone());
                    let new_val = dlg.build_value();
                    if let Some(c) = self
                        .schematic
                        .components
                        .iter_mut()
                        .find(|c| c.id == dlg.comp_id)
                    {
                        c.value = new_val;
                    }
                    self.refresh_netlist();
                }
                Some(false) => {}
                None => {
                    self.source_dialog = Some(dlg);
                }
            }
        }

        // Capacitor Properties Dialog
        if let Some(mut dlg) = self.capacitor_dialog.take() {
            let user_dir = self
                .settings
                .user_model_dirs
                .first()
                .map(std::path::PathBuf::from);
            match dlg.show(ctx, &self.model_index, user_dir.as_ref()) {
                CapacitorDialogAction::Ok => {
                    self.undo_stack.push(self.schematic.clone());
                    let new_cap = dlg.capacitance.trim().to_string();
                    let new_params = dlg.into_params();
                    if let Some(c) = self
                        .schematic
                        .components
                        .iter_mut()
                        .find(|c| c.id == dlg.comp_id)
                    {
                        if !new_cap.is_empty() {
                            c.value = new_cap;
                        }
                        c.params = new_params;
                    }
                    self.refresh_netlist();
                }
                CapacitorDialogAction::Cancel => {}
                CapacitorDialogAction::SavedToLibrary => {
                    self.model_index = ModelIndex::scan(&self.settings.all_model_dirs());
                    self.capacitor_dialog = Some(dlg);
                }
                CapacitorDialogAction::None => {
                    self.capacitor_dialog = Some(dlg);
                }
            }
        }

        // Resistor Properties Dialog
        if let Some(mut dlg) = self.resistor_dialog.take() {
            let user_dir = self
                .settings
                .user_model_dirs
                .first()
                .map(std::path::PathBuf::from);
            match dlg.show(ctx, &self.model_index, user_dir.as_ref()) {
                ResistorDialogAction::Ok => {
                    self.undo_stack.push(self.schematic.clone());
                    let new_res = dlg.resistance.trim().to_string();
                    let new_params = dlg.into_params();
                    if let Some(c) = self
                        .schematic
                        .components
                        .iter_mut()
                        .find(|c| c.id == dlg.comp_id)
                    {
                        if !new_res.is_empty() {
                            c.value = new_res;
                        }
                        c.params = new_params;
                    }
                    self.refresh_netlist();
                }
                ResistorDialogAction::Cancel => {}
                ResistorDialogAction::SavedToLibrary => {
                    self.model_index = ModelIndex::scan(&self.settings.all_model_dirs());
                    self.resistor_dialog = Some(dlg);
                }
                ResistorDialogAction::None => {
                    self.resistor_dialog = Some(dlg);
                }
            }
        }

        // MOSFET Properties Dialog
        if let Some(mut dlg) = self.mosfet_dialog.take() {
            match dlg.show(ctx, &self.model_index) {
                Some(true) => {
                    self.undo_stack.push(self.schematic.clone());
                    let new_model = dlg.model_name.trim().to_string();
                    let is_pmos = dlg.is_pmos();
                    let new_params = dlg.into_params();
                    if let Some(c) = self
                        .schematic
                        .components
                        .iter_mut()
                        .find(|c| c.id == dlg.comp_id)
                    {
                        if !new_model.is_empty() {
                            c.value = new_model;
                        }
                        c.kind = if is_pmos {
                            ComponentKind::Pmos
                        } else {
                            ComponentKind::Nmos
                        };
                        c.params = new_params;
                    }
                    self.refresh_netlist();
                }
                Some(false) => {}
                None => {
                    self.mosfet_dialog = Some(dlg);
                }
            }
        }

        // Edit Component / Directive / NetLabel modal
        if let Some(mut modal) = self.edit_modal.take() {
            let mut keep_open = true;
            let title = match &modal {
                EditModal::Component { .. } => "Edit Component",
                EditModal::Directive { .. } => "Edit SPICE Directive",
                EditModal::NetLabel { .. } => "Place Net Label",
            };
            egui::Window::new(title)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| match &mut modal {
                    EditModal::Component {
                        id,
                        refdes,
                        value,
                        rotation,
                        mirror,
                    } => {
                        ui.horizontal(|ui| {
                            ui.label("RefDes:");
                            ui.text_edit_singleline(refdes);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Value / Model:");
                            ui.text_edit_singleline(value);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Rotation:");
                            for deg in [0, 90, 180, 270] {
                                ui.selectable_value(rotation, deg, format!("{deg}°"));
                            }
                            ui.checkbox(mirror, "Mirror");
                        });
                        ui.horizontal(|ui| {
                            if ui.button("OK").clicked() {
                                self.undo_stack.push(self.schematic.clone());
                                if let Some(c) =
                                    self.schematic.components.iter_mut().find(|c| c.id == *id)
                                {
                                    c.refdes = refdes.clone();
                                    c.value = value.clone();
                                    c.rotation = *rotation;
                                    c.mirror = *mirror;
                                }
                                self.refresh_netlist();
                                keep_open = false;
                            }
                            if ui.button("Cancel").clicked() {
                                keep_open = false;
                            }
                        });
                    }
                    EditModal::Directive { id, x, y, text } => {
                        ui.label("SPICE Directive (e.g. .tran 20, .ac dec 10 1 1Meg, .op):");
                        ui.text_edit_multiline(text);
                        ui.horizontal(|ui| {
                            if ui.button("OK").clicked() {
                                self.undo_stack.push(self.schematic.clone());
                                if let Some(did) = id {
                                    if let Some(d) =
                                        self.schematic.directives.iter_mut().find(|d| d.id == *did)
                                    {
                                        d.text = text.clone();
                                    }
                                } else {
                                    self.schematic.add_directive(*x, *y, text.clone());
                                }
                                self.refresh_netlist();
                                keep_open = false;
                            }
                            if ui.button("Cancel").clicked() {
                                keep_open = false;
                            }
                        });
                    }
                    EditModal::NetLabel { x, y, text } => {
                        ui.horizontal(|ui| {
                            ui.label("Net Name:");
                            ui.text_edit_singleline(text);
                        });
                        ui.horizontal(|ui| {
                            if ui.button("OK").clicked() {
                                self.undo_stack.push(self.schematic.clone());
                                self.schematic.add_component(
                                    ComponentKind::NetLabel,
                                    String::new(),
                                    text.clone(),
                                    *x,
                                    *y,
                                );
                                self.refresh_netlist();
                                keep_open = false;
                            }
                            if ui.button("Cancel").clicked() {
                                keep_open = false;
                            }
                        });
                    }
                });
            if keep_open {
                self.edit_modal = Some(modal);
            }
        }

        // Component Picker
        if self.show_component_picker {
            let mut open = true;
            egui::Window::new("Select Component Symbol")
                .open(&mut open)
                .resizable(false)
                .show(ctx, |ui| {
                    let all_kinds = [
                        ComponentKind::Resistor,
                        ComponentKind::Capacitor,
                        ComponentKind::Inductor,
                        ComponentKind::Diode,
                        ComponentKind::VoltageSource,
                        ComponentKind::CurrentSource,
                        ComponentKind::OpAmp,
                        ComponentKind::Nmos,
                        ComponentKind::Pmos,
                        ComponentKind::Npn,
                        ComponentKind::Pnp,
                        ComponentKind::Ground,
                        ComponentKind::NetLabel,
                    ];
                    for kind in all_kinds {
                        if ui.button(kind.display_name()).clicked() {
                            self.tool = EditorTool::Place(kind);
                            self.show_component_picker = false;
                        }
                    }
                    ui.separator();
                    if ui.button("Verilog / VHDL Block...").clicked() {
                        self.hdl_dialog.open_new();
                        self.show_component_picker = false;
                    }
                    if !self.custom_symbols.is_empty() {
                        ui.separator();
                        ui.strong("Custom Symbols");
                        for sym in self.custom_symbols.clone() {
                            if ui.button(&sym.name).clicked() {
                                self.schematic_view.active_custom_symbol = Some(sym);
                                self.schematic_view.active_hdl_block = None;
                                self.tool = EditorTool::Place(ComponentKind::Custom);
                                self.show_component_picker = false;
                            }
                        }
                    }
                });
            if !open {
                self.show_component_picker = false;
            }
        }

        // SPICE Netlist Viewer
        if self.show_netlist_window {
            let mut open = true;
            egui::Window::new("SPICE Netlist")
                .open(&mut open)
                .default_size([460.0, 360.0])
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let txt = self.netlist.lines.join("\n");
                        ui.add(
                            egui::TextEdit::multiline(&mut txt.as_str())
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY),
                        );
                    });
                });
            if !open {
                self.show_netlist_window = false;
            }
        }

        // SPICE Error / Output Log
        if self.show_log_window {
            let mut open = true;
            egui::Window::new("ngspice Output Log")
                .open(&mut open)
                .default_size([520.0, 340.0])
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let txt = self
                            .sim_result
                            .as_ref()
                            .map(|r| r.log.join("\n"))
                            .unwrap_or_else(|| "No simulation run yet.".into());
                        ui.add(
                            egui::TextEdit::multiline(&mut txt.as_str())
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY),
                        );
                    });
                });
            if !open {
                self.show_log_window = false;
            }
        }

        // Symbol Workbench Window
        match self.symbol_editor.show(ctx) {
            SymbolEditorAction::None => {}
            SymbolEditorAction::SaveAndUse(sym) => {
                if let Some(existing) = self
                    .custom_symbols
                    .iter_mut()
                    .find(|s| s.name == sym.name)
                {
                    *existing = sym.clone();
                } else {
                    self.custom_symbols.push(sym.clone());
                }
                self.schematic_view.active_custom_symbol = Some(sym);
                self.schematic_view.active_hdl_block = None;
                self.tool = EditorTool::Place(ComponentKind::Custom);
                self.status_msg = "Click on schematic to place custom symbol".into();
            }
        }

        // Verilog / VHDL Block Editor Window
        let mut available_nets: Vec<String> = self
            .netlist
            .point_to_net
            .values()
            .filter(|n| *n != "0")
            .cloned()
            .collect();
        available_nets.sort();
        available_nets.dedup();
        match self.hdl_dialog.show(ctx, &available_nets) {
            HdlDialogAction::None => {}
            HdlDialogAction::PlaceNew(block) => {
                let sym = block.build_symbol();
                self.status_msg = format!(
                    "Click on schematic to place {} block '{}'",
                    block.language.label(),
                    block.module_name
                );
                self.schematic_view.active_custom_symbol = Some(sym);
                self.schematic_view.active_hdl_block = Some(block);
                self.tool = EditorTool::Place(ComponentKind::Custom);
            }
            HdlDialogAction::UpdateExisting {
                comp_id,
                refdes,
                block,
            } => {
                self.undo_stack.push(self.schematic.clone());
                if let Some(c) = self
                    .schematic
                    .components
                    .iter_mut()
                    .find(|c| c.id == comp_id)
                {
                    if !refdes.trim().is_empty() {
                        c.refdes = refdes.trim().to_string();
                    }
                    c.value = block.module_name.clone();
                    c.custom_symbol = Some(block.build_symbol());
                    c.hdl = Some(block);
                }
                self.refresh_netlist();
                self.status_msg = "Updated HDL block — click Run (▶) to re-simulate".into();
            }
        }

        // File -> Settings (ngspice Executable & Model Search Folders)
        if self.show_settings_window {
            let mut open = true;
            let mut close_requested = false;
            egui::Window::new("Settings - ngspice & Model Folders")
                .open(&mut open)
                .default_width(540.0)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.heading("ngspice Executable / Library Path");
                    ui.label("Path to ngspice binary or shared library (libngspice.so.0 / ngspice.dll):");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.settings.ngspice_executable)
                                .desired_width(340.0),
                        );
                        if ui.button("Browse...").clicked() {
                            if let Some(file) = rfd::FileDialog::new().pick_file() {
                                self.settings.ngspice_executable = file.display().to_string();
                            }
                        }
                        if ui.button("Auto-Detect").clicked() {
                            self.settings.ngspice_executable = detect_ngspice_path();
                        }
                    });

                    ui.add_space(10.0);
                    ui.separator();
                    ui.heading("Python Executable Path");
                    ui.label("Path to python binary or virtualenv python (e.g. /path/to/venv/bin/python or C:\\...\\python.exe):");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.settings.python_executable)
                                .desired_width(340.0),
                        );
                        if ui.button("Browse...").clicked() {
                            if let Some(file) = rfd::FileDialog::new().pick_file() {
                                self.settings.python_executable = file.display().to_string();
                            }
                        }
                        if ui.button("Auto-Detect").clicked() {
                            self.settings.python_executable = detect_python_path();
                        }
                    });

                    ui.add_space(10.0);
                    ui.separator();
                    ui.heading("ngspice Default Model Folders");
                    ui.label("Built-in / system ngspice library directories:");
                    let mut remove_def: Option<usize> = None;
                    for (idx, dir) in self.settings.ngspice_default_dirs.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(dir).desired_width(380.0));
                            if ui.button("Remove").clicked() {
                                remove_def = Some(idx);
                            }
                        });
                    }
                    if let Some(idx) = remove_def {
                        self.settings.ngspice_default_dirs.remove(idx);
                    }
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.new_default_dir_input)
                                .hint_text("/path/to/ngspice/lib")
                                .desired_width(300.0),
                        );
                        if ui.button("Add").clicked() && !self.new_default_dir_input.trim().is_empty()
                        {
                            self.settings
                                .ngspice_default_dirs
                                .push(self.new_default_dir_input.trim().to_string());
                            self.new_default_dir_input.clear();
                        }
                        if ui.button("Browse...").clicked() {
                            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                                self.settings
                                    .ngspice_default_dirs
                                    .push(folder.display().to_string());
                            }
                        }
                    });

                    ui.add_space(10.0);
                    ui.separator();
                    ui.heading("User Model Folders");
                    ui.label("Custom .lib / .mod / .subckt directories (searched first):");
                    let mut remove_usr: Option<usize> = None;
                    for (idx, dir) in self.settings.user_model_dirs.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(dir).desired_width(380.0));
                            if ui.button("Remove").clicked() {
                                remove_usr = Some(idx);
                            }
                        });
                    }
                    if let Some(idx) = remove_usr {
                        self.settings.user_model_dirs.remove(idx);
                    }
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.new_user_dir_input)
                                .hint_text("/path/to/user/models")
                                .desired_width(300.0),
                        );
                        if ui.button("Add").clicked() && !self.new_user_dir_input.trim().is_empty()
                        {
                            self.settings
                                .user_model_dirs
                                .push(self.new_user_dir_input.trim().to_string());
                            self.new_user_dir_input.clear();
                        }
                        if ui.button("Browse...").clicked() {
                            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                                self.settings
                                    .user_model_dirs
                                    .push(folder.display().to_string());
                            }
                        }
                    });

                    ui.add_space(8.0);
                    ui.separator();
                    ui.label(format!(
                        "Indexed {} model file(s): {} .model, {} .subckt",
                        self.model_index.scanned_files,
                        self.model_index.models.len(),
                        self.model_index.subckts.len()
                    ));

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("Rescan Models").clicked() {
                            self.model_index =
                                ModelIndex::scan(&self.settings.all_model_dirs());
                            self.refresh_netlist();
                        }
                        if ui.button("Restore Defaults").clicked() {
                            self.settings = AppSettings::default();
                            self.python_terminal.python_exe =
                                self.settings.python_executable.clone();
                            self.python_terminal.stop();
                            self.model_index =
                                ModelIndex::scan(&self.settings.all_model_dirs());
                            self.refresh_netlist();
                        }
                        if ui.button("Save & Close").clicked() {
                            if self.python_terminal.python_exe != self.settings.python_executable {
                                self.python_terminal.python_exe =
                                    self.settings.python_executable.clone();
                                self.python_terminal.stop();
                            }
                            self.model_index =
                                ModelIndex::scan(&self.settings.all_model_dirs());
                            if let Err(e) = self.settings.save() {
                                self.status_msg = format!("Settings save error: {e}");
                            } else {
                                self.status_msg = format!(
                                    "Saved settings ({} models, {} subckts)",
                                    self.model_index.models.len(),
                                    self.model_index.subckts.len()
                                );
                            }
                            self.refresh_netlist();
                            close_requested = true;
                        }
                    });
                });
            if !open || close_requested {
                self.show_settings_window = false;
            }
        }

        // Add Trace Picker
        if self.show_add_trace_window {
            let mut open = true;
            egui::Window::new("Add Traces to Plot")
                .open(&mut open)
                .default_size([260.0, 320.0])
                .show(ctx, |ui| {
                    if let Some(sim) = &self.sim_result {
                        let mut names: Vec<String> = sim
                            .traces
                            .keys()
                            .filter(|k| *k != "time" && *k != "frequency")
                            .cloned()
                            .collect();
                        names.sort();
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for name in names {
                                let active = self.waveform_view.active_traces.contains(&name);
                                if ui.selectable_label(active, &name).clicked() {
                                    self.waveform_view.toggle_trace(&name);
                                }
                            }
                        });
                    } else {
                        ui.label("Run a simulation first.");
                    }
                });
            if !open {
                self.show_add_trace_window = false;
            }
        }
    }
}

fn compute_component_power_trace(
    cid: u64,
    schematic: &Schematic,
    netlist: &crate::model::NetlistResult,
    sim: &mut SimulationResult,
) -> Option<String> {
    let comp = schematic.components.iter().find(|c| c.id == cid)?;
    let spice_name = netlist.comp_spice_names.get(&cid)?;
    let clean_low = spice_name.trim_start_matches(['X', 'x']).to_lowercase();
    let p_name = format!("P({clean_low})");

    let pins = crate::model::component_pins_world(comp);
    let pin_nets: Vec<String> = pins
        .iter()
        .map(|(_, px, py)| {
            netlist
                .point_to_net
                .get(&(*px, *py))
                .cloned()
                .unwrap_or_else(|| "0".to_string())
        })
        .collect();

    let n_pts = sim.x_values.len();
    let net_v = |net: &str, idx: usize| -> f64 {
        if net == "0" {
            0.0
        } else {
            sim.traces
                .get(&format!("V({net})"))
                .and_then(|t| t.real.get(idx))
                .copied()
                .unwrap_or(0.0)
        }
    };

    let power_vals: Vec<f64> = match comp.kind {
        ComponentKind::Nmos | ComponentKind::Pmos => {
            let id_tr = sim.traces.get(&format!("Id({clean_low})"))?;
            let ig_tr = sim.traces.get(&format!("Ig({clean_low})"));
            let is_tr = sim.traces.get(&format!("Is({clean_low})"));
            (0..n_pts)
                .map(|k| {
                    let vd = net_v(pin_nets.first().map(|s| s.as_str()).unwrap_or("0"), k);
                    let vg = net_v(pin_nets.get(1).map(|s| s.as_str()).unwrap_or("0"), k);
                    let vs = net_v(pin_nets.get(2).map(|s| s.as_str()).unwrap_or("0"), k);
                    let id = id_tr.real.get(k).copied().unwrap_or(0.0);
                    let ig = ig_tr.and_then(|t| t.real.get(k)).copied().unwrap_or(0.0);
                    let is = is_tr.and_then(|t| t.real.get(k)).copied().unwrap_or(-id - ig);
                    vd * id + vg * ig + vs * is
                })
                .collect()
        }
        ComponentKind::Npn | ComponentKind::Pnp => {
            let ic_tr = sim.traces.get(&format!("Ic({clean_low})"))?;
            let ib_tr = sim.traces.get(&format!("Ib({clean_low})"));
            let ie_tr = sim.traces.get(&format!("Ie({clean_low})"));
            (0..n_pts)
                .map(|k| {
                    let vc = net_v(pin_nets.first().map(|s| s.as_str()).unwrap_or("0"), k);
                    let vb = net_v(pin_nets.get(1).map(|s| s.as_str()).unwrap_or("0"), k);
                    let ve = net_v(pin_nets.get(2).map(|s| s.as_str()).unwrap_or("0"), k);
                    let ic = ic_tr.real.get(k).copied().unwrap_or(0.0);
                    let ib = ib_tr.and_then(|t| t.real.get(k)).copied().unwrap_or(0.0);
                    let ie = ie_tr.and_then(|t| t.real.get(k)).copied().unwrap_or(-ic - ib);
                    vc * ic + vb * ib + ve * ie
                })
                .collect()
        }
        ComponentKind::OpAmp => {
            let tags = ["in+", "in-", "v+", "v-", "out"];
            let pin_trs: Vec<Option<&crate::ngspice::TraceData>> = tags
                .iter()
                .map(|tag| sim.traces.get(&format!("I({clean_low}:{tag})")))
                .collect();
            if pin_trs.iter().all(|t| t.is_none()) {
                return None;
            }
            (0..n_pts)
                .map(|k| {
                    pin_nets
                        .iter()
                        .zip(pin_trs.iter())
                        .map(|(net, tr_opt)| {
                            let v = net_v(net, k);
                            let i = tr_opt.and_then(|t| t.real.get(k)).copied().unwrap_or(0.0);
                            v * i
                        })
                        .sum()
                })
                .collect()
        }
        ComponentKind::Custom => {
            let pin_trs: Vec<Option<&crate::ngspice::TraceData>> = (1..=pin_nets.len())
                .map(|idx| sim.traces.get(&format!("I({clean_low}:{idx})")))
                .collect();
            if pin_trs.iter().all(|t| t.is_none()) {
                return None;
            }
            (0..n_pts)
                .map(|k| {
                    pin_nets
                        .iter()
                        .zip(pin_trs.iter())
                        .map(|(net, tr_opt)| {
                            let v = net_v(net, k);
                            let i = tr_opt.and_then(|t| t.real.get(k)).copied().unwrap_or(0.0);
                            v * i
                        })
                        .sum()
                })
                .collect()
        }
        _ => {
            let i_tr = sim
                .traces
                .get(&format!("I({clean_low})"))
                .or_else(|| sim.traces.get(&format!("I({})", spice_name.to_lowercase())))?;
            let n0 = pin_nets.first().map(|s| s.as_str()).unwrap_or("0");
            let n1 = pin_nets.get(1).map(|s| s.as_str()).unwrap_or("0");
            (0..n_pts)
                .map(|k| {
                    let v_diff = net_v(n0, k) - net_v(n1, k);
                    let i_val = i_tr.real.get(k).copied().unwrap_or(0.0);
                    v_diff * i_val
                })
                .collect()
        }
    };

    sim.traces.insert(
        p_name.clone(),
        crate::ngspice::TraceData {
            name: p_name.clone(),
            real: power_vals,
            imag: None,
        },
    );
    Some(p_name)
}

fn draw_window_header(ui: &mut Ui, title: &str, active: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 20.0),
        Sense::click(),
    );
    let painter = ui.painter_at(rect);
    let bg = if active {
        Color32::from_rgb(178, 206, 240)
    } else {
        Color32::from_rgb(220, 222, 226)
    };
    painter.rect_filled(rect, 0.0, bg);
    painter.line_segment(
        [Pos2::new(rect.left(), rect.bottom()), Pos2::new(rect.right(), rect.bottom())],
        Stroke::new(1.0, Color32::from_rgb(130, 130, 130)),
    );
    painter.text(
        Pos2::new(rect.left() + 8.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(12.0),
        Color32::BLACK,
    );
    resp.clicked()
}
