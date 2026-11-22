use egui::{Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use serde::{Deserialize, Serialize};

use crate::model::GRID_SIZE;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolPin {
    pub name: String,
    pub order: usize,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SymbolShape {
    Line { x1: f32, y1: f32, x2: f32, y2: f32 },
    Rect { x1: f32, y1: f32, x2: f32, y2: f32 },
    Circle { cx: f32, cy: f32, r: f32 },
    Text { x: f32, y: f32, text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomSymbol {
    pub name: String,
    pub prefix: String,
    pub default_value: String,
    pub pins: Vec<SymbolPin>,
    pub shapes: Vec<SymbolShape>,
}

impl Default for CustomSymbol {
    fn default() -> Self {
        Self {
            name: "NewSymbol".into(),
            prefix: "X".into(),
            default_value: "NEW_SUBCKT".into(),
            pins: Vec::new(),
            shapes: vec![SymbolShape::Rect {
                x1: -2.0,
                y1: -2.0,
                x2: 2.0,
                y2: 2.0,
            }],
        }
    }
}

impl CustomSymbol {
    pub fn to_asy(&self) -> String {
        let mut out = String::from("Version 4\nSymbolType BLOCK\n");
        for s in &self.shapes {
            match s {
                SymbolShape::Line { x1, y1, x2, y2 } => {
                    out.push_str(&format!(
                        "LINE Normal {} {} {} {}\n",
                        (*x1 * 16.0) as i32,
                        (*y1 * 16.0) as i32,
                        (*x2 * 16.0) as i32,
                        (*y2 * 16.0) as i32
                    ));
                }
                SymbolShape::Rect { x1, y1, x2, y2 } => {
                    out.push_str(&format!(
                        "RECTANGLE Normal {} {} {} {}\n",
                        (*x1 * 16.0) as i32,
                        (*y1 * 16.0) as i32,
                        (*x2 * 16.0) as i32,
                        (*y2 * 16.0) as i32
                    ));
                }
                SymbolShape::Circle { cx, cy, r } => {
                    out.push_str(&format!(
                        "CIRCLE Normal {} {} {} {}\n",
                        ((*cx - *r) * 16.0) as i32,
                        ((*cy - *r) * 16.0) as i32,
                        ((*cx + *r) * 16.0) as i32,
                        ((*cy + *r) * 16.0) as i32
                    ));
                }
                SymbolShape::Text { x, y, text } => {
                    out.push_str(&format!(
                        "TEXT {} {} Left 2 {text}\n",
                        (*x * 16.0) as i32,
                        (*y * 16.0) as i32
                    ));
                }
            }
        }
        out.push_str(&format!("SYMATTR Prefix {}\n", self.prefix));
        out.push_str(&format!("SYMATTR Value {}\n", self.default_value));
        for p in &self.pins {
            out.push_str(&format!("PIN {} {} NONE 8\n", p.x * 16, p.y * 16));
            out.push_str(&format!("PINATTR PinName {}\n", p.name));
            out.push_str(&format!("PINATTR SpiceOrder {}\n", p.order));
        }
        out
    }

    pub fn from_asy(name: &str, content: &str) -> Self {
        let mut sym = Self {
            name: name.trim_end_matches(".asy").to_string(),
            prefix: "X".into(),
            default_value: name.trim_end_matches(".asy").to_string(),
            pins: Vec::new(),
            shapes: Vec::new(),
        };

        let mut current_pin: Option<SymbolPin> = None;

        for raw in content.lines() {
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }
            match parts[0] {
                "LINE" if parts.len() >= 6 => {
                    if let (Ok(x1), Ok(y1), Ok(x2), Ok(y2)) = (
                        parts[2].parse::<f32>(),
                        parts[3].parse::<f32>(),
                        parts[4].parse::<f32>(),
                        parts[5].parse::<f32>(),
                    ) {
                        sym.shapes.push(SymbolShape::Line {
                            x1: x1 / 16.0,
                            y1: y1 / 16.0,
                            x2: x2 / 16.0,
                            y2: y2 / 16.0,
                        });
                    }
                }
                "RECTANGLE" if parts.len() >= 6 => {
                    if let (Ok(x1), Ok(y1), Ok(x2), Ok(y2)) = (
                        parts[2].parse::<f32>(),
                        parts[3].parse::<f32>(),
                        parts[4].parse::<f32>(),
                        parts[5].parse::<f32>(),
                    ) {
                        sym.shapes.push(SymbolShape::Rect {
                            x1: x1 / 16.0,
                            y1: y1 / 16.0,
                            x2: x2 / 16.0,
                            y2: y2 / 16.0,
                        });
                    }
                }
                "CIRCLE" if parts.len() >= 6 => {
                    if let (Ok(x1), Ok(y1), Ok(x2), Ok(y2)) = (
                        parts[2].parse::<f32>(),
                        parts[3].parse::<f32>(),
                        parts[4].parse::<f32>(),
                        parts[5].parse::<f32>(),
                    ) {
                        let cx = (x1 + x2) / 32.0;
                        let cy = (y1 + y2) / 32.0;
                        let r = ((x2 - x1).abs().max((y2 - y1).abs()) / 32.0).max(0.25);
                        sym.shapes.push(SymbolShape::Circle { cx, cy, r });
                    }
                }
                "TEXT" if parts.len() >= 6 => {
                    if let (Ok(x), Ok(y)) = (parts[1].parse::<f32>(), parts[2].parse::<f32>()) {
                        let txt = parts[5..].join(" ");
                        sym.shapes.push(SymbolShape::Text {
                            x: x / 16.0,
                            y: y / 16.0,
                            text: txt,
                        });
                    }
                }
                "SYMATTR" if parts.len() >= 3 => {
                    let attr = parts[1];
                    let val = parts[2..].join(" ");
                    if attr == "Prefix" {
                        sym.prefix = val;
                    } else if attr == "Value" || attr == "SpiceModel" {
                        sym.default_value = val;
                    }
                }
                "PIN" if parts.len() >= 3 => {
                    if let Some(p) = current_pin.take() {
                        sym.pins.push(p);
                    }
                    let x = (parts[1].parse::<f32>().unwrap_or(0.0) / 16.0).round() as i32;
                    let y = (parts[2].parse::<f32>().unwrap_or(0.0) / 16.0).round() as i32;
                    let order = sym.pins.len() + 1;
                    current_pin = Some(SymbolPin {
                        name: format!("{order}"),
                        order,
                        x,
                        y,
                    });
                }
                "PINATTR" if parts.len() >= 3 => {
                    if let Some(p) = current_pin.as_mut() {
                        if parts[1] == "PinName" {
                            p.name = parts[2..].join(" ");
                        } else if parts[1] == "SpiceOrder" {
                            if let Ok(ord) = parts[2].parse::<usize>() {
                                p.order = ord;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        if let Some(p) = current_pin.take() {
            sym.pins.push(p);
        }
        sym.pins.sort_by_key(|p| p.order);
        sym
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolTool {
    Pin,
    Line,
    Rect,
    Circle,
    Text,
    Delete,
}

pub enum SymbolEditorAction {
    None,
    SaveAndUse(CustomSymbol),
}

pub struct SymbolEditorState {
    pub open: bool,
    pub symbol: CustomSymbol,
    pub tool: SymbolTool,
    pub drag_start: Option<(f32, f32)>,
    pub next_pin_name: String,
    pub next_text: String,
    pub undo_stack: Vec<CustomSymbol>,
    pub zoom: f32,
    pub status: String,
}

impl Default for SymbolEditorState {
    fn default() -> Self {
        Self {
            open: false,
            symbol: CustomSymbol::default(),
            tool: SymbolTool::Pin,
            drag_start: None,
            next_pin_name: "P1".into(),
            next_text: "LBL".into(),
            undo_stack: Vec::new(),
            zoom: 1.5,
            status: "Place pins and draw graphics around origin (0,0)".into(),
        }
    }
}

impl SymbolEditorState {
    pub fn reset_new(&mut self) {
        self.symbol = CustomSymbol::default();
        self.drag_start = None;
        self.next_pin_name = "1".into();
        self.undo_stack.clear();
        self.open = true;
        self.status = "New symbol workbench ready".into();
    }

    pub fn show(&mut self, ctx: &egui::Context) -> SymbolEditorAction {
        if !self.open {
            return SymbolEditorAction::None;
        }
        let mut action = SymbolEditorAction::None;
        let mut open = self.open;

        egui::Window::new(format!("Symbol Workbench - {}", self.symbol.name))
            .id(egui::Id::new("symbol_workbench_window"))
            .open(&mut open)
            .default_size([780.0, 540.0])
            .min_size([480.0, 340.0])
            .resizable([true, true])
            .show(ctx, |ui| {
                // Toolbar
                egui::TopBottomPanel::top("sym_wb_toolbar").show_inside(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.selectable_value(&mut self.tool, SymbolTool::Pin, "Pin");
                        ui.selectable_value(&mut self.tool, SymbolTool::Line, "Line");
                        ui.selectable_value(&mut self.tool, SymbolTool::Rect, "Rect");
                        ui.selectable_value(&mut self.tool, SymbolTool::Circle, "Circle");
                        ui.selectable_value(&mut self.tool, SymbolTool::Text, "Text");
                        ui.selectable_value(&mut self.tool, SymbolTool::Delete, "Delete");
                        ui.separator();
                        if ui
                            .add_enabled(!self.undo_stack.is_empty(), egui::Button::new("Undo"))
                            .clicked()
                        {
                            if let Some(prev) = self.undo_stack.pop() {
                                self.symbol = prev;
                            }
                        }
                        if ui.button("Clear").clicked() {
                            self.undo_stack.push(self.symbol.clone());
                            self.symbol.pins.clear();
                            self.symbol.shapes.clear();
                        }
                        ui.separator();
                        if ui.button("Save Symbol...").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .set_file_name(format!("{}.asy", self.symbol.name))
                                .add_filter("ASCII Symbol (.asy)", &["asy"])
                                .add_filter("Symbol JSON (.sym.json)", &["json"])
                                .save_file()
                            {
                                let content =
                                    if path.extension().and_then(|e| e.to_str()) == Some("json") {
                                        serde_json::to_string_pretty(&self.symbol)
                                            .unwrap_or_default()
                                    } else {
                                        self.symbol.to_asy()
                                    };
                                match std::fs::write(&path, content) {
                                    Ok(()) => self.status = format!("Saved to {}", path.display()),
                                    Err(e) => self.status = format!("Save error: {e}"),
                                }
                            }
                        }
                        if ui.button("Open Symbol...").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("Symbol (.asy / .json)", &["asy", "json"])
                                .pick_file()
                            {
                                if let Ok(content) = std::fs::read_to_string(&path) {
                                    let fname = path
                                        .file_name()
                                        .and_then(|n| n.to_str())
                                        .unwrap_or("symbol.asy");
                                    let sym_opt =
                                        if path.extension().and_then(|e| e.to_str()) == Some("json")
                                        {
                                            serde_json::from_str::<CustomSymbol>(&content).ok()
                                        } else {
                                            Some(CustomSymbol::from_asy(fname, &content))
                                        };
                                    if let Some(sym) = sym_opt {
                                        self.undo_stack.push(self.symbol.clone());
                                        self.symbol = sym;
                                        self.status = format!("Loaded {}", self.symbol.name);
                                    }
                                }
                            }
                        }
                        if ui.button("Add to Schematic").clicked() {
                            action = SymbolEditorAction::SaveAndUse(self.symbol.clone());
                            self.open = false;
                        }
                    });
                });

                // Bottom Status Bar
                egui::TopBottomPanel::bottom("sym_wb_status").show_inside(ui, |ui| {
                    ui.label(&self.status);
                });

                // Left Sidebar: Symbol Properties & Pin Table
                egui::SidePanel::left("sym_wb_sidebar")
                    .resizable(true)
                    .default_width(230.0)
                    .show_inside(ui, |ui| {
                        ui.strong("Symbol Attributes");
                        egui::Grid::new("sym_attr_grid")
                            .num_columns(2)
                            .spacing([6.0, 4.0])
                            .show(ui, |ui| {
                                ui.label("Name:");
                                ui.text_edit_singleline(&mut self.symbol.name);
                                ui.end_row();
                                ui.label("SPICE Prefix:");
                                ui.text_edit_singleline(&mut self.symbol.prefix);
                                ui.end_row();
                                ui.label("Value/Model:");
                                ui.text_edit_singleline(&mut self.symbol.default_value);
                                ui.end_row();
                            });

                        ui.add_space(6.0);
                        match self.tool {
                            SymbolTool::Pin => {
                                ui.horizontal(|ui| {
                                    ui.label("Next Pin Name:");
                                    ui.add(
                                        egui::TextEdit::singleline(&mut self.next_pin_name)
                                            .desired_width(80.0),
                                    );
                                });
                            }
                            SymbolTool::Text => {
                                ui.horizontal(|ui| {
                                    ui.label("Text:");
                                    ui.add(
                                        egui::TextEdit::singleline(&mut self.next_text)
                                            .desired_width(120.0),
                                    );
                                });
                            }
                            _ => {}
                        }

                        ui.add_space(8.0);
                        ui.separator();
                        ui.strong(format!("Pins ({})", self.symbol.pins.len()));
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            let mut remove_idx = None;
                            for (i, p) in self.symbol.pins.iter_mut().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.label(format!("#{}", p.order));
                                    ui.add(
                                        egui::TextEdit::singleline(&mut p.name)
                                            .desired_width(60.0),
                                    );
                                    ui.label(format!("({},{})", p.x, p.y));
                                    if ui.small_button("x").clicked() {
                                        remove_idx = Some(i);
                                    }
                                });
                            }
                            if let Some(i) = remove_idx {
                                self.undo_stack.push(self.symbol.clone());
                                self.symbol.pins.remove(i);
                                for (idx, p) in self.symbol.pins.iter_mut().enumerate() {
                                    p.order = idx + 1;
                                }
                            }
                        });
                    });

                // Center: Symbol Drawing Canvas
                egui::CentralPanel::default()
                    .frame(egui::Frame::none())
                    .show_inside(ui, |ui| {
                        self.draw_canvas(ui);
                    });
            });

        if !open {
            self.open = false;
        }
        action
    }

    fn draw_canvas(&mut self, ui: &mut Ui) {
        let avail = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(avail, Sense::click_and_drag());
        let painter = ui.painter_at(rect);

        // Classic gray canvas
        painter.rect_filled(rect, 0.0, Color32::from_rgb(192, 192, 192));

        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.1 {
                let factor = if scroll > 0.0 { 1.1 } else { 1.0 / 1.1 };
                self.zoom = (self.zoom * factor).clamp(0.5, 3.5);
            }
        }

        let center = rect.center();
        let step = GRID_SIZE * self.zoom;
        let to_screen = |gx: f32, gy: f32| -> Pos2 {
            Pos2::new(center.x + gx * step, center.y + gy * step)
        };
        let to_grid_f = |pos: Pos2| -> (f32, f32) {
            let gx = ((pos.x - center.x) / step * 2.0).round() / 2.0;
            let gy = ((pos.y - center.y) / step * 2.0).round() / 2.0;
            (gx, gy)
        };
        let to_grid_i = |pos: Pos2| -> (i32, i32) {
            (
                ((pos.x - center.x) / step).round() as i32,
                ((pos.y - center.y) / step).round() as i32,
            )
        };

        // Grid dots
        let half_cols = (rect.width() / (2.0 * step)).ceil() as i32;
        let half_rows = (rect.height() / (2.0 * step)).ceil() as i32;
        let dot_color = Color32::from_rgb(150, 150, 150);
        for gx in -half_cols..=half_cols {
            for gy in -half_rows..=half_rows {
                let p = to_screen(gx as f32, gy as f32);
                painter.rect_filled(Rect::from_center_size(p, Vec2::splat(1.4)), 0.0, dot_color);
            }
        }

        // Origin crosshair
        let axis_stroke = Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(80, 80, 80, 120));
        painter.line_segment(
            [Pos2::new(center.x, rect.top()), Pos2::new(center.x, rect.bottom())],
            axis_stroke,
        );
        painter.line_segment(
            [Pos2::new(rect.left(), center.y), Pos2::new(rect.right(), center.y)],
            axis_stroke,
        );

        // Draw existing shapes
        let sym_color = Color32::from_rgb(0, 0, 150);
        let stroke = Stroke::new(1.8_f32, sym_color);
        for s in &self.symbol.shapes {
            match s {
                SymbolShape::Line { x1, y1, x2, y2 } => {
                    painter.line_segment([to_screen(*x1, *y1), to_screen(*x2, *y2)], stroke);
                }
                SymbolShape::Rect { x1, y1, x2, y2 } => {
                    let r = Rect::from_two_pos(to_screen(*x1, *y1), to_screen(*x2, *y2));
                    painter.rect_stroke(r, 0.0, stroke);
                }
                SymbolShape::Circle { cx, cy, r } => {
                    painter.circle_stroke(to_screen(*cx, *cy), *r * step, stroke);
                }
                SymbolShape::Text { x, y, text } => {
                    painter.text(
                        to_screen(*x, *y),
                        egui::Align2::LEFT_CENTER,
                        text,
                        FontId::proportional(13.0),
                        Color32::BLACK,
                    );
                }
            }
        }

        // Draw existing pins
        for p in &self.symbol.pins {
            let sp = to_screen(p.x as f32, p.y as f32);
            painter.rect_stroke(
                Rect::from_center_size(sp, Vec2::splat(8.0)),
                0.0,
                Stroke::new(1.5_f32, Color32::from_rgb(200, 0, 0)),
            );
            painter.text(
                sp + Vec2::new(7.0, -7.0),
                egui::Align2::LEFT_BOTTOM,
                format!("{} ({})", p.name, p.order),
                FontId::monospace(11.5),
                Color32::from_rgb(160, 0, 0),
            );
        }

        // Rubber-band preview & click handling
        if let Some(hover) = response.hover_pos() {
            let (gx, gy) = to_grid_f(hover);
            let (ix, iy) = to_grid_i(hover);

            if let Some((sx, sy)) = self.drag_start {
                let preview_stroke =
                    Stroke::new(1.5_f32, Color32::from_rgba_unmultiplied(0, 0, 200, 160));
                match self.tool {
                    SymbolTool::Line => {
                        painter.line_segment(
                            [to_screen(sx, sy), to_screen(gx, gy)],
                            preview_stroke,
                        );
                    }
                    SymbolTool::Rect => {
                        let r = Rect::from_two_pos(to_screen(sx, sy), to_screen(gx, gy));
                        painter.rect_stroke(r, 0.0, preview_stroke);
                    }
                    SymbolTool::Circle => {
                        let rad = ((gx - sx).powi(2) + (gy - sy).powi(2)).sqrt();
                        painter.circle_stroke(to_screen(sx, sy), rad * step, preview_stroke);
                    }
                    _ => {}
                }
            } else if self.tool == SymbolTool::Pin {
                painter.rect_stroke(
                    Rect::from_center_size(to_screen(ix as f32, iy as f32), Vec2::splat(8.0)),
                    0.0,
                    Stroke::new(1.0_f32, Color32::RED),
                );
            }

            if response.secondary_clicked() {
                self.drag_start = None;
            }

            if response.clicked() {
                match self.tool {
                    SymbolTool::Pin => {
                        self.undo_stack.push(self.symbol.clone());
                        let order = self.symbol.pins.len() + 1;
                        let name = if self.next_pin_name.trim().is_empty() {
                            format!("{order}")
                        } else {
                            self.next_pin_name.trim().to_string()
                        };
                        self.symbol.pins.push(SymbolPin {
                            name,
                            order,
                            x: ix,
                            y: iy,
                        });
                        // Auto-increment numeric pin name
                        if let Ok(num) = self.next_pin_name.trim().parse::<usize>() {
                            self.next_pin_name = format!("{}", num + 1);
                        } else {
                            self.next_pin_name = format!("P{}", order + 1);
                        }
                    }
                    SymbolTool::Line | SymbolTool::Rect | SymbolTool::Circle => {
                        if let Some((sx, sy)) = self.drag_start {
                            self.undo_stack.push(self.symbol.clone());
                            match self.tool {
                                SymbolTool::Line => {
                                    self.symbol.shapes.push(SymbolShape::Line {
                                        x1: sx,
                                        y1: sy,
                                        x2: gx,
                                        y2: gy,
                                    });
                                }
                                SymbolTool::Rect => {
                                    self.symbol.shapes.push(SymbolShape::Rect {
                                        x1: sx,
                                        y1: sy,
                                        x2: gx,
                                        y2: gy,
                                    });
                                }
                                SymbolTool::Circle => {
                                    let r = ((gx - sx).powi(2) + (gy - sy).powi(2))
                                        .sqrt()
                                        .max(0.5);
                                    self.symbol.shapes.push(SymbolShape::Circle {
                                        cx: sx,
                                        cy: sy,
                                        r,
                                    });
                                }
                                _ => {}
                            }
                            self.drag_start = None;
                        } else {
                            self.drag_start = Some((gx, gy));
                        }
                    }
                    SymbolTool::Text => {
                        self.undo_stack.push(self.symbol.clone());
                        self.symbol.shapes.push(SymbolShape::Text {
                            x: gx,
                            y: gy,
                            text: self.next_text.clone(),
                        });
                    }
                    SymbolTool::Delete => {
                        if let Some(pidx) = self
                            .symbol
                            .pins
                            .iter()
                            .position(|p| (p.x - ix).abs() <= 1 && (p.y - iy).abs() <= 1)
                        {
                            self.undo_stack.push(self.symbol.clone());
                            self.symbol.pins.remove(pidx);
                            for (idx, p) in self.symbol.pins.iter_mut().enumerate() {
                                p.order = idx + 1;
                            }
                        } else if let Some(sidx) = self.symbol.shapes.iter().position(|s| match s {
                            SymbolShape::Line { x1, y1, x2, y2 }
                            | SymbolShape::Rect { x1, y1, x2, y2 } => {
                                let min_x = x1.min(*x2) - 0.5;
                                let max_x = x1.max(*x2) + 0.5;
                                let min_y = y1.min(*y2) - 0.5;
                                let max_y = y1.max(*y2) + 0.5;
                                gx >= min_x && gx <= max_x && gy >= min_y && gy <= max_y
                            }
                            SymbolShape::Circle { cx, cy, r } => {
                                ((gx - cx).powi(2) + (gy - cy).powi(2)).sqrt() <= r + 0.5
                            }
                            SymbolShape::Text { x, y, .. } => {
                                (gx - x).abs() <= 1.5 && (gy - y).abs() <= 1.0
                            }
                        }) {
                            self.undo_stack.push(self.symbol.clone());
                            self.symbol.shapes.remove(sidx);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asy_roundtrip() {
        let mut sym = CustomSymbol::default();
        sym.name = "TEST_IC".into();
        sym.prefix = "X".into();
        sym.default_value = "MY_IC".into();
        sym.pins.push(SymbolPin {
            name: "IN".into(),
            order: 1,
            x: -2,
            y: 0,
        });
        sym.pins.push(SymbolPin {
            name: "OUT".into(),
            order: 2,
            x: 2,
            y: 0,
        });
        sym.shapes.push(SymbolShape::Circle {
            cx: 0.0,
            cy: 0.0,
            r: 1.0,
        });

        let asy = sym.to_asy();
        let parsed = CustomSymbol::from_asy("TEST_IC.asy", &asy);
        assert_eq!(parsed.prefix, "X");
        assert_eq!(parsed.default_value, "MY_IC");
        assert_eq!(parsed.pins.len(), 2);
        assert_eq!(parsed.pins[0].name, "IN");
        assert_eq!(parsed.pins[1].x, 2);
        assert_eq!(parsed.shapes.len(), sym.shapes.len());
    }
}
