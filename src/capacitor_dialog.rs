use std::collections::HashMap;
use std::path::PathBuf;

use egui::{Align, Frame, Layout, RichText, Stroke, Ui, Vec2};

use crate::settings::ModelIndex;

#[derive(Debug, Clone)]
pub struct CapacitorDialogState {
    pub comp_id: u64,
    pub refdes: String,
    pub manufacturer: String,
    pub part_number: String,
    pub cap_type: String,
    pub capacitance: String,
    pub voltage_rating: String,
    pub rms_current: String,
    pub esr: String,
    pub esl: String,
    pub rpar: String,
    pub cpar: String,
    pub show_picker: bool,
    pub status: String,
}

pub enum CapacitorDialogAction {
    None,
    Ok,
    Cancel,
    SavedToLibrary,
}

impl CapacitorDialogState {
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
            cap_type: params.get("type").cloned().unwrap_or_else(|| "---------".into()),
            capacitance: value.to_string(),
            voltage_rating: params.get("v").cloned().unwrap_or_default(),
            rms_current: params.get("irms").cloned().unwrap_or_default(),
            esr: params.get("esr").cloned().unwrap_or_default(),
            esl: params.get("esl").cloned().unwrap_or_default(),
            rpar: params.get("rpar").cloned().unwrap_or_default(),
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
        if self.cap_type != "---------" && !self.cap_type.trim().is_empty() {
            m.insert("type".into(), self.cap_type.trim().to_string());
        }
        for (k, v) in [
            ("v", &self.voltage_rating),
            ("irms", &self.rms_current),
            ("esr", &self.esr),
            ("esl", &self.esl),
            ("rpar", &self.rpar),
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
            format!("C_{}", self.capacitance.trim().replace(' ', "_"))
        } else {
            pn_raw.replace(' ', "_")
        };
        if pn.is_empty() {
            return Err("Enter a Part Number or Capacitance first".into());
        }
        let cap_val = self
            .capacitance
            .trim()
            .replace('µ', "u")
            .replace('μ', "u");
        if cap_val.is_empty() {
            return Err("Capacitance[F] cannot be empty".into());
        }

        let dir = user_dir
            .cloned()
            .unwrap_or_else(|| PathBuf::from("./models"));
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create {}: {e}", dir.display()))?;
        let file_path = dir.join("user_capacitors.lib");

        let mut block = format!(
            "\n* User Capacitor: {pn} mfg={} type={} v={} irms={}\n",
            self.manufacturer.trim(),
            self.cap_type.trim(),
            self.voltage_rating.trim(),
            self.rms_current.trim()
        );
        block.push_str(&format!(".subckt {pn} p1 p2\n"));
        let mut cur = "p1".to_string();
        if !self.esl.trim().is_empty() {
            block.push_str(&format!("Lser {cur} n_esl {}\n", self.esl.trim()));
            cur = "n_esl".to_string();
        }
        if !self.esr.trim().is_empty() {
            block.push_str(&format!("Rser {cur} n_esr {}\n", self.esr.trim()));
            cur = "n_esr".to_string();
        }
        block.push_str(&format!("Cmain {cur} p2 {cap_val}\n"));
        if !self.rpar.trim().is_empty() {
            block.push_str(&format!("Rpar p1 p2 {}\n", self.rpar.trim()));
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
    ) -> CapacitorDialogAction {
        let mut action = CapacitorDialogAction::None;
        let title = format!("Capacitor - {}", self.refdes);

        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .default_width(350.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        egui::Grid::new("cap_meta_grid")
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
                                    egui::TextEdit::singleline(&mut self.cap_type)
                                        .desired_width(110.0),
                                );
                                ui.end_row();
                            });

                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            if ui.button("Select Capacitor").clicked() {
                                self.show_picker = !self.show_picker;
                            }
                            if ui.button("Save to Library").clicked() {
                                match self.save_to_user_library(user_dir) {
                                    Ok(p) => {
                                        self.status = format!("Saved to {}", p.display());
                                        action = CapacitorDialogAction::SavedToLibrary;
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
                                action = CapacitorDialogAction::Ok;
                            }
                            ui.add_space(4.0);
                            if ui
                                .add_sized(Vec2::new(72.0, 24.0), egui::Button::new("Cancel"))
                                .clicked()
                            {
                                action = CapacitorDialogAction::Cancel;
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
                        ui.label(RichText::new("Capacitor Properties").strong());
                        ui.add_space(4.0);
                        egui::Grid::new("cap_props_grid")
                            .num_columns(2)
                            .spacing([10.0, 4.0])
                            .show(ui, |ui| {
                                prop_row(ui, "Capacitance[F]:", &mut self.capacitance);
                                prop_row(ui, "Voltage Rating[V]:", &mut self.voltage_rating);
                                prop_row(ui, "RMS Current Rating[A]:", &mut self.rms_current);
                                prop_row(ui, "Equiv. Series Resistance[Ω]:", &mut self.esr);
                                prop_row(ui, "Equiv. Series Inductance[H]:", &mut self.esl);
                                prop_row(ui, "Equiv. Parallel Resistance[Ω]:", &mut self.rpar);
                                prop_row(ui, "Equiv. Parallel Capacitance[F]:", &mut self.cpar);
                            });
                    });

                if !self.status.is_empty() {
                    ui.add_space(4.0);
                    ui.label(&self.status);
                }
            });

        if self.show_picker {
            let mut open = true;
            let cap_models = model_index.list_models_for_kinds(&["C"]);
            let subckts = model_index.list_subckts();
            egui::Window::new("Select Capacitor (from ngspice / User Libraries)")
                .open(&mut open)
                .default_size([460.0, 260.0])
                .resizable(true)
                .show(ctx, |ui| {
                    if cap_models.is_empty() && subckts.is_empty() {
                        ui.label("No capacitor .model C or .subckt models found in configured folders.");
                        ui.label("Add .lib / .mod files or click 'Save to Library' to create one.");
                    } else {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            egui::Grid::new("cap_lib_grid")
                                .striped(true)
                                .spacing([12.0, 4.0])
                                .show(ui, |ui| {
                                    ui.strong("Model / Subckt");
                                    ui.strong("Kind");
                                    ui.strong("Library File");
                                    ui.label("");
                                    ui.end_row();
                                    for (name, kind, path) in &cap_models {
                                        ui.label(name);
                                        ui.label(kind);
                                        ui.label(
                                            path.file_name()
                                                .and_then(|n| n.to_str())
                                                .unwrap_or(""),
                                        );
                                        if ui.button("Select").clicked() {
                                            self.part_number = name.clone();
                                            self.cap_type = kind.clone();
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
                                            self.cap_type = "SUBCKT".into();
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
