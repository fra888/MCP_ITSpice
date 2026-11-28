use std::collections::HashMap;

use egui::{Color32, FontId, Pos2, Rect, RichText, ScrollArea, Sense, Stroke, Ui, Vec2};

use crate::hdl::{
    has_verilator_cosim_tools, parse_hdl, verilog_templates, vhdl_templates, HdlBackend, HdlBlock,
    HdlLanguage, HdlPortConfig, HdlPortDir,
};
use crate::model::GRID_SIZE;
use crate::symbol_editor::{CustomSymbol, SymbolShape};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum HdlDialogTab {
    #[default]
    CodeAndInterface,
    Symbol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum HdlSymTool {
    #[default]
    MovePin,
    Line,
    Rect,
    Circle,
    Text,
    Delete,
}

#[derive(Default, Clone)]
struct PortEditRow {
    ref_net: String,
    v_il: String,
    v_ih: String,
    v_low: String,
    v_high: String,
}

pub struct HdlDialogState {
    pub open: bool,
    /// `Some(comp_id)` when editing an existing HDL component, `None` when placing a new one
    pub editing_comp_id: Option<u64>,
    pub refdes: String,
    pub block: HdlBlock,
    pub selected_template_idx: usize,
    pub v_low_str: String,
    pub v_high_str: String,
    pub v_il_str: String,
    pub v_ih_str: String,
    new_ref_pin_input: String,
    port_rows: HashMap<String, PortEditRow>,
    active_tab: HdlDialogTab,
    sym_tool: HdlSymTool,
    sym_drag_start: Option<(f32, f32)>,
    sym_dragging_pin: Option<usize>,
    sym_dragging_shape: Option<(usize, (f32, f32), SymbolShape)>,
    sym_next_text: String,
    sym_undo: Vec<CustomSymbol>,
    sym_zoom: f32,
}

impl Default for HdlDialogState {
    fn default() -> Self {
        let block = HdlBlock::default();
        Self {
            open: false,
            editing_comp_id: None,
            refdes: String::new(),
            v_low_str: format!("{}", block.v_low),
            v_high_str: format!("{}", block.v_high),
            v_il_str: format!("{}", block.v_thresh_low),
            v_ih_str: format!("{}", block.v_thresh_high),
            block,
            selected_template_idx: 0,
            new_ref_pin_input: String::new(),
            port_rows: HashMap::new(),
            active_tab: HdlDialogTab::CodeAndInterface,
            sym_tool: HdlSymTool::MovePin,
            sym_drag_start: None,
            sym_dragging_pin: None,
            sym_dragging_shape: None,
            sym_next_text: "LBL".into(),
            sym_undo: Vec::new(),
            sym_zoom: 1.8,
        }
    }
}

pub enum HdlDialogAction {
    None,
    PlaceNew(HdlBlock),
    UpdateExisting {
        comp_id: u64,
        refdes: String,
        block: HdlBlock,
    },
}

impl HdlDialogState {
    pub fn open_new(&mut self) {
        self.open = true;
        self.editing_comp_id = None;
        self.refdes.clear();
        self.block = HdlBlock::default();
        self.selected_template_idx = 0;
        self.active_tab = HdlDialogTab::CodeAndInterface;
        self.sym_undo.clear();
        self.sym_drag_start = None;
        self.sym_dragging_pin = None;
        self.sym_dragging_shape = None;
        self.sync_voltage_strings();
    }

    pub fn open_existing(&mut self, comp_id: u64, refdes: &str, block: &HdlBlock) {
        self.open = true;
        self.editing_comp_id = Some(comp_id);
        self.refdes = refdes.to_string();
        self.block = block.clone();
        self.active_tab = HdlDialogTab::CodeAndInterface;
        self.sym_undo.clear();
        self.sym_drag_start = None;
        self.sym_dragging_pin = None;
        self.sym_dragging_shape = None;
        self.sync_voltage_strings();
    }

    fn sync_voltage_strings(&mut self) {
        self.v_low_str = format!("{}", self.block.v_low);
        self.v_high_str = format!("{}", self.block.v_high);
        self.v_il_str = format!("{}", self.block.v_thresh_low);
        self.v_ih_str = format!("{}", self.block.v_thresh_high);
        self.port_rows.clear();
        for (name, cfg) in &self.block.port_configs {
            self.port_rows.insert(
                name.clone(),
                PortEditRow {
                    ref_net: cfg.ref_net.clone(),
                    v_il: cfg.v_thresh_low.map(|v| format!("{v}")).unwrap_or_default(),
                    v_ih: cfg.v_thresh_high.map(|v| format!("{v}")).unwrap_or_default(),
                    v_low: cfg.v_low.map(|v| format!("{v}")).unwrap_or_default(),
                    v_high: cfg.v_high.map(|v| format!("{v}")).unwrap_or_default(),
                },
            );
        }
    }

    fn apply_voltage_strings(&mut self) {
        if let Ok(v) = self.v_low_str.trim().parse::<f64>() {
            self.block.v_low = v;
        }
        if let Ok(v) = self.v_high_str.trim().parse::<f64>() {
            self.block.v_high = v;
        }
        if let Ok(v) = self.v_il_str.trim().parse::<f64>() {
            self.block.v_thresh_low = v;
        }
        if let Ok(v) = self.v_ih_str.trim().parse::<f64>() {
            self.block.v_thresh_high = v;
        }
        self.block.port_configs.clear();
        for (name, row) in &self.port_rows {
            let cfg = HdlPortConfig {
                ref_net: row.ref_net.trim().to_string(),
                v_thresh_low: row.v_il.trim().parse::<f64>().ok(),
                v_thresh_high: row.v_ih.trim().parse::<f64>().ok(),
                v_low: row.v_low.trim().parse::<f64>().ok(),
                v_high: row.v_high.trim().parse::<f64>().ok(),
            };
            if !cfg.is_default() {
                self.block.port_configs.insert(name.clone(), cfg);
            }
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, available_nets: &[String]) -> HdlDialogAction {
        if !self.open {
            return HdlDialogAction::None;
        }

        let mut action = HdlDialogAction::None;
        let mut open = self.open;
        let title = if self.editing_comp_id.is_some() {
            format!("Edit HDL Block ({})", self.refdes)
        } else {
            "Insert Verilog / VHDL Block".to_string()
        };

        egui::Window::new(title)
            .open(&mut open)
            .default_size([700.0, 600.0])
            .min_size([460.0, 380.0])
            .resizable(true)
            .show(ctx, |ui| {
                // Tab bar: Code & Interface | Symbol
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.active_tab,
                        HdlDialogTab::CodeAndInterface,
                        "Code & Interface",
                    );
                    let sym_tab_lbl = if self.block.custom_symbol_override.is_some() {
                        "Symbol (Custom)"
                    } else {
                        "Symbol"
                    };
                    if ui
                        .selectable_value(&mut self.active_tab, HdlDialogTab::Symbol, sym_tab_lbl)
                        .clicked()
                    {
                        self.apply_voltage_strings();
                        self.block.refresh_metadata();
                        if let Some(mut sym) = self.block.custom_symbol_override.take() {
                            self.block.sync_symbol_pins(&mut sym);
                            self.block.custom_symbol_override = Some(sym);
                        }
                    }
                });
                ui.separator();

                let mut can_commit = false;
                if self.active_tab == HdlDialogTab::Symbol {
                    self.apply_voltage_strings();
                    if let Ok(parsed) = parse_hdl(self.block.language, &self.block.source) {
                        self.block.module_name = parsed.module_name;
                        self.block.ports = parsed.ports;
                        can_commit = self.block.synthesize_subckt("HDL_PREVIEW").is_ok();
                    }
                    self.show_symbol_tab(ui);
                } else {
                // Top bar: Language, Template, Load/Save
                ui.horizontal_wrapped(|ui| {
                    ui.label("Language:");
                    let prev_lang = self.block.language;
                    ui.selectable_value(&mut self.block.language, HdlLanguage::Verilog, "Verilog (.v)");
                    ui.selectable_value(&mut self.block.language, HdlLanguage::Vhdl, "VHDL (.vhd)");
                    if self.block.language != prev_lang && self.editing_comp_id.is_none() {
                        self.selected_template_idx = 0;
                        let tpl = match self.block.language {
                            HdlLanguage::Verilog => verilog_templates()[0].1,
                            HdlLanguage::Vhdl => vhdl_templates()[0].1,
                        };
                        self.block.source = tpl.to_string();
                    }

                    ui.separator();
                    let templates = match self.block.language {
                        HdlLanguage::Verilog => verilog_templates(),
                        HdlLanguage::Vhdl => vhdl_templates(),
                    };
                    let cur_tpl_name = templates
                        .get(self.selected_template_idx)
                        .map(|t| t.0)
                        .unwrap_or("Template");
                    egui::ComboBox::from_id_salt("hdl_template_cb")
                        .selected_text(format!("Template: {cur_tpl_name}"))
                        .show_ui(ui, |ui| {
                            for (i, (name, src)) in templates.iter().enumerate() {
                                if ui.selectable_label(self.selected_template_idx == i, *name).clicked() {
                                    self.selected_template_idx = i;
                                    self.block.source = (*src).to_string();
                                }
                            }
                        });

                    ui.separator();
                    if ui.button("Load File...").clicked() {
                        let exts: &[&str] = match self.block.language {
                            HdlLanguage::Verilog => &["v", "sv"],
                            HdlLanguage::Vhdl => &["vhd", "vhdl"],
                        };
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("HDL Source", exts)
                            .add_filter("All Files", &["v", "sv", "vhd", "vhdl"])
                            .pick_file()
                        {
                            if let Ok(txt) = std::fs::read_to_string(&path) {
                                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                                    if ext.eq_ignore_ascii_case("vhd") || ext.eq_ignore_ascii_case("vhdl") {
                                        self.block.language = HdlLanguage::Vhdl;
                                    } else if ext.eq_ignore_ascii_case("v") || ext.eq_ignore_ascii_case("sv") {
                                        self.block.language = HdlLanguage::Verilog;
                                    }
                                }
                                self.block.source = txt;
                                self.block.source_file = Some(path.display().to_string());
                            }
                        }
                    }
                    if ui.button("Save File...").clicked() {
                        let ext = match self.block.language {
                            HdlLanguage::Verilog => "v",
                            HdlLanguage::Vhdl => "vhd",
                        };
                        if let Some(path) = rfd::FileDialog::new()
                            .set_file_name(&format!("{}.{ext}", self.block.module_name))
                            .save_file()
                        {
                            if std::fs::write(&path, &self.block.source).is_ok() {
                                self.block.source_file = Some(path.display().to_string());
                            }
                        }
                    }
                });

                if self.editing_comp_id.is_some() {
                    ui.horizontal(|ui| {
                        ui.label("RefDes:");
                        ui.add(egui::TextEdit::singleline(&mut self.refdes).desired_width(100.0));
                    });
                }

                ui.add_space(4.0);

                // Allocate heights strictly within `ui.available_height()` so the window
                // NEVER self-expands in a layout feedback loop, while still growing smoothly
                // when the user manually resizes the window.
                let avail_h = ui.available_height();
                let buttons_reserve = 38.0;
                let has_port_panel =
                    !self.block.port_configs.is_empty() || !self.block.ref_pins.is_empty();
                let bottom_target = if has_port_panel {
                    (avail_h * 0.44).clamp(180.0, 270.0)
                } else {
                    (avail_h * 0.36).clamp(150.0, 220.0)
                };
                let code_h = (avail_h - bottom_target - buttons_reserve - 16.0).max(110.0);
                let min_rows = ((code_h / 16.0).floor() as usize).max(6);

                // Source code editor with always-visible solid scrollbars
                ui.scope(|ui| {
                    ui.style_mut().spacing.scroll = egui::style::ScrollStyle::solid();
                    ScrollArea::both()
                        .id_salt("hdl_code_scroll")
                        .min_scrolled_height(code_h)
                        .max_height(code_h)
                        .auto_shrink([false, false])
                        .scroll_bar_visibility(
                            egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                        )
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut self.block.source)
                                    .font(egui::TextStyle::Monospace)
                                    .code_editor()
                                    .desired_rows(min_rows)
                                    .desired_width(f32::INFINITY),
                            );
                        });
                });

                ui.add_space(4.0);
                ui.separator();

                // Bounded scrollable settings & per-port configuration area
                self.apply_voltage_strings();
                ScrollArea::vertical()
                    .id_salt("hdl_settings_scroll")
                    .max_height(bottom_target)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        let parse_res = parse_hdl(self.block.language, &self.block.source);
                        match &parse_res {
                            Ok(parsed) => {
                                self.block.module_name = parsed.module_name.clone();
                                self.block.ports = parsed.ports.clone();
                                let pin_summary: Vec<String> = parsed
                                    .ports
                                    .iter()
                                    .map(|p| {
                                        if p.width > 1 {
                                            format!("{}:{}[{}:{}]", p.name, p.dir.label(), p.msb, p.lsb)
                                        } else {
                                            format!("{}:{}", p.name, p.dir.label())
                                        }
                                    })
                                    .collect();
                                ui.label(
                                    RichText::new(format!(
                                        "Module '{}' — {} port(s) ({} pins): {}",
                                        parsed.module_name,
                                        parsed.ports.len(),
                                        self.block.expanded_pins().len(),
                                        pin_summary.join(", ")
                                    ))
                                    .color(Color32::from_rgb(0, 120, 40)),
                                );
                                match self.block.synthesize_subckt("HDL_PREVIEW") {
                                    Ok(lines) => {
                                        can_commit = true;
                                        ui.label(
                                            RichText::new(format!(
                                                "Synthesis OK ({} XSPICE netlist lines)",
                                                lines.len()
                                            ))
                                            .small()
                                            .color(Color32::from_rgb(40, 90, 40)),
                                        );
                                    }
                                    Err(e) => {
                                        ui.label(
                                            RichText::new(format!("Synthesis error: {e}"))
                                                .color(Color32::from_rgb(190, 40, 20)),
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                ui.label(
                                    RichText::new(format!("Parse error: {e}"))
                                        .color(Color32::from_rgb(190, 40, 20)),
                                );
                            }
                        }

                        ui.add_space(4.0);

                        // Analog / Digital Bridge Electrical Settings
                        ui.label(RichText::new("Analog/Digital Interface (ADC / DAC Bridge)").strong());
                        ui.horizontal_wrapped(|ui| {
                            ui.label("V_low (V):");
                            ui.add(egui::TextEdit::singleline(&mut self.v_low_str).desired_width(48.0));
                            ui.label("V_high (V):");
                            ui.add(egui::TextEdit::singleline(&mut self.v_high_str).desired_width(48.0));
                            ui.label("V_IL (V):");
                            ui.add(egui::TextEdit::singleline(&mut self.v_il_str).desired_width(48.0));
                            ui.label("V_IH (V):");
                            ui.add(egui::TextEdit::singleline(&mut self.v_ih_str).desired_width(48.0));
                            ui.label("t_rise:");
                            ui.add(egui::TextEdit::singleline(&mut self.block.t_rise).desired_width(48.0));
                            ui.label("t_fall:");
                            ui.add(egui::TextEdit::singleline(&mut self.block.t_fall).desired_width(48.0));
                        });

                        ui.horizontal(|ui| {
                            ui.label("Backend:");
                            egui::ComboBox::from_id_salt("hdl_backend_cb")
                                .selected_text(self.block.backend.label())
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut self.block.backend,
                                        HdlBackend::Auto,
                                        HdlBackend::Auto.label(),
                                    );
                                    ui.selectable_value(
                                        &mut self.block.backend,
                                        HdlBackend::BuiltInXspice,
                                        HdlBackend::BuiltInXspice.label(),
                                    );
                                    ui.selectable_value(
                                        &mut self.block.backend,
                                        HdlBackend::VerilatorCosim,
                                        HdlBackend::VerilatorCosim.label(),
                                    );
                                });
                            if ui.small_button("3.3V CMOS").clicked() {
                                self.block.v_low = 0.0;
                                self.block.v_high = 3.3;
                                self.block.v_thresh_low = 1.0;
                                self.block.v_thresh_high = 2.3;
                                self.v_low_str = format!("{}", self.block.v_low);
                                self.v_high_str = format!("{}", self.block.v_high);
                                self.v_il_str = format!("{}", self.block.v_thresh_low);
                                self.v_ih_str = format!("{}", self.block.v_thresh_high);
                            }
                            if ui.small_button("5V CMOS").clicked() {
                                self.block.v_low = 0.0;
                                self.block.v_high = 5.0;
                                self.block.v_thresh_low = 1.5;
                                self.block.v_thresh_high = 3.5;
                                self.v_low_str = format!("{}", self.block.v_low);
                                self.v_high_str = format!("{}", self.block.v_high);
                                self.v_il_str = format!("{}", self.block.v_thresh_low);
                                self.v_ih_str = format!("{}", self.block.v_thresh_high);
                            }
                        });

                        // Wired Analog Reference Input Pins (added to bottom of symbol)
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new("Analog Reference Pins:").strong());
                            ui.add(
                                egui::TextEdit::singleline(&mut self.new_ref_pin_input)
                                    .hint_text("e.g. REF1, VSS")
                                    .desired_width(105.0),
                            );
                            if ui.small_button("+ Add Ref Pin").clicked() {
                                let raw = self.new_ref_pin_input.trim();
                                let name = if raw.is_empty() {
                                    format!("REF{}", self.block.ref_pins.len() + 1)
                                } else {
                                    raw.to_string()
                                };
                                if !self
                                    .block
                                    .ref_pins
                                    .iter()
                                    .any(|r| r.eq_ignore_ascii_case(&name))
                                {
                                    self.block.ref_pins.push(name);
                                }
                                self.new_ref_pin_input.clear();
                            }
                            let mut remove_ref_idx: Option<usize> = None;
                            for (idx, rpin) in self.block.ref_pins.iter().enumerate() {
                                if ui
                                    .small_button(format!("{rpin} ×"))
                                    .on_hover_text("Remove analog reference pin from symbol")
                                    .clicked()
                                {
                                    remove_ref_idx = Some(idx);
                                }
                            }
                            if let Some(idx) = remove_ref_idx {
                                let removed = self.block.ref_pins.remove(idx);
                                for row in self.port_rows.values_mut() {
                                    if row.ref_net.eq_ignore_ascii_case(&removed) {
                                        row.ref_net.clear();
                                    }
                                }
                            }
                        });

                        // Per-Port Reference Net & Threshold Overrides
                        if !self.block.ports.is_empty() {
                            let ports_snapshot = self.block.ports.clone();
                            let ref_pins_snapshot = self.block.ref_pins.clone();
                            let default_open =
                                !self.block.port_configs.is_empty() || !ref_pins_snapshot.is_empty();
                            let v_il_def = format!("{}", self.block.v_thresh_low);
                            let v_ih_def = format!("{}", self.block.v_thresh_high);
                            let v_low_def = format!("{}", self.block.v_low);
                            let v_high_def = format!("{}", self.block.v_high);

                            ui.add_space(4.0);
                            egui::CollapsingHeader::new(format!(
                                "Per-Port Reference Signal & Thresholds ({} custom)",
                                self.block.port_configs.len()
                            ))
                            .default_open(default_open)
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(
                                        "Leave blank to inherit block defaults. Select a wired Analog Ref Pin (bottom of symbol), any schematic net, or 0 (GND).",
                                    )
                                    .small()
                                    .color(Color32::from_gray(90)),
                                );
                                egui::Grid::new("hdl_port_cfg_grid")
                                    .striped(true)
                                    .spacing([6.0, 3.0])
                                    .show(ui, |ui| {
                                        ui.strong("Port");
                                        ui.strong("Dir");
                                        ui.strong("Ref Pin / Net");
                                        ui.strong("V_IL / V_low (V)");
                                        ui.strong("V_IH / V_high (V)");
                                        ui.strong("");
                                        ui.end_row();

                                        for port in &ports_snapshot {
                                            let row = self
                                                .port_rows
                                                .entry(port.name.clone())
                                                .or_default();
                                            let port_lbl = if port.width > 1 {
                                                format!("{}[{}:{}]", port.name, port.msb, port.lsb)
                                            } else {
                                                port.name.clone()
                                            };
                                            ui.monospace(port_lbl);
                                            ui.label(port.dir.label());

                                            ui.horizontal(|ui| {
                                                ui.add(
                                                    egui::TextEdit::singleline(&mut row.ref_net)
                                                        .hint_text("0 (GND)")
                                                        .desired_width(72.0),
                                                );
                                                egui::ComboBox::from_id_salt(format!(
                                                    "ref_net_pick_{}",
                                                    port.name
                                                ))
                                                .width(20.0)
                                                .selected_text("")
                                                .show_ui(ui, |ui| {
                                                    if ui
                                                        .selectable_label(
                                                            row.ref_net.trim().is_empty(),
                                                            "0 (GND)",
                                                        )
                                                        .clicked()
                                                    {
                                                        row.ref_net.clear();
                                                    }
                                                    if !ref_pins_snapshot.is_empty() {
                                                        ui.separator();
                                                        for rpin in &ref_pins_snapshot {
                                                            if ui
                                                                .selectable_label(
                                                                    row.ref_net
                                                                        .trim()
                                                                        .eq_ignore_ascii_case(rpin),
                                                                    format!("{rpin} (ref pin)"),
                                                                )
                                                                .clicked()
                                                            {
                                                                row.ref_net = rpin.clone();
                                                            }
                                                        }
                                                    }
                                                    if !available_nets.is_empty() {
                                                        ui.separator();
                                                        for net in available_nets {
                                                            if ui
                                                                .selectable_label(
                                                                    row.ref_net.trim()
                                                                        == net.as_str(),
                                                                    net,
                                                                )
                                                                .clicked()
                                                            {
                                                                row.ref_net = net.clone();
                                                            }
                                                        }
                                                    }
                                                });
                                            });

                                            match port.dir {
                                                HdlPortDir::Input => {
                                                    ui.add(
                                                        egui::TextEdit::singleline(&mut row.v_il)
                                                            .hint_text(&v_il_def)
                                                            .desired_width(60.0),
                                                    );
                                                    ui.add(
                                                        egui::TextEdit::singleline(&mut row.v_ih)
                                                            .hint_text(&v_ih_def)
                                                            .desired_width(60.0),
                                                    );
                                                }
                                                HdlPortDir::Output | HdlPortDir::Inout => {
                                                    ui.add(
                                                        egui::TextEdit::singleline(&mut row.v_low)
                                                            .hint_text(&v_low_def)
                                                            .desired_width(60.0),
                                                    );
                                                    ui.add(
                                                        egui::TextEdit::singleline(&mut row.v_high)
                                                            .hint_text(&v_high_def)
                                                            .desired_width(60.0),
                                                    );
                                                }
                                            }

                                            if ui
                                                .small_button("×")
                                                .on_hover_text("Reset port to block defaults")
                                                .clicked()
                                            {
                                                *row = PortEditRow::default();
                                            }
                                            ui.end_row();
                                        }
                                    });
                            });
                        }

                        if !has_verilator_cosim_tools()
                            && self.block.backend == HdlBackend::VerilatorCosim
                        {
                            ui.label(
                                RichText::new("Note: 'verilator' is not detected on PATH. Built-in XSPICE Synthesizer works without external tools.")
                                    .small()
                                    .color(Color32::from_rgb(170, 90, 0)),
                            );
                        }
                    });
                }

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let btn_label = if self.editing_comp_id.is_some() {
                        "Update Block"
                    } else {
                        "Place on Schematic"
                    };
                    if ui.add_enabled(can_commit, egui::Button::new(btn_label)).clicked() {
                        self.apply_voltage_strings();
                        self.block.refresh_metadata();
                        if let Some(cid) = self.editing_comp_id {
                            action = HdlDialogAction::UpdateExisting {
                                comp_id: cid,
                                refdes: self.refdes.clone(),
                                block: self.block.clone(),
                            };
                        } else {
                            action = HdlDialogAction::PlaceNew(self.block.clone());
                        }
                        self.open = false;
                    }
                    if ui.button("Cancel").clicked() {
                        self.open = false;
                    }
                });
            });

        if !open {
            self.open = false;
        }
        action
    }

    fn show_symbol_tab(&mut self, ui: &mut Ui) {
        // Ensure we have a mutable working copy of the symbol synced to current ports/ref pins
        let mut sym = self.block.build_symbol();
        let mut modified = false;

        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.sym_tool, HdlSymTool::MovePin, "Move");
            ui.selectable_value(&mut self.sym_tool, HdlSymTool::Line, "Line");
            ui.selectable_value(&mut self.sym_tool, HdlSymTool::Rect, "Rect");
            ui.selectable_value(&mut self.sym_tool, HdlSymTool::Circle, "Circle");
            ui.selectable_value(&mut self.sym_tool, HdlSymTool::Text, "Text");
            ui.selectable_value(&mut self.sym_tool, HdlSymTool::Delete, "Delete");
            ui.separator();
            if self.sym_tool == HdlSymTool::Text {
                ui.label("Text:");
                ui.add(egui::TextEdit::singleline(&mut self.sym_next_text).desired_width(80.0));
                ui.separator();
            }
            if ui
                .add_enabled(!self.sym_undo.is_empty(), egui::Button::new("Undo"))
                .clicked()
            {
                if let Some(prev) = self.sym_undo.pop() {
                    sym = prev;
                    modified = true;
                }
            }
            if ui
                .button("Reset Default")
                .on_hover_text("Regenerate default IC symbol from current HDL ports")
                .clicked()
            {
                self.sym_undo.push(sym.clone());
                self.block.custom_symbol_override = None;
                return;
            }
            if ui.button("Load .asy...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("LTspice Symbol", &["asy"])
                    .pick_file()
                {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let name = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(&self.block.module_name);
                        self.sym_undo.push(sym.clone());
                        let mut loaded = CustomSymbol::from_asy(name, &content);
                        self.block.sync_symbol_pins(&mut loaded);
                        sym = loaded;
                        modified = true;
                    }
                }
            }
            if ui.button("Save .asy...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name(&format!("{}.asy", self.block.module_name))
                    .add_filter("LTspice Symbol", &["asy"])
                    .save_file()
                {
                    let _ = std::fs::write(&path, sym.to_asy());
                }
            }
        });

        ui.label(
            RichText::new(
                "Drag pins or shapes (lines, rects, circles, text) with 'Move', draw custom shapes (Line/Rect/Circle/Text), or delete shapes. Scroll to zoom.",
            )
            .small()
            .color(Color32::from_gray(90)),
        );

        let canvas_h = (ui.available_height() - 44.0).max(180.0);
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), canvas_h), Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 2.0, Color32::from_rgb(192, 192, 192));
        painter.rect_stroke(rect, 2.0, Stroke::new(1.0_f32, Color32::from_gray(110)));

        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.1 {
                let f = if scroll > 0.0 { 1.12 } else { 1.0 / 1.12 };
                self.sym_zoom = (self.sym_zoom * f).clamp(0.7, 4.0);
            }
        }

        let center = rect.center();
        let step = GRID_SIZE * self.sym_zoom;
        let to_screen = |gx: f32, gy: f32| -> Pos2 { Pos2::new(center.x + gx * step, center.y + gy * step) };
        let to_grid_half = |pos: Pos2| -> (f32, f32) {
            let gx = (((pos.x - center.x) / step) * 2.0).round() / 2.0;
            let gy = (((pos.y - center.y) / step) * 2.0).round() / 2.0;
            (gx, gy)
        };
        let to_grid_int = |pos: Pos2| -> (i32, i32) {
            (
                ((pos.x - center.x) / step).round() as i32,
                ((pos.y - center.y) / step).round() as i32,
            )
        };

        // Draw grid dots
        let half_cols = (rect.width() / (2.0 * step)).ceil() as i32 + 1;
        let half_rows = (rect.height() / (2.0 * step)).ceil() as i32 + 1;
        let dot_col = Color32::from_rgb(145, 145, 145);
        for gx in -half_cols..=half_cols {
            for gy in -half_rows..=half_rows {
                let p = to_screen(gx as f32, gy as f32);
                if rect.contains(p) {
                    painter.rect_filled(Rect::from_center_size(p, Vec2::splat(1.5)), 0.0, dot_col);
                }
            }
        }
        // Origin crosshair
        let orig_col = Color32::from_rgba_unmultiplied(120, 120, 120, 140);
        painter.line_segment(
            [Pos2::new(rect.left(), center.y), Pos2::new(rect.right(), center.y)],
            Stroke::new(1.0_f32, orig_col),
        );
        painter.line_segment(
            [Pos2::new(center.x, rect.top()), Pos2::new(center.x, rect.bottom())],
            Stroke::new(1.0_f32, orig_col),
        );

        // Draw shapes
        let sym_col = Color32::from_rgb(0, 0, 150);
        let drag_col = Color32::from_rgb(220, 100, 0);
        let stroke_w = (1.6 * self.sym_zoom).clamp(1.2, 3.0);
        let dragging_shape_idx = self.sym_dragging_shape.as_ref().map(|(i, _, _)| *i);
        for (i, s) in sym.shapes.iter().enumerate() {
            let is_drag = dragging_shape_idx == Some(i);
            let st = Stroke::new(stroke_w, if is_drag { drag_col } else { sym_col });
            match s {
                SymbolShape::Line { x1, y1, x2, y2 } => {
                    painter.line_segment([to_screen(*x1, *y1), to_screen(*x2, *y2)], st);
                }
                SymbolShape::Rect { x1, y1, x2, y2 } => {
                    painter.rect_stroke(
                        Rect::from_two_pos(to_screen(*x1, *y1), to_screen(*x2, *y2)),
                        0.0,
                        st,
                    );
                }
                SymbolShape::Circle { cx, cy, r } => {
                    painter.circle_stroke(to_screen(*cx, *cy), *r * step, st);
                }
                SymbolShape::Text { x, y, text } => {
                    painter.text(
                        to_screen(*x, *y),
                        egui::Align2::LEFT_CENTER,
                        text,
                        FontId::proportional((11.5 * self.sym_zoom).clamp(9.0, 20.0)),
                        if is_drag { drag_col } else { Color32::BLACK },
                    );
                }
            }
        }

        // Draw pins with order badges
        for (idx, p) in sym.pins.iter().enumerate() {
            let sp = to_screen(p.x as f32, p.y as f32);
            let is_drag = self.sym_dragging_pin == Some(idx);
            let col = if is_drag {
                Color32::from_rgb(220, 100, 0)
            } else {
                Color32::from_rgb(0, 80, 200)
            };
            painter.rect_filled(Rect::from_center_size(sp, Vec2::splat(7.0)), 1.0, col);
            painter.text(
                sp + Vec2::new(6.0, -8.0),
                egui::Align2::LEFT_BOTTOM,
                format!("{} (#{})", p.name, p.order),
                FontId::monospace(10.5),
                Color32::from_rgb(130, 0, 0),
            );
        }

        // Handle interactions
        match self.sym_tool {
            HdlSymTool::MovePin => {
                if response.drag_started_by(egui::PointerButton::Primary) {
                    if let Some(pos) = response.interact_pointer_pos() {
                        let pin_hit = sym
                            .pins
                            .iter()
                            .enumerate()
                            .map(|(i, p)| (i, to_screen(p.x as f32, p.y as f32).distance(pos)))
                            .filter(|(_, d)| *d <= 16.0)
                            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                            .map(|(i, _)| i);
                        if let Some(idx) = pin_hit {
                            self.sym_undo.push(sym.clone());
                            self.sym_dragging_pin = Some(idx);
                            self.sym_dragging_shape = None;
                        } else {
                            // Try hitting a shape (prefer text/lines/circles/rect edges over rect interior)
                            let shape_hit = sym
                                .shapes
                                .iter()
                                .enumerate()
                                .filter_map(|(i, s)| {
                                    let score = match s {
                                        SymbolShape::Text { x, y, text } => {
                                            let p = to_screen(*x, *y);
                                            let w = (text.len() as f32 * 7.0 * self.sym_zoom).max(20.0);
                                            let r = Rect::from_min_max(
                                                Pos2::new(p.x - 4.0, p.y - 12.0),
                                                Pos2::new(p.x + w, p.y + 12.0),
                                            );
                                            r.contains(pos).then_some(0.0_f32)
                                        }
                                        SymbolShape::Line { x1, y1, x2, y2 } => {
                                            let d = point_seg_dist(pos, to_screen(*x1, *y1), to_screen(*x2, *y2));
                                            (d < 10.0).then_some(d + 1.0)
                                        }
                                        SymbolShape::Circle { cx, cy, r } => {
                                            let d = (to_screen(*cx, *cy).distance(pos) - *r * step).abs();
                                            (d < 10.0).then_some(d + 1.0)
                                        }
                                        SymbolShape::Rect { x1, y1, x2, y2 } => {
                                            let r = Rect::from_two_pos(to_screen(*x1, *y1), to_screen(*x2, *y2));
                                            if r.expand(6.0).contains(pos) && !r.shrink(8.0).contains(pos) {
                                                Some(5.0)
                                            } else if r.contains(pos) {
                                                Some(20.0)
                                            } else {
                                                None
                                            }
                                        }
                                    };
                                    score.map(|sc| (i, sc))
                                })
                                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                                .map(|(i, _)| i);
                            if let Some(sidx) = shape_hit {
                                self.sym_undo.push(sym.clone());
                                self.sym_dragging_shape =
                                    Some((sidx, to_grid_half(pos), sym.shapes[sidx].clone()));
                                self.sym_dragging_pin = None;
                            }
                        }
                    }
                }
                if let Some(idx) = self.sym_dragging_pin {
                    if let Some(pos) = response.hover_pos().or_else(|| response.interact_pointer_pos()) {
                        let (gx, gy) = to_grid_int(pos);
                        if let Some(pin) = sym.pins.get_mut(idx) {
                            if pin.x != gx || pin.y != gy {
                                pin.x = gx;
                                pin.y = gy;
                                modified = true;
                            }
                        }
                    }
                    if response.drag_stopped_by(egui::PointerButton::Primary) {
                        self.sym_dragging_pin = None;
                    }
                } else if let Some((sidx, (sx, sy), orig)) = self.sym_dragging_shape.clone() {
                    if let Some(pos) = response.hover_pos().or_else(|| response.interact_pointer_pos()) {
                        let (gx, gy) = to_grid_half(pos);
                        let dx = gx - sx;
                        let dy = gy - sy;
                        if let Some(target) = sym.shapes.get_mut(sidx) {
                            let moved = match &orig {
                                SymbolShape::Line { x1, y1, x2, y2 } => SymbolShape::Line {
                                    x1: x1 + dx,
                                    y1: y1 + dy,
                                    x2: x2 + dx,
                                    y2: y2 + dy,
                                },
                                SymbolShape::Rect { x1, y1, x2, y2 } => SymbolShape::Rect {
                                    x1: x1 + dx,
                                    y1: y1 + dy,
                                    x2: x2 + dx,
                                    y2: y2 + dy,
                                },
                                SymbolShape::Circle { cx, cy, r } => SymbolShape::Circle {
                                    cx: cx + dx,
                                    cy: cy + dy,
                                    r: *r,
                                },
                                SymbolShape::Text { x, y, text } => SymbolShape::Text {
                                    x: x + dx,
                                    y: y + dy,
                                    text: text.clone(),
                                },
                            };
                            *target = moved;
                            modified = true;
                        }
                    }
                    if response.drag_stopped_by(egui::PointerButton::Primary) {
                        self.sym_dragging_shape = None;
                    }
                }
            }
            HdlSymTool::Line | HdlSymTool::Rect | HdlSymTool::Circle => {
                if response.drag_started_by(egui::PointerButton::Primary) {
                    if let Some(pos) = response.interact_pointer_pos() {
                        self.sym_drag_start = Some(to_grid_half(pos));
                    }
                }
                if let Some((sx, sy)) = self.sym_drag_start {
                    if let Some(pos) = response.hover_pos() {
                        let (ex, ey) = to_grid_half(pos);
                        let prev_stroke =
                            Stroke::new(1.5_f32, Color32::from_rgba_unmultiplied(0, 0, 220, 150));
                        match self.sym_tool {
                            HdlSymTool::Line => {
                                painter.line_segment([to_screen(sx, sy), to_screen(ex, ey)], prev_stroke);
                            }
                            HdlSymTool::Rect => {
                                painter.rect_stroke(
                                    Rect::from_two_pos(to_screen(sx, sy), to_screen(ex, ey)),
                                    0.0,
                                    prev_stroke,
                                );
                            }
                            HdlSymTool::Circle => {
                                let r = ((ex - sx).hypot(ey - sy)).max(0.25);
                                painter.circle_stroke(to_screen(sx, sy), r * step, prev_stroke);
                            }
                            _ => {}
                        }
                    }
                    if response.drag_stopped_by(egui::PointerButton::Primary) {
                        if let Some(pos) = response.interact_pointer_pos() {
                            let (ex, ey) = to_grid_half(pos);
                            if (ex - sx).abs() > 0.01 || (ey - sy).abs() > 0.01 {
                                self.sym_undo.push(sym.clone());
                                match self.sym_tool {
                                    HdlSymTool::Line => sym.shapes.push(SymbolShape::Line {
                                        x1: sx,
                                        y1: sy,
                                        x2: ex,
                                        y2: ey,
                                    }),
                                    HdlSymTool::Rect => sym.shapes.push(SymbolShape::Rect {
                                        x1: sx.min(ex),
                                        y1: sy.min(ey),
                                        x2: sx.max(ex),
                                        y2: sy.max(ey),
                                    }),
                                    HdlSymTool::Circle => sym.shapes.push(SymbolShape::Circle {
                                        cx: sx,
                                        cy: sy,
                                        r: (ex - sx).hypot(ey - sy).max(0.25),
                                    }),
                                    _ => {}
                                }
                                modified = true;
                            }
                        }
                        self.sym_drag_start = None;
                    }
                }
            }
            HdlSymTool::Text => {
                if response.clicked() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        let (gx, gy) = to_grid_half(pos);
                        if !self.sym_next_text.trim().is_empty() {
                            self.sym_undo.push(sym.clone());
                            sym.shapes.push(SymbolShape::Text {
                                x: gx,
                                y: gy,
                                text: self.sym_next_text.clone(),
                            });
                            modified = true;
                        }
                    }
                }
            }
            HdlSymTool::Delete => {
                if response.clicked() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        let hit_idx = sym.shapes.iter().enumerate().find_map(|(i, s)| {
                            let hit = match s {
                                SymbolShape::Line { x1, y1, x2, y2 } => {
                                    point_seg_dist(pos, to_screen(*x1, *y1), to_screen(*x2, *y2)) < 10.0
                                }
                                SymbolShape::Rect { x1, y1, x2, y2 } => {
                                    let r = Rect::from_two_pos(to_screen(*x1, *y1), to_screen(*x2, *y2));
                                    r.expand(6.0).contains(pos) && !r.shrink(8.0).contains(pos)
                                }
                                SymbolShape::Circle { cx, cy, r } => {
                                    let d = to_screen(*cx, *cy).distance(pos);
                                    (d - *r * step).abs() < 10.0
                                }
                                SymbolShape::Text { x, y, .. } => {
                                    to_screen(*x, *y).distance(pos) < 22.0
                                }
                            };
                            hit.then_some(i)
                        });
                        if let Some(i) = hit_idx {
                            self.sym_undo.push(sym.clone());
                            sym.shapes.remove(i);
                            modified = true;
                        }
                    }
                }
            }
        }

        if modified {
            self.block.custom_symbol_override = Some(sym);
        }
    }
}

fn point_seg_dist(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    if len2 < 1e-4 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}
