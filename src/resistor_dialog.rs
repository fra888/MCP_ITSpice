use std::collections::HashMap;
use std::path::PathBuf;

use egui::{Align, Frame, Layout, RichText, Stroke, Ui, Vec2};

use crate::settings::ModelIndex;

#[derive(Debug, Clone)]
pub struct ResistorDialogState {
    pub comp_id: u64,
    pub refdes: String,
    pub manufacturer: String,
    pub part_number: String,
    pub res_type: String,
    pub resistance: String,
    pub tolerance: String,
    pub power_rating: String,
    pub lser: String,
    pub cpar: String,
    pub show_picker: bool,
    pub status: String,
}

pub enum ResistorDialogAction {
    None,
    Ok,
    Cancel,
    SavedToLibrary,
}

impl ResistorDialogState {
    pub fn from_component(
        comp_id: u64,
        refdes: &str,
        value: &str,
        params: &HashMap<String, String>,
    ) -> Self {
        Self {
            comp_id,
            refdes: refdes.to_string(),
            manufacturer: params.get("mfg").cloned().unwrap_or_else(|| "---------".into()),
            part_number: params.get("pn").cloned().unwrap_or_else(|| "---------".into()),
            res_type: params.get("type").cloned().unwrap_or_else(|| "---------".into()),
            resistance: value.to_string(),
            tolerance: params.get("tol").cloned().unwrap_or_default(),
            power_rating: params.get("pwr").cloned().unwrap_or_default(),
            lser: params.get("lser").cloned().unwrap_or_default(),
            cpar: params.get("cpar").cloned().unwrap_or_default(),
            show_picker: false,
            status: String::new(),
        }
    }

    pub fn into_params(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        if self.manufacturer != "---------" && !self.manufacturer.trim().is_empty() {
            m.insert("mfg".into(), self.manufacturer.trim().to_string());
        }
        if self.part_number != "---------" && !self.part_number.trim().is_empty() {
            m.insert("pn".into(), self.part_number.trim().to_string());
        }
        if self.res_type != "---------" && !self.res_type.trim().is_empty() {
            m.insert("type".into(), self.res_type.trim().to_string());
        }
        for (k, v) in [
            ("tol", &self.tolerance),
            ("pwr", &self.power_rating),
            ("lser", &self.lser),
            ("cpar", &self.cpar),
        ] {
            if !v.trim().is_empty() {
                m.insert(k.into(), v.trim().to_string());
            }
        }
        m
    }

    fn save_to_user_library(&mut self, user_dir: Option<&PathBuf>) -> Result<PathBuf, String> {
        let pn_raw = self.part_number.trim();
        let pn = if pn_raw.is_empty() || pn_raw == "---------" {
            format!("R_{}", self.resistance.trim().replace(' ', "_"))
        } else {
            pn_raw.replace(' ', "_")
        };
        if pn.is_empty() {
            return Err("Enter a Part Number or Resistance first".into());
        }
        let r_val = self.resistance.trim();
        if r_val.is_empty() {
            return Err("Resistance[Ω] cannot be empty".into());
        }

        let dir = user_dir
            .cloned()
            .unwrap_or_else(|| PathBuf::from("./models"));
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create {}: {e}", dir.display()))?;
        let file_path = dir.join("user_resistors.lib");

        let mut block = format!(
            "\n* User Resistor: {pn} mfg={} type={} tol={} pwr={}\n",
            self.manufacturer.trim(),
            self.res_type.trim(),
            self.tolerance.trim(),
            self.power_rating.trim()
        );
        block.push_str(&format!(".subckt {pn} p1 p2\n"));
        if !self.lser.trim().is_empty() {
            block.push_str(&format!("Lser p1 n_int {}\n", self.lser.trim()));
            block.push_str(&format!("Rmain n_int p2 {r_val}\n"));
        } else {
            block.push_str(&format!("Rmain p1 p2 {r_val}\n"));
        }
        if !self.cpar.trim().is_empty() {
            block.push_str(&format!("Cpar p1 p2 {}\n", self.cpar.trim()));
        }
        block.push_str(&format!(".ends {pn}\n"));

        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .map_err(|e| format!("Failed to open {}: {e}", file_path.display()))?;
        f.write_all(block.as_bytes())
            .map_err(|e| format!("Failed to write {}: {e}", file_path.display()))?;

        self.part_number = pn;
        Ok(file_path)
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        model_index: &ModelIndex,
        user_dir: Option<&PathBuf>,
    ) -> ResistorDialogAction {
        let mut action = ResistorDialogAction::None;
        let title = format!("Resistor - {}", self.refdes);

        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .default_width(350.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        egui::Grid::new("res_meta_grid")
                            .num_columns(2)
                            .spacing([8.0, 3.0])
                            .show(ui, |ui| {
                                ui.label("Manufacturer:");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.manufacturer)
                                        .desired_width(110.0),
                                );
                                ui.end_row();

                                ui.label("Part Number:");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.part_number)
                                        .desired_width(110.0),
                                );
                                ui.end_row();

