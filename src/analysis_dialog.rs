use egui::{Align, Frame, Layout, RichText, Stroke, Ui, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisTab {
    Transient,
    Ac,
    DcSweep,
    Noise,
    DcTransfer,
    DcOp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SweepType {
    Decade,
    Octave,
    Linear,
}

impl SweepType {
    fn spice_kw(self) -> &'static str {
        match self {
            Self::Decade => "dec",
            Self::Octave => "oct",
            Self::Linear => "lin",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AnalysisDialogState {
    pub directive_id: Option<u64>,
    pub x: i32,
    pub y: i32,
    pub tab: AnalysisTab,
    // Transient
    pub tran_stop: String,
    pub tran_start: String,
    pub tran_max_step: String,
    pub tran_startup: bool,
    pub tran_steady: bool,
    pub tran_no_reset_t0: bool,
    pub tran_step_load: bool,
    pub tran_uic: bool,
    pub tran_conv_report: bool,
    pub tran_save_state: bool,
    pub tran_save_time: String,
    pub tran_save_file: String,
    pub tran_load_state: bool,
    pub tran_load_file: String,
    // AC
    pub ac_sweep: SweepType,
    pub ac_points: String,
    pub ac_fstart: String,
    pub ac_fstop: String,
    // DC Sweep
    pub dc_src: String,
    pub dc_sweep: SweepType,
    pub dc_start: String,
    pub dc_stop: String,
    pub dc_incr: String,
    // Noise
    pub noise_out: String,
    pub noise_in: String,
    pub noise_sweep: SweepType,
    pub noise_points: String,
    pub noise_fstart: String,
    pub noise_fstop: String,
    // DC Transfer
    pub tf_out: String,
    pub tf_in: String,
    // Live command string
    pub cmd_text: String,
}

impl AnalysisDialogState {
    pub fn from_directive(id: Option<u64>, x: i32, y: i32, text: &str) -> Self {
        let mut st = Self {
            directive_id: id,
            x,
            y,
            tab: AnalysisTab::Transient,
            tran_stop: "20".into(),
            tran_start: String::new(),
            tran_max_step: String::new(),
            tran_startup: false,
            tran_steady: false,
            tran_no_reset_t0: false,
            tran_step_load: false,
            tran_uic: false,
            tran_conv_report: false,
            tran_save_state: false,
            tran_save_time: String::new(),
            tran_save_file: String::new(),
            tran_load_state: false,
            tran_load_file: String::new(),
            ac_sweep: SweepType::Decade,
            ac_points: "10".into(),
            ac_fstart: "1".into(),
            ac_fstop: "1Meg".into(),
            dc_src: "V1".into(),
            dc_sweep: SweepType::Linear,
            dc_start: "0".into(),
            dc_stop: "5".into(),
            dc_incr: "0.1".into(),
            noise_out: "V(out)".into(),
            noise_in: "V1".into(),
            noise_sweep: SweepType::Decade,
            noise_points: "10".into(),
            noise_fstart: "1".into(),
            noise_fstop: "1Meg".into(),
            tf_out: "V(out)".into(),
            tf_in: "V1".into(),
            cmd_text: text.trim().to_string(),
        };
        st.parse_existing(text);
        if st.cmd_text.is_empty() {
            st.rebuild_cmd();
        }
        st
    }

    fn parse_existing(&mut self, text: &str) {
        let first_line = text
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("");
        let tokens: Vec<&str> = first_line.split_whitespace().collect();
        if tokens.is_empty() {
            return;
        }
        let kw = tokens[0].to_ascii_lowercase();
        match kw.as_str() {
            ".tran" => {
                self.tab = AnalysisTab::Transient;
                let mut nums: Vec<&str> = Vec::new();
                for &t in &tokens[1..] {
                    let low = t.to_ascii_lowercase();
                    match low.as_str() {
                        "startup" => self.tran_startup = true,
                        "steady" => self.tran_steady = true,
                        "nodiscard" => self.tran_no_reset_t0 = true,
                        "step" => self.tran_step_load = true,
                        "uic" => self.tran_uic = true,
                        _ => nums.push(t),
                    }
                }
                if nums.len() == 1 {
                    self.tran_stop = nums[0].to_string();
                } else if nums.len() >= 2 {
                    self.tran_stop = nums[1].to_string();
                    if nums.len() >= 3 {
                        self.tran_start = nums[2].to_string();
                    }
                    if nums.len() >= 4 {
                        self.tran_max_step = nums[3].to_string();
                    }
                }
            }
            ".ac" => {
                self.tab = AnalysisTab::Ac;
                if tokens.len() >= 5 {
                    self.ac_sweep = match tokens[1].to_ascii_lowercase().as_str() {
                        "oct" => SweepType::Octave,
                        "lin" => SweepType::Linear,
                        _ => SweepType::Decade,
                    };
                    self.ac_points = tokens[2].to_string();
                    self.ac_fstart = tokens[3].to_string();
                    self.ac_fstop = tokens[4].to_string();
                }
            }
            ".dc" => {
                self.tab = AnalysisTab::DcSweep;
                if tokens.len() >= 5 {
                    self.dc_src = tokens[1].to_string();
                    self.dc_start = tokens[2].to_string();
                    self.dc_stop = tokens[3].to_string();
                    self.dc_incr = tokens[4].to_string();
                }
            }
            ".noise" => {
                self.tab = AnalysisTab::Noise;
                if tokens.len() >= 7 {
                    self.noise_out = tokens[1].to_string();
                    self.noise_in = tokens[2].to_string();
                    self.noise_sweep = match tokens[3].to_ascii_lowercase().as_str() {
                        "oct" => SweepType::Octave,
                        "lin" => SweepType::Linear,
                        _ => SweepType::Decade,
                    };
                    self.noise_points = tokens[4].to_string();
                    self.noise_fstart = tokens[5].to_string();
                    self.noise_fstop = tokens[6].to_string();
                }
            }
            ".tf" => {
                self.tab = AnalysisTab::DcTransfer;
                if tokens.len() >= 3 {
                    self.tf_out = tokens[1].to_string();
                    self.tf_in = tokens[2].to_string();
                }
            }
            ".op" => {
                self.tab = AnalysisTab::DcOp;
            }
            _ => {}
        }
    }

    fn rebuild_cmd(&mut self) {
        self.cmd_text = match self.tab {
            AnalysisTab::Transient => {
                let stop = self.tran_stop.trim();
                let start = self.tran_start.trim();
                let max_step = self.tran_max_step.trim();
                let mut parts = vec![".tran".to_string()];
                if start.is_empty() && max_step.is_empty() {
                    parts.push(if stop.is_empty() { "1m" } else { stop }.to_string());
                } else {
                    parts.push("0".to_string());
                    parts.push(if stop.is_empty() { "1m" } else { stop }.to_string());
                    parts.push(if start.is_empty() { "0" } else { start }.to_string());
                    if !max_step.is_empty() {
                        parts.push(max_step.to_string());
                    }
                }
                if self.tran_startup {
                    parts.push("startup".into());
                }
                if self.tran_steady {
                    parts.push("steady".into());
                }
                if self.tran_steady && self.tran_no_reset_t0 {
                    parts.push("nodiscard".into());
                }
                if self.tran_step_load {
                    parts.push("step".into());
                }
                if self.tran_uic {
                    parts.push("uic".into());
                }
                parts.join(" ")
            }
            AnalysisTab::Ac => format!(
                ".ac {} {} {} {}",
                self.ac_sweep.spice_kw(),
                self.ac_points.trim(),
                self.ac_fstart.trim(),
                self.ac_fstop.trim()
            ),
            AnalysisTab::DcSweep => format!(
                ".dc {} {} {} {}",
                self.dc_src.trim(),
                self.dc_start.trim(),
                self.dc_stop.trim(),
                self.dc_incr.trim()
            ),
            AnalysisTab::Noise => format!(
                ".noise {} {} {} {} {} {}",
                self.noise_out.trim(),
                self.noise_in.trim(),
                self.noise_sweep.spice_kw(),
                self.noise_points.trim(),
                self.noise_fstart.trim(),
                self.noise_fstop.trim()
            ),
            AnalysisTab::DcTransfer => {
                format!(".tf {} {}", self.tf_out.trim(), self.tf_in.trim())
            }
            AnalysisTab::DcOp => ".op".to_string(),
        };
    }

    fn syntax_hint(&self) -> &'static str {
        match self.tab {
            AnalysisTab::Transient => "Syntax:  .tran <Tstop> [<option> [<option>] ...]",
            AnalysisTab::Ac => "Syntax:  .ac <oct, dec, lin> <Npoints> <StartFreq> <EndFreq>",
            AnalysisTab::DcSweep => "Syntax:  .dc <srcnam> <Vstart> <Vstop> <Vincr>",
            AnalysisTab::Noise => {
                "Syntax:  .noise V(<out>[,<ref>]) <src> <oct, dec, lin> <Npts> <StartFreq> <EndFreq>"
            }
            AnalysisTab::DcTransfer => "Syntax:  .tf V(<out>) <src>",
            AnalysisTab::DcOp => "Syntax:  .op",
        }
    }

    /// Returns `Some(true)` on OK, `Some(false)` on Cancel, `None` while open.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<bool> {
        let mut result = None;

        egui::Window::new("Configure Analysis")
            .collapsible(false)
            .resizable(false)
            .default_width(540.0)
            .show(ctx, |ui| {
                // Tabs bar matching LTspice Configure Analysis
                ui.horizontal(|ui| {
                    let tabs = [
                        (AnalysisTab::Transient, "Transient"),
                        (AnalysisTab::Ac, "AC Analysis"),
                        (AnalysisTab::DcSweep, "DC sweep"),
                        (AnalysisTab::Noise, "Noise"),
                        (AnalysisTab::DcTransfer, "DC Transfer"),
                        (AnalysisTab::DcOp, "DC op pnt"),
                    ];
                    for (t, label) in tabs {
                        if ui.selectable_label(self.tab == t, label).clicked() {
                            self.tab = t;
                            self.rebuild_cmd();
                        }
                    }
                });
                ui.separator();

                let mut changed = false;
                match self.tab {
                    AnalysisTab::Transient => {
                        ui.vertical_centered(|ui| {
                            ui.label("Perform a non-linear, time-domain simulation.");
                        });
                        ui.add_space(6.0);

                        egui::Grid::new("tran_params_grid")
                            .num_columns(2)
                            .spacing([10.0, 4.0])
                            .show(ui, |ui| {
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label("Stop time:");
                                });
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut self.tran_stop)
                                            .desired_width(140.0),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                ui.end_row();

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label("Time to start saving data:");
                                });
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut self.tran_start)
                                            .desired_width(140.0),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                ui.end_row();

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label("Maximum Timestep:");
                                });
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut self.tran_max_step)
                                            .desired_width(140.0),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                ui.end_row();
                            });

                        ui.add_space(6.0);
                        if ui
                            .checkbox(
                                &mut self.tran_startup,
                                "Start external DC supply voltages at 0V",
                            )
                            .changed()
                        {
                            changed = true;
                        }
                        if ui
                            .checkbox(
                                &mut self.tran_steady,
                                "Stop simulating if steady state is detected",
                            )
                            .changed()
                        {
                            changed = true;
                        }
                        ui.add_enabled_ui(self.tran_steady, |ui| {
                            if ui
                                .checkbox(
                                    &mut self.tran_no_reset_t0,
                                    "Don't reset T=0 when steady state is detected",
                                )
                                .changed()
                            {
                                changed = true;
                            }
                        });
                        if ui
                            .checkbox(&mut self.tran_step_load, "Step the load current source")
                            .changed()
                        {
                            changed = true;
                        }
                        if ui
                            .checkbox(&mut self.tran_uic, "Skip initial operating point solution")
                            .changed()
                        {
                            changed = true;
                        }
                        if ui
                            .checkbox(
                                &mut self.tran_conv_report,
                                "Add convergence report to log (slows down sim)",
                            )
                            .changed()
                        {
                            changed = true;
                        }

                        ui.add_space(6.0);
                        Frame::none()
                            .stroke(Stroke::new(
                                1.0_f32,
                                egui::Color32::from_rgb(175, 175, 175),
                            ))
                            .inner_margin(egui::Margin::same(6.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.checkbox(
                                        &mut self.tran_save_state,
                                        "Save transient state to file:",
                                    );
                                    ui.label("Time (optional):");
                                    ui.add(
                                        egui::TextEdit::singleline(&mut self.tran_save_time)
                                            .desired_width(80.0),
                                    );
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Save state file name (optional):");
                                    ui.add(
                                        egui::TextEdit::singleline(&mut self.tran_save_file)
                                            .desired_width(210.0),
                                    );
                                    if ui.button("Browse").clicked() {
                                        if let Some(p) = rfd::FileDialog::new().save_file() {
                                            self.tran_save_file = p.display().to_string();
                                        }
                                    }
                                });
                                ui.checkbox(
                                    &mut self.tran_load_state,
                                    "Load initial state from file:",
                                );
                                ui.horizontal(|ui| {
                                    ui.label("Load state file name (optional):");
                                    ui.add(
                                        egui::TextEdit::singleline(&mut self.tran_load_file)
                                            .desired_width(210.0),
                                    );
                                    if ui.button("Browse").clicked() {
                                        if let Some(p) = rfd::FileDialog::new().pick_file() {
                                            self.tran_load_file = p.display().to_string();
                                        }
                                    }
                                });
                            });
                    }
                    AnalysisTab::Ac => {
                        ui.label("Compute small-signal AC behavior over frequency.");
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.label("Type of sweep:");
                            if ui
                                .radio_value(&mut self.ac_sweep, SweepType::Decade, "Decade")
                                .changed()
                                || ui
                                    .radio_value(&mut self.ac_sweep, SweepType::Octave, "Octave")
                                    .changed()
                                || ui
                                    .radio_value(&mut self.ac_sweep, SweepType::Linear, "Linear")
                                    .changed()
                            {
                                changed = true;
                            }
                        });
                        egui::Grid::new("ac_ana_grid").num_columns(2).show(ui, |ui| {
                            ui.label("Number of points:");
                            changed |= ui.text_edit_singleline(&mut self.ac_points).changed();
                            ui.end_row();
                            ui.label("Start frequency:");
                            changed |= ui.text_edit_singleline(&mut self.ac_fstart).changed();
                            ui.end_row();
                            ui.label("Stop frequency:");
                            changed |= ui.text_edit_singleline(&mut self.ac_fstop).changed();
                            ui.end_row();
                        });
                    }
                    AnalysisTab::DcSweep => {
                        ui.label("Compute DC operating point while sweeping a source.");
                        ui.add_space(6.0);
                        egui::Grid::new("dc_ana_grid").num_columns(2).show(ui, |ui| {
                            ui.label("Name of 1st source to sweep:");
                            changed |= ui.text_edit_singleline(&mut self.dc_src).changed();
                            ui.end_row();
                            ui.label("Start value:");
                            changed |= ui.text_edit_singleline(&mut self.dc_start).changed();
                            ui.end_row();
                            ui.label("Stop value:");
                            changed |= ui.text_edit_singleline(&mut self.dc_stop).changed();
                            ui.end_row();
                            ui.label("Increment:");
                            changed |= ui.text_edit_singleline(&mut self.dc_incr).changed();
                            ui.end_row();
                        });
                    }
                    AnalysisTab::Noise => {
                        ui.label("Perform stochastic small-signal noise analysis.");
                        ui.add_space(6.0);
                        egui::Grid::new("noise_ana_grid").num_columns(2).show(ui, |ui| {
                            ui.label("Output:");
                            changed |= ui.text_edit_singleline(&mut self.noise_out).changed();
                            ui.end_row();
                            ui.label("Input:");
                            changed |= ui.text_edit_singleline(&mut self.noise_in).changed();
                            ui.end_row();
                            ui.label("Number of points:");
                            changed |= ui.text_edit_singleline(&mut self.noise_points).changed();
                            ui.end_row();
                            ui.label("Start frequency:");
                            changed |= ui.text_edit_singleline(&mut self.noise_fstart).changed();
                            ui.end_row();
                            ui.label("Stop frequency:");
                            changed |= ui.text_edit_singleline(&mut self.noise_fstop).changed();
                            ui.end_row();
                        });
                    }
                    AnalysisTab::DcTransfer => {
                        ui.label("Find DC small-signal transfer function.");
                        ui.add_space(6.0);
                        egui::Grid::new("tf_ana_grid").num_columns(2).show(ui, |ui| {
                            ui.label("Output:");
                            changed |= ui.text_edit_singleline(&mut self.tf_out).changed();
                            ui.end_row();
                            ui.label("Input source:");
                            changed |= ui.text_edit_singleline(&mut self.tf_in).changed();
                            ui.end_row();
                        });
                    }
                    AnalysisTab::DcOp => {
                        ui.label(
                            "Compute the DC operating point treating capacitances as opens and inductances as shorts.",
                        );
                    }
                }

                if changed {
                    self.rebuild_cmd();
                }

                ui.add_space(8.0);
                ui.label(RichText::new(self.syntax_hint()).monospace().size(11.5));
                ui.add(
                    egui::TextEdit::singleline(&mut self.cmd_text)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY),
                );

                ui.add_space(8.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add_sized(Vec2::new(72.0, 24.0), egui::Button::new("Cancel"))
                        .clicked()
                    {
                        result = Some(false);
                    }
                    if ui
                        .add_sized(Vec2::new(72.0, 24.0), egui::Button::new("OK"))
                        .clicked()
                    {
                        result = Some(true);
                    }
                });
            });

        result
    }
}
