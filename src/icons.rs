use egui::{Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarIcon {
    New,
    Open,
    Save,
    Run,
    Halt,
    Split,
    SplitVertical,
    ZoomIn,
    ZoomOut,
    ZoomFit,
    Probe,
    CurrentProbe,
    Wire,
    Ground,
    NetLabel,
    Resistor,
    Capacitor,
    Inductor,
    Diode,
    Component,
    Directive,
    ConfigureAnalysis,
    Move,
    Drag,
    Delete,
    Undo,
    Redo,
    AddTrace,
}

pub fn icon_button(
    ui: &mut Ui,
    icon: ToolbarIcon,
    tooltip: &str,
    active: bool,
    enabled: bool,
) -> bool {
    let size = Vec2::new(26.0, 24.0);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    let painter = ui.painter_at(rect);

    if active {
        painter.rect_filled(rect, 3.0, Color32::from_rgb(185, 212, 248));
        painter.rect_stroke(rect, 3.0, Stroke::new(1.0_f32, Color32::from_rgb(70, 120, 200)));
    } else if enabled && response.hovered() {
        painter.rect_filled(rect, 3.0, Color32::from_rgb(225, 232, 242));
        painter.rect_stroke(rect, 3.0, Stroke::new(1.0_f32, Color32::from_rgb(160, 175, 195)));
    }

    draw_icon(&painter, rect.shrink(3.5), icon, enabled);

    let clicked = enabled && response.clicked();
    response.on_hover_text(tooltip);
    clicked
}

fn draw_icon(painter: &egui::Painter, r: Rect, icon: ToolbarIcon, enabled: bool) {
    let c = r.center();
    let w = r.width();
    let h = r.height();
    let dim = |col: Color32| -> Color32 {
        if enabled {
            col
        } else {
            Color32::from_rgba_unmultiplied(150, 150, 150, 160)
        }
    };
    let blue = dim(Color32::from_rgb(0, 40, 190));
    let dark = dim(Color32::from_rgb(40, 40, 45));
    let green = dim(Color32::from_rgb(0, 175, 0));
    let red = dim(Color32::from_rgb(210, 35, 35));
    let folder_gold = dim(Color32::from_rgb(225, 175, 55));

    match icon {
        ToolbarIcon::New => {
            let page = Rect::from_min_max(
                Pos2::new(r.left() + 2.0, r.top() + 1.0),
                Pos2::new(r.right() - 2.0, r.bottom() - 1.0),
            );
            painter.rect_filled(page, 1.0, Color32::WHITE);
            painter.rect_stroke(page, 1.0, Stroke::new(1.2_f32, dark));
            // '+' badge
            let bc = Pos2::new(r.right() - 4.0, r.bottom() - 4.0);
            painter.circle_filled(bc, 4.0, green);
            painter.line_segment(
                [bc - Vec2::new(2.2, 0.0), bc + Vec2::new(2.2, 0.0)],
                Stroke::new(1.4_f32, Color32::WHITE),
            );
            painter.line_segment(
                [bc - Vec2::new(0.0, 2.2), bc + Vec2::new(0.0, 2.2)],
                Stroke::new(1.4_f32, Color32::WHITE),
            );
        }
        ToolbarIcon::Open => {
            let tab = Rect::from_min_max(
                Pos2::new(r.left() + 1.0, r.top() + 2.0),
                Pos2::new(r.left() + w * 0.45, r.top() + 6.0),
            );
            let body = Rect::from_min_max(
                Pos2::new(r.left() + 1.0, r.top() + 5.0),
                Pos2::new(r.right() - 1.0, r.bottom() - 2.0),
            );
            painter.rect_filled(tab, 1.0, folder_gold);
            painter.rect_filled(body, 1.5, folder_gold);
            painter.rect_stroke(body, 1.5, Stroke::new(1.1_f32, dark));
        }
        ToolbarIcon::Save => {
            let body = r.shrink(1.0);
            painter.rect_filled(body, 1.5, blue);
            painter.rect_stroke(body, 1.5, Stroke::new(1.1_f32, dark));
            let label = Rect::from_min_max(
                Pos2::new(body.left() + 3.0, body.top() + 1.0),
                Pos2::new(body.right() - 3.0, body.top() + h * 0.42),
            );
            painter.rect_filled(label, 0.5, Color32::WHITE);
            let shutter = Rect::from_min_max(
                Pos2::new(body.left() + 4.0, body.bottom() - h * 0.36),
                Pos2::new(body.right() - 4.0, body.bottom() - 1.0),
            );
            painter.rect_filled(shutter, 0.5, Color32::from_rgb(210, 210, 215));
        }
        ToolbarIcon::Run => {
            let pts = vec![
                Pos2::new(r.left() + 3.0, r.top() + 1.5),
                Pos2::new(r.right() - 2.0, c.y),
                Pos2::new(r.left() + 3.0, r.bottom() - 1.5),
            ];
            painter.add(egui::Shape::convex_polygon(
                pts,
                green,
                Stroke::new(1.0_f32, dim(Color32::from_rgb(0, 110, 0))),
            ));
        }
        ToolbarIcon::Halt => {
            let sq = r.shrink(3.0);
            painter.rect_filled(sq, 1.5, if enabled { red } else { dim(Color32::GRAY) });
        }
        ToolbarIcon::Split => {
            let top = Rect::from_min_max(
                Pos2::new(r.left() + 1.0, r.top() + 1.0),
                Pos2::new(r.right() - 1.0, c.y - 1.5),
            );
            let bot = Rect::from_min_max(
                Pos2::new(r.left() + 1.0, c.y + 1.5),
                Pos2::new(r.right() - 1.0, r.bottom() - 1.0),
            );
            painter.rect_filled(top, 0.5, Color32::BLACK);
            painter.rect_stroke(top, 0.5, Stroke::new(1.0_f32, dark));
            painter.rect_filled(bot, 0.5, Color32::from_rgb(200, 200, 200));
            painter.rect_stroke(bot, 0.5, Stroke::new(1.0_f32, dark));
        }
        ToolbarIcon::SplitVertical => {
            let left = Rect::from_min_max(
                Pos2::new(r.left() + 1.0, r.top() + 1.0),
                Pos2::new(c.x - 1.5, r.bottom() - 1.0),
            );
            let right = Rect::from_min_max(
                Pos2::new(c.x + 1.5, r.top() + 1.0),
                Pos2::new(r.right() - 1.0, r.bottom() - 1.0),
            );
            painter.rect_filled(left, 0.5, Color32::BLACK);
            painter.rect_stroke(left, 0.5, Stroke::new(1.0_f32, dark));
            painter.rect_filled(right, 0.5, Color32::from_rgb(200, 200, 200));
            painter.rect_stroke(right, 0.5, Stroke::new(1.0_f32, dark));
        }
        ToolbarIcon::ZoomIn | ToolbarIcon::ZoomOut | ToolbarIcon::ZoomFit => {
            let lc = Pos2::new(r.left() + w * 0.42, r.top() + h * 0.42);
            let rad = w * 0.30;
            painter.circle_filled(lc, rad, Color32::from_rgba_unmultiplied(220, 240, 255, 220));
            painter.circle_stroke(lc, rad, Stroke::new(1.5_f32, dark));
            painter.line_segment(
                [lc + Vec2::splat(rad * 0.7), r.right_bottom() - Vec2::splat(1.0)],
                Stroke::new(2.2_f32, dark),
            );
            if icon == ToolbarIcon::ZoomIn {
                painter.line_segment(
                    [lc - Vec2::new(3.0, 0.0), lc + Vec2::new(3.0, 0.0)],
                    Stroke::new(1.4_f32, dark),
                );
                painter.line_segment(
                    [lc - Vec2::new(0.0, 3.0), lc + Vec2::new(0.0, 3.0)],
                    Stroke::new(1.4_f32, dark),
                );
            } else if icon == ToolbarIcon::ZoomOut {
                painter.line_segment(
                    [lc - Vec2::new(3.0, 0.0), lc + Vec2::new(3.0, 0.0)],
                    Stroke::new(1.4_f32, dark),
                );
            } else {
                painter.rect_stroke(
                    Rect::from_center_size(lc, Vec2::splat(4.5)),
                    0.0,
                    Stroke::new(1.2_f32, blue),
                );
            }
        }
        ToolbarIcon::Probe => {
            // Red voltage probe tip
            painter.line_segment(
                [Pos2::new(r.left() + 3.0, r.bottom() - 2.0), Pos2::new(r.right() - 3.0, r.top() + 2.0)],
                Stroke::new(2.5_f32, red),
            );
            painter.circle_filled(Pos2::new(r.left() + 3.0, r.bottom() - 2.0), 1.8, dark);
        }
        ToolbarIcon::CurrentProbe => {
            // Blue current clamp probe + 'I' indicator
            let clamp_color = Color32::from_rgb(0, 120, 220);
            painter.circle_stroke(
                Pos2::new(c.x - 1.5, c.y),
                5.0,
                Stroke::new(1.8_f32, clamp_color),
            );
            painter.line_segment(
                [Pos2::new(r.left() + 2.0, c.y), Pos2::new(r.right() - 2.0, c.y)],
                Stroke::new(1.3_f32, dark),
            );
            painter.text(
                Pos2::new(r.right() - 2.0, r.top() + 1.0),
                egui::Align2::RIGHT_TOP,
                "I",
                FontId::monospace(9.5),
                clamp_color,
            );
        }
        ToolbarIcon::Wire => {
            let st = Stroke::new(1.8_f32, blue);
            let p1 = Pos2::new(r.left() + 2.0, r.bottom() - 3.0);
            let pm = Pos2::new(c.x, r.bottom() - 3.0);
            let p2 = Pos2::new(c.x, r.top() + 3.0);
            let p3 = Pos2::new(r.right() - 2.0, r.top() + 3.0);
            painter.line_segment([p1, pm], st);
            painter.line_segment([pm, p2], st);
            painter.line_segment([p2, p3], st);
            painter.circle_filled(p1, 2.0, blue);
            painter.circle_filled(p3, 2.0, blue);
        }
        ToolbarIcon::Ground => {
            let st = Stroke::new(1.6_f32, blue);
            painter.line_segment([Pos2::new(c.x, r.top() + 2.0), Pos2::new(c.x, c.y)], st);
            painter.line_segment([Pos2::new(r.left() + 3.0, c.y), Pos2::new(r.right() - 3.0, c.y)], st);
            painter.line_segment([Pos2::new(r.left() + 3.0, c.y), Pos2::new(c.x, r.bottom() - 2.0)], st);
            painter.line_segment([Pos2::new(r.right() - 3.0, c.y), Pos2::new(c.x, r.bottom() - 2.0)], st);
        }
        ToolbarIcon::NetLabel => {
            painter.text(c, egui::Align2::CENTER_CENTER, "A", FontId::monospace(14.0), blue);
            painter.line_segment(
                [Pos2::new(r.left() + 3.0, r.bottom() - 2.0), Pos2::new(r.right() - 3.0, r.bottom() - 2.0)],
                Stroke::new(1.4_f32, blue),
            );
        }
        ToolbarIcon::Resistor => {
            let st = Stroke::new(1.5_f32, blue);
            let pts = [
                Pos2::new(r.left() + 1.0, c.y),
                Pos2::new(r.left() + 4.0, c.y),
                Pos2::new(r.left() + 6.0, c.y - 4.0),
                Pos2::new(r.left() + 9.0, c.y + 4.0),
                Pos2::new(r.left() + 12.0, c.y - 4.0),
                Pos2::new(r.left() + 15.0, c.y + 4.0),
                Pos2::new(r.right() - 4.0, c.y),
                Pos2::new(r.right() - 1.0, c.y),
            ];
            for w2 in pts.windows(2) {
                painter.line_segment([w2[0], w2[1]], st);
            }
        }
        ToolbarIcon::Capacitor => {
            let st = Stroke::new(1.7_f32, blue);
            painter.line_segment([Pos2::new(r.left() + 1.0, c.y), Pos2::new(c.x - 2.5, c.y)], st);
            painter.line_segment([Pos2::new(c.x - 2.5, r.top() + 3.0), Pos2::new(c.x - 2.5, r.bottom() - 3.0)], st);
            painter.line_segment([Pos2::new(c.x + 2.5, r.top() + 3.0), Pos2::new(c.x + 2.5, r.bottom() - 3.0)], st);
            painter.line_segment([Pos2::new(c.x + 2.5, c.y), Pos2::new(r.right() - 1.0, c.y)], st);
        }
        ToolbarIcon::Inductor => {
            let st = Stroke::new(1.5_f32, blue);
            painter.line_segment([Pos2::new(r.left() + 1.0, c.y), Pos2::new(r.left() + 3.5, c.y)], st);
            for i in 0..3 {
                let cx = r.left() + 6.0 + i as f32 * 4.0;
                painter.circle_stroke(Pos2::new(cx, c.y), 2.1, st);
            }
            painter.line_segment([Pos2::new(r.right() - 3.5, c.y), Pos2::new(r.right() - 1.0, c.y)], st);
        }
        ToolbarIcon::Diode => {
            let st = Stroke::new(1.5_f32, blue);
            painter.line_segment([Pos2::new(r.left() + 1.0, c.y), Pos2::new(c.x - 4.0, c.y)], st);
            let tri = vec![
                Pos2::new(c.x - 4.0, r.top() + 3.5),
                Pos2::new(c.x + 3.5, c.y),
                Pos2::new(c.x - 4.0, r.bottom() - 3.5),
            ];
            painter.add(egui::Shape::convex_polygon(tri, blue, st));
            painter.line_segment(
                [Pos2::new(c.x + 4.0, r.top() + 3.5), Pos2::new(c.x + 4.0, r.bottom() - 3.5)],
                st,
            );
            painter.line_segment([Pos2::new(c.x + 4.0, c.y), Pos2::new(r.right() - 1.0, c.y)], st);
        }
        ToolbarIcon::Component => {
            // Op-Amp / Gate symbol
            let st = Stroke::new(1.5_f32, blue);
            let pts = vec![
                Pos2::new(r.left() + 4.0, r.top() + 2.5),
                Pos2::new(r.right() - 3.0, c.y),
                Pos2::new(r.left() + 4.0, r.bottom() - 2.5),
            ];
            painter.add(egui::Shape::closed_line(pts, st));
            painter.line_segment([Pos2::new(r.left() + 1.0, c.y - 3.0), Pos2::new(r.left() + 4.0, c.y - 3.0)], st);
            painter.line_segment([Pos2::new(r.left() + 1.0, c.y + 3.0), Pos2::new(r.left() + 4.0, c.y + 3.0)], st);
            painter.line_segment([Pos2::new(r.right() - 3.0, c.y), Pos2::new(r.right() - 1.0, c.y)], st);
        }
        ToolbarIcon::Directive => {
            painter.text(c, egui::Align2::CENTER_CENTER, ".op", FontId::monospace(11.0), dark);
        }
        ToolbarIcon::ConfigureAnalysis => {
            painter.text(c, egui::Align2::CENTER_CENTER, ".ac", FontId::monospace(11.0), blue);
        }
        ToolbarIcon::Move => {
            let st = Stroke::new(1.6_f32, dark);
            painter.line_segment([Pos2::new(r.left() + 2.0, c.y), Pos2::new(r.right() - 2.0, c.y)], st);
            painter.line_segment([Pos2::new(c.x, r.top() + 2.0), Pos2::new(c.x, r.bottom() - 2.0)], st);
        }
        ToolbarIcon::Drag => {
            // Hand/box drag with connected orthogonal wires
            let st_wire = Stroke::new(1.5_f32, blue);
            let st_box = Stroke::new(1.5_f32, dark);
            painter.line_segment([Pos2::new(r.left() + 1.5, c.y), Pos2::new(c.x - 4.0, c.y)], st_wire);
            painter.line_segment([Pos2::new(c.x + 4.0, c.y), Pos2::new(r.right() - 1.5, c.y)], st_wire);
            painter.rect_stroke(
                Rect::from_center_size(c, Vec2::splat(8.0)),
                1.0,
                st_box,
            );
            painter.line_segment([Pos2::new(c.x - 2.5, c.y), Pos2::new(c.x + 2.5, c.y)], st_box);
            painter.line_segment([Pos2::new(c.x, c.y - 2.5), Pos2::new(c.x, c.y + 2.5)], st_box);
        }
        ToolbarIcon::Delete => {
            let st = Stroke::new(2.0_f32, red);
            painter.line_segment([r.left_top() + Vec2::splat(3.0), r.right_bottom() - Vec2::splat(3.0)], st);
            painter.line_segment([r.right_top() + Vec2::new(-3.0, 3.0), r.left_bottom() + Vec2::new(3.0, -3.0)], st);
        }
        ToolbarIcon::Undo => {
            let st = Stroke::new(1.8_f32, blue);
            painter.line_segment([Pos2::new(r.left() + 3.0, c.y), Pos2::new(r.right() - 3.0, c.y)], st);
            painter.line_segment([Pos2::new(r.left() + 3.0, c.y), Pos2::new(r.left() + 7.5, c.y - 4.0)], st);
            painter.line_segment([Pos2::new(r.left() + 3.0, c.y), Pos2::new(r.left() + 7.5, c.y + 4.0)], st);
        }
        ToolbarIcon::Redo => {
            let st = Stroke::new(1.8_f32, blue);
            painter.line_segment([Pos2::new(r.left() + 3.0, c.y), Pos2::new(r.right() - 3.0, c.y)], st);
            painter.line_segment([Pos2::new(r.right() - 3.0, c.y), Pos2::new(r.right() - 7.5, c.y - 4.0)], st);
            painter.line_segment([Pos2::new(r.right() - 3.0, c.y), Pos2::new(r.right() - 7.5, c.y + 4.0)], st);
        }
        ToolbarIcon::AddTrace => {
            let st = Stroke::new(1.6_f32, green);
            let pts = [
                Pos2::new(r.left() + 2.0, r.bottom() - 4.0),
                Pos2::new(r.left() + 7.0, r.top() + 4.0),
                Pos2::new(r.left() + 12.0, r.bottom() - 5.0),
                Pos2::new(r.right() - 2.0, r.top() + 5.0),
            ];
            for w2 in pts.windows(2) {
                painter.line_segment([w2[0], w2[1]], st);
            }
        }
    }
}

pub fn app_window_icon() -> egui::IconData {
    let size = 32usize;
    let mut rgba = vec![0u8; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            let idx = (y * size + x) * 4;
            // Dark background with blue border and green sine-like trace
            let is_border = x < 2 || x >= size - 2 || y < 2 || y >= size - 2;
            if is_border {
                rgba[idx] = 40;
                rgba[idx + 1] = 90;
                rgba[idx + 2] = 200;
                rgba[idx + 3] = 255;
            } else {
                let fx = (x as f32 / size as f32) * std::f32::consts::TAU;
                let wave_y = (size as f32 * 0.5) - (fx.sin() * (size as f32 * 0.28));
                let on_wave = ((y as f32) - wave_y).abs() < 1.6;
                if on_wave {
                    rgba[idx] = 0;
                    rgba[idx + 1] = 240;
                    rgba[idx + 2] = 60;
                    rgba[idx + 3] = 255;
                } else {
                    rgba[idx] = 18;
                    rgba[idx + 1] = 18;
                    rgba[idx + 2] = 22;
                    rgba[idx + 3] = 255;
                }
            }
        }
    }
    egui::IconData {
        rgba,
        width: size as u32,
        height: size as u32,
    }
}
