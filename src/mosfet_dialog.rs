use std::collections::HashMap;

use egui::{Align, Frame, Layout, RichText, Stroke, Ui, Vec2};

use crate::settings::ModelIndex;

#[derive(Debug, Clone)]
pub struct MosfetDialogState {
    pub comp_id: u64,
    pub refdes: String,
    pub model_name: String,
    pub manufacturer: String,
    pub polarity: String,
    pub vds: String,
    pub rds_on: String,
    pub qgate: String,
    pub show_picker: bool,
}

impl MosfetDialogState {
    pub fn from_component(
        comp_id: u64,
        refdes: &str,
        value: &str,
        is_pmos: bool,
        params: &HashMap<String, String>,
    ) -> Self {
        let default_pol = if is_pmos { "P-chan" } else { "N-chan" };
        Self {
            comp_id,
            refdes: refdes.to_string(),
            model_name: value.to_string(),
            manufacturer: params.get("mfg").cloned().unwrap_or_default(),
            polarity: params
                .get("pol")
                .cloned()
                .unwrap_or_else(|| default_pol.into()),
            vds: params.get("vds").cloned().unwrap_or_default(),
            rds_on: params.get("rds_on").cloned().unwrap_or_default(),
            qgate: params.get("qgate").cloned().unwrap_or_default(),
            show_picker: false,
        }
    }

    pub fn into_params(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        for (k, v) in [
            ("mfg", &self.manufacturer),
            ("pol", &self.polarity),
            ("vds", &self.vds),
            ("rds_on", &self.rds_on),
            ("qgate", &self.qgate),
        ] {
            if !v.trim().is_empty() {
                m.insert(k.into(), v.trim().to_string());
            }
        }
        m
    }

    pub fn is_pmos(&self) -> bool {
        self.polarity.to_ascii_lowercase().starts_with('p')
    }

    /// Returns `Some(true)` on OK, `Some(false)` on Cancel, `None` while open.
    pub fn show(&mut self, ctx: &egui::Context, model_index: &ModelIndex) -> Option<bool> {
        let mut result = None;
        let title = format!("MOSFET - {}", self.refdes);

        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .default_width(290.0)
            .show(ctx, |ui| {
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    ui.vertical(|ui| {
                        if ui
                            .add_sized(Vec2::new(128.0, 24.0), egui::Button::new("OK"))
                            .clicked()
                        {
                            result = Some(true);
                        }
                        ui.add_space(4.0);
                        if ui
                            .add_sized(Vec2::new(128.0, 24.0), egui::Button::new("Cancel"))
                            .clicked()
                        {
                            result = Some(false);
                        }
                        ui.add_space(4.0);
                        if ui
                            .add_sized(Vec2::new(128.0, 24.0), egui::Button::new("Pick New MOSFET"))
                            .clicked()
                        {
                            self.show_picker = !self.show_picker;
                        }
                    });
                });

                ui.add_space(6.0);
                Frame::none()
                    .stroke(Stroke::new(
                        1.0_f32,
                        egui::Color32::from_rgb(175, 175, 175),
                    ))
                    .inner_margin(egui::Margin::same(8.0))
                    .rounding(3.0)
                    .show(ui, |ui| {
                        ui.label(RichText::new("MOSFET Properties").strong());
                        ui.add_space(4.0);
                        egui::Grid::new("mos_props_grid")
                            .num_columns(2)
                            .spacing([10.0, 4.0])
                            .show(ui, |ui| {
                                prop_row(ui, "MOSFET:", &mut self.model_name);
                                prop_row(ui, "Manufacturer:", &mut self.manufacturer);
                                prop_row(ui, "Polarity:", &mut self.polarity);
                                prop_row(ui, "Vds[V]:", &mut self.vds);
                                prop_row(ui, "Rds(on)[Ω]:", &mut self.rds_on);
                                prop_row(ui, "Qgate[C]:", &mut self.qgate);
                            });
                    });
            });

        if self.show_picker {
            let mut open = true;
            let mos_models = model_index.list_models_for_kinds(&["NMOS", "PMOS"]);
            egui::Window::new("Select MOSFET (from ngspice / User Libraries)")
                .open(&mut open)
                .default_size([440.0, 260.0])
                .resizable(true)
                .show(ctx, |ui| {
                    if mos_models.is_empty() {
                        ui.label("No .model NMOS / PMOS definitions found in configured folders.");
                        ui.label("Add .lib / .mod files in File -> Settings... (User Model Folders).");
                    } else {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            egui::Grid::new("mos_lib_grid")
                                .striped(true)
                                .spacing([12.0, 4.0])
                                .show(ui, |ui| {
                                    ui.strong("Model");
                                    ui.strong("Polarity");
                                    ui.strong("Library File");
                                    ui.label("");
                                    ui.end_row();
                                    for (name, kind, path) in &mos_models {
                                        let pol = if kind.eq_ignore_ascii_case("PMOS") {
                                            "P-chan"
                                        } else {
                                            "N-chan"
                                        };
                                        ui.label(name);
                                        ui.label(pol);
                                        ui.label(
                                            path.file_name()
                                                .and_then(|n| n.to_str())
                                                .unwrap_or(""),
                                        );
                                        if ui.button("Select").clicked() {
                                            self.model_name = name.clone();
                                            self.polarity = pol.into();
                                            self.manufacturer = "Library".into();
                                            self.show_picker = false;
                                        }
                                        ui.end_row();
                                    }
                                });
                        });
                    }
                });
            if !open {
                self.show_picker = false;
            }
        }

        result
    }
}

fn prop_row(ui: &mut Ui, label: &str, value: &mut String) {
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.label(label);
    });
    ui.add(egui::TextEdit::singleline(value).desired_width(95.0));
    ui.end_row();
}
