use egui::{Align, Frame, Layout, RichText, Stroke, Ui, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFunc {
    None,
    Pulse,
    Sine,
    Exp,
    Sffm,
    Pwl,
    PwlFile,
}

#[derive(Debug, Clone)]
pub struct SourceDialogState {
    pub comp_id: u64,
    pub refdes: String,
    pub is_current: bool,
    pub func: SourceFunc,
    // PULSE(V1 V2 Td Tr Tf Ton Per Ncycles)
    pub pulse: [String; 8],
    // SINE(Voff Vamp Freq Td Theta Phi Ncycles)
    pub sine: [String; 7],
    // EXP(V1 V2 Td1 Tau1 Td2 Tau2)
    pub exp: [String; 6],
    // SFFM(Voff Vamp Fcar MDI Fsig)
    pub sffm: [String; 5],
    // PWL(t1 v1 t2 v2...)
    pub pwl_points: String,
    pub pwl_file: String,
    // Right column
    pub dc_value: String,
    pub ac_amp: String,
    pub ac_phase: String,
    pub rser: String,
    pub cpar: String,
    // Visibility flags
    pub vis_func: bool,
    pub vis_dc: bool,
    pub vis_ac: bool,
    pub vis_parasitic: bool,
}

impl SourceDialogState {
    pub fn from_component(comp_id: u64, refdes: &str, value: &str, is_current: bool) -> Self {
        let mut st = Self {
            comp_id,
            refdes: refdes.to_string(),
            is_current,
            func: SourceFunc::None,
            pulse: Default::default(),
            sine: Default::default(),
            exp: Default::default(),
            sffm: Default::default(),
            pwl_points: String::new(),
            pwl_file: String::new(),
            dc_value: String::new(),
            ac_amp: String::new(),
            ac_phase: String::new(),
            rser: String::new(),
            cpar: String::new(),
            vis_func: true,
            vis_dc: true,
            vis_ac: true,
            vis_parasitic: true,
        };
        st.parse_value(value);
        st
    }

    fn parse_value(&mut self, raw: &str) {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return;
        }
        let upper = trimmed.to_ascii_uppercase();

        if let Some(args) = extract_func_args(trimmed, &upper, "PULSE") {
            self.func = SourceFunc::Pulse;
            for (i, a) in args.iter().take(8).enumerate() {
                self.pulse[i] = (*a).to_string();
            }
        } else if let Some(args) = extract_func_args(trimmed, &upper, "SINE")
            .or_else(|| extract_func_args(trimmed, &upper, "SIN"))
        {
            self.func = SourceFunc::Sine;
            for (i, a) in args.iter().take(7).enumerate() {
                self.sine[i] = (*a).to_string();
            }
        } else if let Some(args) = extract_func_args(trimmed, &upper, "EXP") {
            self.func = SourceFunc::Exp;
            for (i, a) in args.iter().take(6).enumerate() {
                self.exp[i] = (*a).to_string();
            }
        } else if let Some(args) = extract_func_args(trimmed, &upper, "SFFM") {
            self.func = SourceFunc::Sffm;
            for (i, a) in args.iter().take(5).enumerate() {
                self.sffm[i] = (*a).to_string();
            }
        } else if let Some(inner) = extract_func_raw(trimmed, &upper, "PWL") {
            if inner.to_ascii_uppercase().starts_with("FILE=") {
                self.func = SourceFunc::PwlFile;
                self.pwl_file = inner[5..].trim().to_string();
            } else {
                self.func = SourceFunc::Pwl;
                self.pwl_points = inner.to_string();
            }
        } else {
            // Parse DC and optional AC tokens (e.g. "5", "DC 5 AC 1 0")
            let tokens: Vec<&str> = trimmed.split_whitespace().collect();
            if tokens.len() == 1 {
                self.dc_value = tokens[0].to_string();
            } else {
                let mut i = 0;
                while i < tokens.len() {
                    let t_up = tokens[i].to_ascii_uppercase();
                    if t_up == "DC" && i + 1 < tokens.len() {
                        self.dc_value = tokens[i + 1].to_string();
                        i += 2;
                    } else if t_up == "AC" && i + 1 < tokens.len() {
                        self.ac_amp = tokens[i + 1].to_string();
                        if i + 2 < tokens.len()
                            && !tokens[i + 2].eq_ignore_ascii_case("DC")
                        {
                            self.ac_phase = tokens[i + 2].to_string();
                            i += 3;
                        } else {
                            i += 2;
                        }
                    } else {
                        if self.dc_value.is_empty() {
                            self.dc_value = tokens[i].to_string();
                        }
                        i += 1;
                    }
                }
            }
        }
    }

    pub fn build_value(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        match self.func {
            SourceFunc::None => {
                let dc = self.dc_value.trim();
                let ac = self.ac_amp.trim();
                if !dc.is_empty() && ac.is_empty() {
                    parts.push(dc.to_string());
                } else {
                    if !dc.is_empty() {
                        parts.push(format!("DC {dc}"));
                    }
                    if !ac.is_empty() {
                        let ph = self.ac_phase.trim();
                        if ph.is_empty() {
                            parts.push(format!("AC {ac}"));
                        } else {
                            parts.push(format!("AC {ac} {ph}"));
                        }
                    }
                }
            }
            SourceFunc::Pulse => {
                parts.push(format!("PULSE({})", join_non_empty(&self.pulse)));
            }
            SourceFunc::Sine => {
                parts.push(format!("SINE({})", join_non_empty(&self.sine)));
            }
            SourceFunc::Exp => {
                parts.push(format!("EXP({})", join_non_empty(&self.exp)));
            }
            SourceFunc::Sffm => {
                parts.push(format!("SFFM({})", join_non_empty(&self.sffm)));
            }
            SourceFunc::Pwl => {
                parts.push(format!("PWL({})", self.pwl_points.trim()));
            }
            SourceFunc::PwlFile => {
                parts.push(format!("PWL file={}", self.pwl_file.trim()));
            }
        }

        if parts.is_empty() {
            "0".to_string()
        } else {
            parts.join(" ")
        }
    }

    /// Returns `Some(true)` if OK was clicked, `Some(false)` if Cancel/closed, `None` if still open.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<bool> {
        let mut result = None;
        let kind_name = if self.is_current {
            "Independent Current Source"
        } else {
            "Independent Voltage Source"
        };
        let title = format!("{kind_name} - {}", self.refdes);

        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .default_width(600.0)
            .show(ctx, |ui| {
                ui.columns(2, |cols| {
                    // Left Column: Functions
                    group_box(&mut cols[0], "Functions", |ui| {
                        ui.radio_value(&mut self.func, SourceFunc::None, "(none)");
                        ui.radio_value(
                            &mut self.func,
                            SourceFunc::Pulse,
                            "PULSE(V1 V2 Tdelay Trise Tfall Ton Period Ncycles)",
                        );
                        ui.radio_value(
                            &mut self.func,
                            SourceFunc::Sine,
                            "SINE(Voffset Vamp Freq Td Theta Phi Ncycles)",
                        );
                        ui.radio_value(
                            &mut self.func,
                            SourceFunc::Exp,
                            "EXP(V1 V2 Td1 Tau1 Td2 Tau2)",
                        );
                        ui.radio_value(
                            &mut self.func,
                            SourceFunc::Sffm,
                            "SFFM(Voff Vamp Fcar MDI Fsig)",
                        );
                        ui.radio_value(&mut self.func, SourceFunc::Pwl, "PWL(t1 v1 t2 v2...)");
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut self.func, SourceFunc::PwlFile, "PWL FILE:");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.pwl_file)
                                    .desired_width(130.0),
                            );
                            if ui.button("Browse").clicked() {
                                if let Some(path) = rfd::FileDialog::new().pick_file() {
                                    self.pwl_file = path.display().to_string();
                                    self.func = SourceFunc::PwlFile;
                                }
                            }
                        });

                        ui.add_space(6.0);
                        match self.func {
                            SourceFunc::None => {}
                            SourceFunc::Pulse => {
                                let labels = [
                                    "Vinitial[V]:",
                                    "Von[V]:",
                                    "Tdelay[s]:",
                                    "Trise[s]:",
                                    "Tfall[s]:",
                                    "Ton[s]:",
                                    "Tperiod[s]:",
                                    "Ncycles:",
                                ];
                                param_grid(ui, "pulse_grid", &labels, &mut self.pulse);
                            }
                            SourceFunc::Sine => {
                                let labels = [
                                    "DC offset[V]:",
                                    "Amplitude[V]:",
                                    "Freq[Hz]:",
                                    "Tdelay[s]:",
                                    "Theta[1/s]:",
                                    "Phi[deg]:",
                                    "Ncycles:",
                                ];
                                param_grid(ui, "sine_grid", &labels, &mut self.sine);
                            }
                            SourceFunc::Exp => {
                                let labels = [
                                    "Vinitial[V]:",
                                    "Vpulsed[V]:",
                                    "Td1[s]:",
                                    "Tau1[s]:",
                                    "Td2[s]:",
                                    "Tau2[s]:",
                                ];
                                param_grid(ui, "exp_grid", &labels, &mut self.exp);
                            }
                            SourceFunc::Sffm => {
                                let labels = [
                                    "DC offset[V]:",
                                    "Amplitude[V]:",
                                    "Carrier Freq[Hz]:",
                                    "Mod Index:",
                                    "Signal Freq[Hz]:",
                                ];
                                param_grid(ui, "sffm_grid", &labels, &mut self.sffm);
                            }
                            SourceFunc::Pwl => {
                                ui.label("Time/Value pairs (t1 v1 t2 v2 ...):");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.pwl_points)
                                        .desired_width(240.0),
                                );
                            }
                            SourceFunc::PwlFile => {}
                        }

                        ui.add_space(10.0);
                        ui.checkbox(
                            &mut self.vis_func,
                            "Make this information visible on schematic",
                        );
                    });

                    // Right Column: DC Value, Small signal AC, Parasitic Properties
                    let rcol = &mut cols[1];
                    group_box(rcol, "DC Value", |ui| {
                        ui.horizontal(|ui| {
                            ui.label("DC value:");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.dc_value)
                                    .desired_width(110.0),
                            );
                        });
                        ui.checkbox(
                            &mut self.vis_dc,
                            "Make this information visible on schematic",
                        );
                    });

                    rcol.add_space(6.0);
                    group_box(rcol, "Small signal AC analysis(.AC)", |ui| {
                        egui::Grid::new("ac_grid").num_columns(2).show(ui, |ui| {
                            ui.label("AC Amplitude:");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.ac_amp)
                                    .desired_width(110.0),
                            );
                            ui.end_row();
                            ui.label("AC Phase:");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.ac_phase)
                                    .desired_width(110.0),
                            );
                            ui.end_row();
                        });
                        ui.checkbox(
                            &mut self.vis_ac,
                            "Make this information visible on schematic",
                        );
                    });

                    rcol.add_space(6.0);
                    group_box(rcol, "Parasitic Properties", |ui| {
                        egui::Grid::new("par_grid").num_columns(2).show(ui, |ui| {
                            ui.label("Series Resistance[Ω]:");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.rser)
                                    .desired_width(110.0),
                            );
                            ui.end_row();
                            ui.label("Parallel Capacitance[F]:");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.cpar)
                                    .desired_width(110.0),
                            );
                            ui.end_row();
                        });
                        ui.checkbox(
                            &mut self.vis_parasitic,
                            "Make this information visible on schematic",
                        );
                    });
                });

                ui.add_space(8.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add_sized(Vec2::new(72.0, 24.0), egui::Button::new("OK"))
                        .clicked()
                    {
                        result = Some(true);
                    }
                    if ui
                        .add_sized(Vec2::new(72.0, 24.0), egui::Button::new("Cancel"))
                        .clicked()
                    {
                        result = Some(false);
                    }
                });
            });

        result
    }
}