                                ui.label("Type:");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.res_type)
                                        .desired_width(110.0),
                                );
                                ui.end_row();
                            });

                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            if ui.button("Select Resistor").clicked() {
                                self.show_picker = !self.show_picker;
                            }
                            if ui.button("Save to Library").clicked() {
                                match self.save_to_user_library(user_dir) {
                                    Ok(p) => {
                                        self.status = format!("Saved to {}", p.display());
                                        action = ResistorDialogAction::SavedToLibrary;
                                    }
                                    Err(e) => self.status = e,
                                }
                            }
                        });
                    });

                    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                        ui.vertical(|ui| {
                            if ui
                                .add_sized(Vec2::new(72.0, 24.0), egui::Button::new("OK"))
                                .clicked()
                            {
                                action = ResistorDialogAction::Ok;
                            }
                            ui.add_space(4.0);
                            if ui
                                .add_sized(Vec2::new(72.0, 24.0), egui::Button::new("Cancel"))
                                .clicked()
                            {
                                action = ResistorDialogAction::Cancel;
                            }
                        });
                    });
                });

                ui.add_space(8.0);
                Frame::none()
                    .stroke(Stroke::new(
                        1.0_f32,
                        egui::Color32::from_rgb(175, 175, 175),
                    ))
                    .inner_margin(egui::Margin::same(8.0))
                    .rounding(3.0)
                    .show(ui, |ui| {
                        ui.label(RichText::new("Resistor Properties").strong());
                        ui.add_space(4.0);
                        egui::Grid::new("res_props_grid")
                            .num_columns(2)
                            .spacing([10.0, 4.0])
                            .show(ui, |ui| {
                                prop_row(ui, "Resistance[Ω]:", &mut self.resistance);
                                prop_row(ui, "Tolerance[%]:", &mut self.tolerance);
                                prop_row(ui, "Power Rating[W]:", &mut self.power_rating);
                                prop_row(ui, "Series Inductance[H]:", &mut self.lser);
                                prop_row(ui, "Parallel Capacitance[F]:", &mut self.cpar);
                            });
                    });

                if !self.status.is_empty() {
                    ui.add_space(4.0);
                    ui.label(&self.status);
                }
            });

        if self.show_picker {
            let mut open = true;
            let res_models = model_index.list_models_for_kinds(&["R"]);
            let subckts = model_index.list_subckts();
            egui::Window::new("Select Resistor (from ngspice / User Libraries)")
                .open(&mut open)
                .default_size([460.0, 260.0])
                .resizable(true)
                .show(ctx, |ui| {
                    if res_models.is_empty() && subckts.is_empty() {
                        ui.label("No resistor .model R or .subckt models found in configured folders.");
                        ui.label("Add .lib / .mod files or click 'Save to Library' to create one.");
                    } else {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            egui::Grid::new("res_lib_grid")
                                .striped(true)
                                .spacing([12.0, 4.0])
                                .show(ui, |ui| {
                                    ui.strong("Model / Subckt");
                                    ui.strong("Kind");
                                    ui.strong("Library File");
                                    ui.label("");
                                    ui.end_row();
                                    for (name, kind, path) in &res_models {
                                        ui.label(name);
                                        ui.label(kind);
                                        ui.label(
                                            path.file_name()
                                                .and_then(|n| n.to_str())
                                                .unwrap_or(""),
                                        );
                                        if ui.button("Select").clicked() {
                                            self.part_number = name.clone();
                                            self.res_type = kind.clone();
                                            self.manufacturer = "Library".into();
                                            self.show_picker = false;
                                        }
                                        ui.end_row();
                                    }
                                    for (name, path) in &subckts {
                                        ui.label(name);
                                        ui.label("SUBCKT");
                                        ui.label(
                                            path.file_name()
                                                .and_then(|n| n.to_str())
                                                .unwrap_or(""),
                                        );
                                        if ui.button("Select").clicked() {
                                            self.part_number = name.clone();
                                            self.res_type = "SUBCKT".into();
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

        action
    }
}

fn prop_row(ui: &mut Ui, label: &str, value: &mut String) {
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.label(label);
    });
    ui.add(egui::TextEdit::singleline(value).desired_width(95.0));
    ui.end_row();
}
