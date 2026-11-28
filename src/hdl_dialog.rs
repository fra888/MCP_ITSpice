use std::collections::HashMap;

use egui::{Color32, RichText, ScrollArea};

use crate::hdl::{
    has_verilator_cosim_tools, parse_hdl, verilog_templates, vhdl_templates, HdlBackend, HdlBlock,
    HdlLanguage, HdlPortConfig, HdlPortDir,
};

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
        self.sync_voltage_strings();
    }

    pub fn open_existing(&mut self, comp_id: u64, refdes: &str, block: &HdlBlock) {
        self.open = true;
        self.editing_comp_id = Some(comp_id);
        self.refdes = refdes.to_string();
        self.block = block.clone();
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
                let mut can_commit = false;
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
}