fn group_box(ui: &mut Ui, title: &str, add_contents: impl FnOnce(&mut Ui)) {
    Frame::none()
        .stroke(Stroke::new(1.0_f32, egui::Color32::from_rgb(175, 175, 175)))
        .inner_margin(egui::Margin::same(8.0))
        .rounding(3.0)
        .show(ui, |ui| {
            ui.label(RichText::new(title).strong());
            ui.add_space(4.0);
            add_contents(ui);
        });
}

fn param_grid(ui: &mut Ui, id: &str, labels: &[&str], values: &mut [String]) {
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([8.0, 3.0])
        .show(ui, |ui| {
            for (lbl, val) in labels.iter().zip(values.iter_mut()) {
                ui.label(*lbl);
                ui.add(egui::TextEdit::singleline(val).desired_width(100.0));
                ui.end_row();
            }
        });
}

fn join_non_empty(vals: &[String]) -> String {
    let mut last_non_empty = 0;
    for (i, v) in vals.iter().enumerate() {
        if !v.trim().is_empty() {
            last_non_empty = i + 1;
        }
    }
    vals[..last_non_empty]
        .iter()
        .map(|s| {
            let t = s.trim();
            if t.is_empty() {
                "0"
            } else {
                t
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn extract_func_raw<'a>(raw: &'a str, upper: &str, name: &str) -> Option<&'a str> {
    let prefix = format!("{name}(");
    if upper.starts_with(&prefix) && raw.ends_with(')') {
        Some(&raw[prefix.len()..raw.len() - 1])
    } else {
        None
    }
}

fn extract_func_args<'a>(raw: &'a str, upper: &str, name: &str) -> Option<Vec<&'a str>> {
    extract_func_raw(raw, upper, name).map(|inner| inner.split_whitespace().collect())
}
