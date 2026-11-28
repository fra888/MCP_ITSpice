use std::collections::HashSet;

use egui::{Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::hdl::HdlBlock;
use crate::model::{
    component_pins_world, transform_offset, Component, ComponentKind, Directive, NetlistResult,
    Schematic, Wire, GRID_SIZE,
};
use crate::ngspice::SimulationResult;
use crate::symbol_editor::{CustomSymbol, SymbolShape};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorTool {
    SelectOrProbe,
    ProbeCurrent,
    Wire,
    Place(ComponentKind),
    Move,
    Drag,
    Delete,
    PlaceDirective,
}

#[derive(Debug, Clone)]
pub enum SchematicAction {
    None,
    ProbeTrace(String),
    ProbePower(u64),
    EditComponent(u64),
    EditDirective(u64),
    RequestNewDirective(i32, i32),
    RequestNewNetLabel(i32, i32),
    Modified,
}

#[derive(Debug, Clone)]
pub struct ClipboardContent {
    pub components: Vec<Component>,
    pub wires: Vec<Wire>,
    pub directives: Vec<Directive>,
    pub anchor_x: i32,
    pub anchor_y: i32,
}

#[derive(Debug, Clone)]
pub struct DraggingText {
    pub comp_id: u64,
    pub is_value: bool,
    pub orig_offset: (f32, f32),
    pub start_pos: Pos2,
}

pub struct SchematicViewState {
    pub pan: Vec2,
    pub zoom: f32,
    pub wire_start: Option<(i32, i32)>,
    pub place_rotation: i32,
    pub place_mirror: bool,
    pub moving_comp_id: Option<u64>,
    pub moving_anchor: Option<(i32, i32)>,
    pub marquee_start: Option<Pos2>,
    pub dragging_text: Option<DraggingText>,
    pub selected_components: HashSet<u64>,
    pub selected_wires: HashSet<u64>,
    pub selected_directives: HashSet<u64>,
    pub clipboard: Option<ClipboardContent>,
    pub active_custom_symbol: Option<CustomSymbol>,
    pub active_hdl_block: Option<HdlBlock>,
    pub cursor_grid: Option<(i32, i32)>,
    pub hovered_probe: Option<String>,
}

impl Default for SchematicViewState {
    fn default() -> Self {
        Self {
            pan: Vec2::new(40.0, 20.0),
            zoom: 1.0,
            wire_start: None,
            place_rotation: 0,
            place_mirror: false,
            moving_comp_id: None,
            moving_anchor: None,
            marquee_start: None,
            dragging_text: None,
            selected_components: HashSet::new(),
            selected_wires: HashSet::new(),
            selected_directives: HashSet::new(),
            clipboard: None,
            active_custom_symbol: None,
            active_hdl_block: None,
            cursor_grid: None,
            hovered_probe: None,
        }
    }
}

impl SchematicViewState {
    pub fn has_selection(&self) -> bool {
        !self.selected_components.is_empty()
            || !self.selected_wires.is_empty()
            || !self.selected_directives.is_empty()
    }

    pub fn clear_selection(&mut self) {
        self.selected_components.clear();
        self.selected_wires.clear();
        self.selected_directives.clear();
    }
    pub fn zoom_to_fit(&mut self, schematic: &Schematic, rect: Rect) {
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;

        for c in &schematic.components {
            min_x = min_x.min(c.x - 3);
            min_y = min_y.min(c.y - 3);
            max_x = max_x.max(c.x + 3);
            max_y = max_y.max(c.y + 3);
        }
        for w in &schematic.wires {
            min_x = min_x.min(w.x1.min(w.x2));
            min_y = min_y.min(w.y1.min(w.y2));
            max_x = max_x.max(w.x1.max(w.x2));
            max_y = max_y.max(w.y1.max(w.y2));
        }
        for d in &schematic.directives {
            min_x = min_x.min(d.x);
            min_y = min_y.min(d.y);
            max_x = max_x.max(d.x + 6);
            max_y = max_y.max(d.y + 2);
        }

        if min_x > max_x {
            self.zoom = 1.0;
            self.pan = Vec2::new(rect.width() * 0.1, rect.height() * 0.1);
            return;
        }

        let world_w = ((max_x - min_x).max(10) as f32) * GRID_SIZE;
        let world_h = ((max_y - min_y).max(10) as f32) * GRID_SIZE;
        let zx = (rect.width() * 0.85) / world_w;
        let zy = (rect.height() * 0.85) / world_h;
        self.zoom = zx.min(zy).clamp(0.35, 2.5);

        let center_world = Vec2::new(
            (min_x + max_x) as f32 * 0.5 * GRID_SIZE,
            (min_y + max_y) as f32 * 0.5 * GRID_SIZE,
        );
        self.pan = rect.size() * 0.5 - center_world * self.zoom;
    }
}

pub fn draw_schematic_pane(
    ui: &mut Ui,
    schematic: &mut Schematic,
    view: &mut SchematicViewState,
    tool: &mut EditorTool,
    netlist: &NetlistResult,
    sim_result: Option<&SimulationResult>,
    undo_stack: &mut Vec<Schematic>,
) -> SchematicAction {
    let mut action = SchematicAction::None;
    let available = ui.available_size();
    let (rect, response) = ui.allocate_exact_size(available, Sense::click_and_drag());

    let painter = ui.painter_at(rect);
    // LTspice classic gray background (#C0C0C0)
    painter.rect_filled(rect, 0.0, Color32::from_rgb(192, 192, 192));

    let to_screen = |gx: f32, gy: f32, view: &SchematicViewState| -> Pos2 {
        Pos2::new(
            rect.left() + view.pan.x + gx * GRID_SIZE * view.zoom,
            rect.top() + view.pan.y + gy * GRID_SIZE * view.zoom,
        )
    };
    let to_grid = |pos: Pos2, view: &SchematicViewState| -> (i32, i32) {
        let wx = (pos.x - rect.left() - view.pan.x) / (GRID_SIZE * view.zoom);
        let wy = (pos.y - rect.top() - view.pan.y) / (GRID_SIZE * view.zoom);
        (wx.round() as i32, wy.round() as i32)
    };

    // Pan with middle mouse or Alt+drag
    if response.dragged_by(egui::PointerButton::Middle)
        || (response.dragged_by(egui::PointerButton::Primary)
            && ui.input(|i| i.modifiers.alt))
    {
        view.pan += response.drag_delta();
    }

    // Zoom with scroll wheel
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll.abs() > 0.1 {
            if let Some(pointer) = response.hover_pos() {
                let old_zoom = view.zoom;
                let factor = if scroll > 0.0 { 1.12 } else { 1.0 / 1.12 };
                view.zoom = (view.zoom * factor).clamp(0.3, 4.0);
                let rel = pointer - rect.left_top();
                view.pan = rel - (rel - view.pan) * (view.zoom / old_zoom);
            }
        }
    }

    // Keyboard shortcuts when hovered or focused
    if response.hovered() && !ui.ctx().wants_keyboard_input() {
        let rotate_pressed = ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::R));
        if rotate_pressed {
            if matches!(*tool, EditorTool::Place(_)) {
                view.place_rotation = (view.place_rotation + 90) % 360;
            } else if !view.selected_components.is_empty() {
                undo_stack.push(schematic.clone());
                for c in &mut schematic.components {
                    if view.selected_components.contains(&c.id) {
                        c.rotation = (c.rotation + 90) % 360;
                    }
                }
                action = SchematicAction::Modified;
            }
        }
        if ui.input(|i| i.key_pressed(egui::Key::E)) {
            if matches!(*tool, EditorTool::Place(_)) {
                view.place_mirror = !view.place_mirror;
            } else if !view.selected_components.is_empty() {
                undo_stack.push(schematic.clone());
                for c in &mut schematic.components {
                    if view.selected_components.contains(&c.id) {
                        c.mirror = !c.mirror;
                    }
                }
                action = SchematicAction::Modified;
            }
        }
        if ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
            && view.has_selection()
        {
            undo_stack.push(schematic.clone());
            schematic
                .components
                .retain(|c| !view.selected_components.contains(&c.id));
            schematic
                .wires
                .retain(|w| !view.selected_wires.contains(&w.id));
            schematic
                .directives
                .retain(|d| !view.selected_directives.contains(&d.id));
            view.clear_selection();
            action = SchematicAction::Modified;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            view.wire_start = None;
            view.moving_comp_id = None;
            view.moving_anchor = None;
            view.marquee_start = None;
            view.clear_selection();
            *tool = EditorTool::SelectOrProbe;
        }
    }

    // Ctrl+C / Ctrl+X / Ctrl+V clipboard shortcuts (when not typing in a modal)
    if !ui.ctx().wants_keyboard_input() {
        let copy_pressed = ui.input(|i| {
            i.events.iter().any(|e| matches!(e, egui::Event::Copy))
                || (i.modifiers.command && i.key_pressed(egui::Key::C))
        });
        let cut_pressed = ui.input(|i| {
            i.events.iter().any(|e| matches!(e, egui::Event::Cut))
                || (i.modifiers.command && i.key_pressed(egui::Key::X))
        });
        let paste_pressed = ui.input(|i| {
            i.events.iter().any(|e| matches!(e, egui::Event::Paste(_)))
                || (i.modifiers.command && i.key_pressed(egui::Key::V))
        });

        if (copy_pressed || cut_pressed) && view.has_selection() {
            let comps: Vec<Component> = schematic
                .components
                .iter()
                .filter(|c| view.selected_components.contains(&c.id))
                .cloned()
                .collect();
            let wires: Vec<Wire> = schematic
                .wires
                .iter()
                .filter(|w| view.selected_wires.contains(&w.id))
                .cloned()
                .collect();
            let dirs: Vec<Directive> = schematic
                .directives
                .iter()
                .filter(|d| view.selected_directives.contains(&d.id))
                .cloned()
                .collect();

            let mut min_x = i32::MAX;
            let mut min_y = i32::MAX;
            let mut max_x = i32::MIN;
            let mut max_y = i32::MIN;
            for c in &comps {
                min_x = min_x.min(c.x);
                min_y = min_y.min(c.y);
                max_x = max_x.max(c.x);
                max_y = max_y.max(c.y);
            }
            for w in &wires {
                min_x = min_x.min(w.x1.min(w.x2));
                min_y = min_y.min(w.y1.min(w.y2));
                max_x = max_x.max(w.x1.max(w.x2));
                max_y = max_y.max(w.y1.max(w.y2));
            }
            for d in &dirs {
                min_x = min_x.min(d.x);
                min_y = min_y.min(d.y);
                max_x = max_x.max(d.x);
                max_y = max_y.max(d.y);
            }

            let anchor_x = if min_x <= max_x { (min_x + max_x) / 2 } else { 0 };
            let anchor_y = if min_y <= max_y { (min_y + max_y) / 2 } else { 0 };
            view.clipboard = Some(ClipboardContent {
                components: comps,
                wires,
                directives: dirs,
                anchor_x,
                anchor_y,
            });

            if cut_pressed {
                undo_stack.push(schematic.clone());
                schematic
                    .components
                    .retain(|c| !view.selected_components.contains(&c.id));
                schematic
                    .wires
                    .retain(|w| !view.selected_wires.contains(&w.id));
                schematic
                    .directives
                    .retain(|d| !view.selected_directives.contains(&d.id));
                view.clear_selection();
                action = SchematicAction::Modified;
            }
        }

        if paste_pressed {
            if let Some(clip) = view.clipboard.clone() {
                undo_stack.push(schematic.clone());
                let (tx, ty) = view
                    .cursor_grid
                    .unwrap_or((clip.anchor_x + 2, clip.anchor_y + 2));
                let dx = tx - clip.anchor_x;
                let dy = ty - clip.anchor_y;

                view.clear_selection();
                for mut c in clip.components {
                    let id = schematic.alloc_id();
                    c.id = id;
                    c.x += dx;
                    c.y += dy;
                    if c.kind.has_refdes() {
                        c.refdes = schematic.next_refdes(c.kind);
                    }
                    schematic.components.push(c);
                    view.selected_components.insert(id);
                }
                for mut w in clip.wires {
                    let id = schematic.alloc_id();
                    w.id = id;
                    w.x1 += dx;
                    w.y1 += dy;
                    w.x2 += dx;
                    w.y2 += dy;
                    schematic.wires.push(w);
                    view.selected_wires.insert(id);
                }
                for mut d in clip.directives {
                    let id = schematic.alloc_id();
                    d.id = id;
                    d.x += dx;
                    d.y += dy;
                    schematic.directives.push(d);
                    view.selected_directives.insert(id);
                }
                action = SchematicAction::Modified;
            }
        }
    }

    // Draw subtle grid dots
    let step = GRID_SIZE * view.zoom;
    if step >= 8.0 {
        let start_gx = ((-view.pan.x) / step).floor() as i32;
        let end_gx = ((rect.width() - view.pan.x) / step).ceil() as i32;
        let start_gy = ((-view.pan.y) / step).floor() as i32;
        let end_gy = ((rect.height() - view.pan.y) / step).ceil() as i32;
        let dot_color = Color32::from_rgb(155, 155, 155);
        for gx in start_gx..=end_gx {
            for gy in start_gy..=end_gy {
                let p = to_screen(gx as f32, gy as f32, view);
                painter.rect_filled(Rect::from_center_size(p, Vec2::splat(1.2)), 0.0, dot_color);
            }
        }
    }

    // Update cursor grid
    view.cursor_grid = response.hover_pos().map(|p| to_grid(p, view));

    // Draw wires (LTspice blue #0000FF, or bright amber when selected)
    let wire_color = Color32::from_rgb(0, 0, 220);
    let sel_wire_color = Color32::from_rgb(255, 130, 0);
    let wire_stroke = Stroke::new((1.6 * view.zoom).clamp(1.0, 3.0), wire_color);
    let sel_wire_stroke = Stroke::new((2.8 * view.zoom).clamp(2.0, 4.5), sel_wire_color);
    for w in &schematic.wires {
        let p1 = to_screen(w.x1 as f32, w.y1 as f32, view);
        let p2 = to_screen(w.x2 as f32, w.y2 as f32, view);
        let st = if view.selected_wires.contains(&w.id) {
            sel_wire_stroke
        } else {
            wire_stroke
        };
        painter.line_segment([p1, p2], st);
    }

    // Draw junction dots where 3+ wire endpoints/segments meet
    draw_junctions(&painter, schematic, &|x, y| to_screen(x, y, view), view.zoom);

    // Draw components and collect text hitboxes for label/value dragging
    let sel_box_stroke = Stroke::new(1.5_f32, Color32::from_rgb(0, 120, 215));
    let sel_box_fill = Color32::from_rgba_unmultiplied(0, 120, 215, 28);
    let mut text_hitboxes: Vec<(u64, bool, Rect)> = Vec::new();
    for c in &schematic.components {
        if view.selected_components.contains(&c.id) {
            let p_min = to_screen((c.x - 2) as f32, (c.y - 2) as f32, view);
            let p_max = to_screen((c.x + 2) as f32, (c.y + 2) as f32, view);
            let sel_r = Rect::from_two_pos(p_min, p_max);
            painter.rect_filled(sel_r, 2.0, sel_box_fill);
            painter.rect_stroke(sel_r, 2.0, sel_box_stroke);
        }
        let (r_rect, v_rect) =
            draw_component(&painter, c, &|x, y| to_screen(x, y, view), view.zoom, false);
        if let Some(rr) = r_rect {
            text_hitboxes.push((c.id, false, rr));
        }
        if let Some(vr) = v_rect {
            text_hitboxes.push((c.id, true, vr));
        }
    }

    // Draw SPICE directives
    for d in &schematic.directives {
        let pos = to_screen(d.x as f32, d.y as f32, view);
        let text_galley = painter.layout_no_wrap(
            d.text.clone(),
            FontId::monospace((12.0 * view.zoom).clamp(8.0, 22.0)),
            Color32::BLACK,
        );
        if view.selected_directives.contains(&d.id) {
            let r = Rect::from_min_size(pos, text_galley.size()).expand(3.0);
            painter.rect_filled(r, 2.0, sel_box_fill);
            painter.rect_stroke(r, 2.0, sel_box_stroke);
        }
        painter.galley(pos, text_galley, Color32::BLACK);
    }

    // Draw tool previews
    if let Some((gx, gy)) = view.cursor_grid {
        match *tool {
            EditorTool::Wire => {
                if let Some((sx, sy)) = view.wire_start {
                    let p1 = to_screen(sx as f32, sy as f32, view);
                    let pm = to_screen(gx as f32, sy as f32, view);
                    let p2 = to_screen(gx as f32, gy as f32, view);
                    let preview_stroke =
                        Stroke::new(1.5, Color32::from_rgba_unmultiplied(0, 0, 255, 160));
                    painter.line_segment([p1, pm], preview_stroke);
                    painter.line_segment([pm, p2], preview_stroke);
                } else {
                    let p = to_screen(gx as f32, gy as f32, view);
                    painter.circle_stroke(p, 4.0, Stroke::new(1.0, Color32::BLUE));
                }
            }
            EditorTool::Place(kind) => {
                let (refdes, value, custom_symbol) = if kind == ComponentKind::Custom {
                    let sym = view.active_custom_symbol.clone();
                    let val = sym
                        .as_ref()
                        .map(|s| s.default_value.clone())
                        .unwrap_or_else(|| kind.default_value().to_string());
                    (schematic.next_refdes(kind), val, sym)
                } else {
                    (
                        schematic.next_refdes(kind),
                        kind.default_value().to_string(),
                        None,
                    )
                };
                let ghost = Component {
                    id: 0,
                    kind,
                    refdes,
                    value,
                    x: gx,
                    y: gy,
                    rotation: view.place_rotation,
                    mirror: view.place_mirror,
                    params: Default::default(),
                    custom_symbol,
                    hdl: None,
                    label_offset: None,
                    value_offset: None,
                };
                draw_component(
                    &painter,
                    &ghost,
                    &|x, y| to_screen(x, y, view),
                    view.zoom,
                    true,
                );
            }
            EditorTool::Move | EditorTool::Drag => {
                let is_drag = *tool == EditorTool::Drag;
                let (comp_ids, wire_ids, dx, dy) = if let Some((ax, ay)) = view.moving_anchor {
                    (
                        view.selected_components.clone(),
                        view.selected_wires.clone(),
                        gx - ax,
                        gy - ay,
                    )
                } else if let Some(cid) = view.moving_comp_id {
                    if let Some(orig) = schematic.components.iter().find(|c| c.id == cid) {
                        let mut cset = HashSet::new();
                        cset.insert(cid);
                        (cset, HashSet::new(), gx - orig.x, gy - orig.y)
                    } else {
                        (HashSet::new(), HashSet::new(), 0, 0)
                    }
                } else {
                    (HashSet::new(), HashSet::new(), 0, 0)
                };

                if !comp_ids.is_empty() || !wire_ids.is_empty() {
                    for c in &schematic.components {
                        if comp_ids.contains(&c.id) {
                            let mut ghost = c.clone();
                            ghost.x += dx;
                            ghost.y += dy;
                            draw_component(
                                &painter,
                                &ghost,
                                &|x, y| to_screen(x, y, view),
                                view.zoom,
                                true,
                            );
                        }
                    }
                    let ghost_wire_stroke = Stroke::new(
                        (1.6 * view.zoom).clamp(1.0, 3.0),
                        Color32::from_rgba_unmultiplied(0, 0, 220, 140),
                    );
                    if is_drag {
                        let mut preview_sch = schematic.clone();
                        drag_with_wires(
                            &mut preview_sch,
                            &comp_ids,
                            &wire_ids,
                            &HashSet::new(),
                            dx,
                            dy,
                        );
                        for w in &preview_sch.wires {
                            let p1 = to_screen(w.x1 as f32, w.y1 as f32, view);
                            let p2 = to_screen(w.x2 as f32, w.y2 as f32, view);
                            painter.line_segment([p1, p2], ghost_wire_stroke);
                        }
                    } else {
                        for w in &schematic.wires {
                            if wire_ids.contains(&w.id) {
                                let p1 = to_screen((w.x1 + dx) as f32, (w.y1 + dy) as f32, view);
                                let p2 = to_screen((w.x2 + dx) as f32, (w.y2 + dy) as f32, view);
                                painter.line_segment([p1, p2], ghost_wire_stroke);
                            }
                        }
                    }
                }
            }
            EditorTool::SelectOrProbe | EditorTool::ProbeCurrent
                if ui.input(|i| i.modifiers.alt) =>
            {
                view.hovered_probe = None;
                if let Some(sim) = sim_result {
                    if sim.success {
                        if let Some((_, p_label)) =
                            find_power_probe_target(gx, gy, schematic, netlist, sim)
                        {
                            view.hovered_probe = Some(p_label.clone());
                            let p = to_screen(gx as f32, gy as f32, view);
                            let pwr_color = Color32::from_rgb(255, 140, 0);
                            // Thermometer-style power probe icon
                            painter.circle_filled(p, 4.5, pwr_color);
                            painter.line_segment(
                                [p, p + Vec2::new(0.0, -10.0)],
                                Stroke::new(2.2_f32, pwr_color),
                            );
                            painter.text(
                                p + Vec2::new(8.0, -14.0),
                                egui::Align2::LEFT_BOTTOM,
                                &p_label,
                                FontId::monospace(12.0),
                                pwr_color,
                            );
                        }
                    }
                }
            }
            EditorTool::SelectOrProbe => {
                view.hovered_probe = None;
                if let Some(sim) = sim_result {
                    if sim.success {
                        if let Some(probe) =
                            find_voltage_probe_target(gx, gy, schematic, netlist, sim)
                        {
                            view.hovered_probe = Some(probe.clone());
                            let p = to_screen(gx as f32, gy as f32, view);
                            painter.circle_filled(p, 4.5, Color32::from_rgb(220, 30, 30));
                            painter.text(
                                p + Vec2::new(8.0, -14.0),
                                egui::Align2::LEFT_BOTTOM,
                                &probe,
                                FontId::monospace(12.0),
                                Color32::from_rgb(180, 0, 0),
                            );
                        }
                    }
                }
            }
            EditorTool::ProbeCurrent => {
                view.hovered_probe = None;
                if let Some(sim) = sim_result {
                    if sim.success {
                        if let Some(probe) =
                            find_current_probe_target(gx, gy, schematic, netlist, sim)
                        {
                            view.hovered_probe = Some(probe.clone());
                            let p = to_screen(gx as f32, gy as f32, view);
                            let blue_probe = Color32::from_rgb(0, 120, 220);
                            painter.circle_stroke(p, 6.0, Stroke::new(2.0_f32, blue_probe));
                            painter.text(
                                p + Vec2::new(8.0, -14.0),
                                egui::Align2::LEFT_BOTTOM,
                                &probe,
                                FontId::monospace(12.0),
                                blue_probe,
                            );
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Label/Value dragging OR Marquee (box) selection in SelectOrProbe mode
    if *tool == EditorTool::SelectOrProbe && !ui.input(|i| i.modifiers.alt) {
        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(pos) = response.interact_pointer_pos() {
                if let Some(&(cid, is_val, _)) =
                    text_hitboxes.iter().find(|(_, _, r)| r.expand(4.0).contains(pos))
                {
                    if let Some(comp) = schematic.components.iter().find(|c| c.id == cid) {
                        undo_stack.push(schematic.clone());
                        let orig = if is_val {
                            comp.value_offset
                                .unwrap_or_else(|| default_value_offset(comp))
                        } else {
                            comp.label_offset
                                .unwrap_or_else(|| default_label_offset(comp))
                        };
                        view.dragging_text = Some(DraggingText {
                            comp_id: cid,
                            is_value: is_val,
                            orig_offset: orig,
                            start_pos: pos,
                        });
                    }
                } else {
                    view.marquee_start = Some(pos);
                }
            }
        }
        if let Some(drag) = view.dragging_text.clone() {
            if let Some(cur) = response.hover_pos() {
                let delta_g = (cur - drag.start_pos) / (GRID_SIZE * view.zoom);
                let nx = ((drag.orig_offset.0 + delta_g.x) * 4.0).round() / 4.0;
                let ny = ((drag.orig_offset.1 + delta_g.y) * 4.0).round() / 4.0;
                if let Some(comp) = schematic
                    .components
                    .iter_mut()
                    .find(|c| c.id == drag.comp_id)
                {
                    if drag.is_value {
                        comp.value_offset = Some((nx, ny));
                    } else {
                        comp.label_offset = Some((nx, ny));
                    }
                }
            }
            if response.drag_stopped_by(egui::PointerButton::Primary) {
                view.dragging_text = None;
                action = SchematicAction::Modified;
            }
        }
        if let Some(start) = view.marquee_start {
            if let Some(cur) = response.hover_pos() {
                let box_rect = Rect::from_two_pos(start, cur).intersect(rect);
                painter.rect_filled(
                    box_rect,
                    0.0,
                    Color32::from_rgba_unmultiplied(0, 120, 215, 30),
                );
                painter.rect_stroke(
                    box_rect,
                    0.0,
                    Stroke::new(1.0_f32, Color32::from_rgb(0, 120, 215)),
                );
            }
            if response.drag_stopped_by(egui::PointerButton::Primary) {
                if let Some(end) = response.interact_pointer_pos() {
                    let box_rect = Rect::from_two_pos(start, end).intersect(rect);
                    if box_rect.width() > 6.0 || box_rect.height() > 6.0 {
                        let shift = ui.input(|i| i.modifiers.shift);
                        if !shift {
                            view.clear_selection();
                        }
                        for c in &schematic.components {
                            let p = to_screen(c.x as f32, c.y as f32, view);
                            if box_rect.contains(p) {
                                view.selected_components.insert(c.id);
                            }
                        }
                        for w in &schematic.wires {
                            let p1 = to_screen(w.x1 as f32, w.y1 as f32, view);
                            let p2 = to_screen(w.x2 as f32, w.y2 as f32, view);
                            let w_rect = Rect::from_two_pos(p1, p2);
                            if box_rect.intersects(w_rect) {
                                view.selected_wires.insert(w.id);
                            }
                        }
                        for d in &schematic.directives {
                            let p = to_screen(d.x as f32, d.y as f32, view);
                            if box_rect.contains(p) {
                                view.selected_directives.insert(d.id);
                            }
                        }
                    }
                }
                view.marquee_start = None;
            }
        }
    }

    // Handle right-click cancel
    if response.secondary_clicked() {
        view.wire_start = None;
        view.moving_comp_id = None;
        view.moving_anchor = None;
        view.marquee_start = None;
        *tool = EditorTool::SelectOrProbe;
    }

    // Handle double-click to edit component or directive
    if response.double_clicked() {
        let click_cid = response
            .interact_pointer_pos()
            .and_then(|pos| {
                text_hitboxes
                    .iter()
                    .find(|(_, _, r)| r.expand(4.0).contains(pos))
                    .map(|(cid, _, _)| *cid)
            })
            .or_else(|| {
                view.cursor_grid
                    .and_then(|(gx, gy)| hit_test_component(gx, gy, schematic))
            });
        if let Some(cid) = click_cid {
            return SchematicAction::EditComponent(cid);
        }
        if let Some((gx, gy)) = view.cursor_grid {
            if let Some(did) = hit_test_directive(gx, gy, schematic) {
                return SchematicAction::EditDirective(did);
            }
        }
    }

    // Handle Alt+click on a component to compute & probe Power P(comp)
    if response.clicked() && ui.input(|i| i.modifiers.alt) {
        if matches!(*tool, EditorTool::SelectOrProbe | EditorTool::ProbeCurrent) {
            if let (Some((gx, gy)), Some(sim)) = (view.cursor_grid, sim_result) {
                if sim.success {
                    if let Some((cid, _)) = find_power_probe_target(gx, gy, schematic, netlist, sim)
                    {
                        return SchematicAction::ProbePower(cid);
                    }
                }
            }
        }
    }

    // Handle left-click
    if response.clicked() && !ui.input(|i| i.modifiers.alt) {
        if let Some((gx, gy)) = view.cursor_grid {
            match *tool {
                EditorTool::Wire => {
                    if let Some((sx, sy)) = view.wire_start {
                        if (sx, sy) != (gx, gy) {
                            undo_stack.push(schematic.clone());
                            if sx != gx && sy != gy {
                                schematic.add_wire(sx, sy, gx, sy);
                                schematic.add_wire(gx, sy, gx, gy);
                            } else {
                                schematic.add_wire(sx, sy, gx, gy);
                            }
                            schematic.split_wires_for_all_components();
                            view.wire_start = Some((gx, gy));
                            action = SchematicAction::Modified;
                        } else {
                            view.wire_start = None;
                        }
                    } else {
                        view.wire_start = Some((gx, gy));
                    }
                }
                EditorTool::Place(kind) => {
                    if kind == ComponentKind::NetLabel {
                        return SchematicAction::RequestNewNetLabel(gx, gy);
                    }
                    undo_stack.push(schematic.clone());
                    let refdes = schematic.next_refdes(kind);
                    let (val, custom_sym) = if kind == ComponentKind::Custom {
                        let sym = view.active_custom_symbol.clone();
                        let v = sym
                            .as_ref()
                            .map(|s| s.default_value.clone())
                            .unwrap_or_else(|| kind.default_value().to_string());
                        (v, sym)
                    } else {
                        (kind.default_value().to_string(), None)
                    };
                    let id = schematic.add_component(kind, refdes, val, gx, gy);
                    if let Some(c) = schematic.components.iter_mut().find(|c| c.id == id) {
                        c.rotation = view.place_rotation;
                        c.mirror = view.place_mirror;
                        c.custom_symbol = custom_sym;
                        if kind == ComponentKind::Custom {
                            c.hdl = view.active_hdl_block.clone();
                        }
                    }
                    schematic.split_wires_for_component(id);
                    action = SchematicAction::Modified;
                }
                EditorTool::PlaceDirective => {
                    *tool = EditorTool::SelectOrProbe;
                    return SchematicAction::RequestNewDirective(gx, gy);
                }
                EditorTool::Move | EditorTool::Drag => {
                    let is_drag = *tool == EditorTool::Drag;
                    if let Some((ax, ay)) = view.moving_anchor {
                        let dx = gx - ax;
                        let dy = gy - ay;
                        if dx != 0 || dy != 0 {
                            undo_stack.push(schematic.clone());
                            if is_drag {
                                drag_with_wires(
                                    schematic,
                                    &view.selected_components,
                                    &view.selected_wires,
                                    &view.selected_directives,
                                    dx,
                                    dy,
                                );
                            } else {
                                for c in &mut schematic.components {
                                    if view.selected_components.contains(&c.id) {
                                        c.x += dx;
                                        c.y += dy;
                                    }
                                }
                                for w in &mut schematic.wires {
                                    if view.selected_wires.contains(&w.id) {
                                        w.x1 += dx;
                                        w.y1 += dy;
                                        w.x2 += dx;
                                        w.y2 += dy;
                                    }
                                }
                                for d in &mut schematic.directives {
                                    if view.selected_directives.contains(&d.id) {
                                        d.x += dx;
                                        d.y += dy;
                                    }
                                }
                            }
                            action = SchematicAction::Modified;
                        }
                        view.moving_anchor = None;
                        *tool = EditorTool::SelectOrProbe;
                    } else if let Some(cid) = view.moving_comp_id {
                        if let Some(orig) = schematic.components.iter().find(|c| c.id == cid) {
                            let dx = gx - orig.x;
                            let dy = gy - orig.y;
                            if dx != 0 || dy != 0 {
                                undo_stack.push(schematic.clone());
                                if is_drag {
                                    let mut cset = HashSet::new();
                                    cset.insert(cid);
                                    drag_with_wires(
                                        schematic,
                                        &cset,
                                        &HashSet::new(),
                                        &HashSet::new(),
                                        dx,
                                        dy,
                                    );
                                } else {
                                    if let Some(c) =
                                        schematic.components.iter_mut().find(|c| c.id == cid)
                                    {
                                        c.x = gx;
                                        c.y = gy;
                                    }
                                    schematic.split_wires_for_component(cid);
                                }
                                action = SchematicAction::Modified;
                            }
                        }
                        view.moving_comp_id = None;
                    } else if let Some(cid) = hit_test_component(gx, gy, schematic) {
                        if view.selected_components.contains(&cid) {
                            view.moving_anchor = Some((gx, gy));
                        } else {
                            view.clear_selection();
                            view.moving_comp_id = Some(cid);
                        }
                    } else if view.has_selection() {
                        view.moving_anchor = Some((gx, gy));
                    }
                }
                EditorTool::Delete => {
                    if view.has_selection() {
                        undo_stack.push(schematic.clone());
                        schematic
                            .components
                            .retain(|c| !view.selected_components.contains(&c.id));
                        schematic
                            .wires
                            .retain(|w| !view.selected_wires.contains(&w.id));
                        schematic
                            .directives
                            .retain(|d| !view.selected_directives.contains(&d.id));
                        view.clear_selection();
                        action = SchematicAction::Modified;
                    } else if let Some(cid) = hit_test_component(gx, gy, schematic) {
                        undo_stack.push(schematic.clone());
                        schematic.components.retain(|c| c.id != cid);
                        action = SchematicAction::Modified;
                    } else if let Some(wid) = hit_test_wire(gx, gy, schematic) {
                        undo_stack.push(schematic.clone());
                        schematic.wires.retain(|w| w.id != wid);
                        action = SchematicAction::Modified;
                    } else if let Some(did) = hit_test_directive(gx, gy, schematic) {
                        undo_stack.push(schematic.clone());
                        schematic.directives.retain(|d| d.id != did);
                        action = SchematicAction::Modified;
                    }
                }
                EditorTool::SelectOrProbe | EditorTool::ProbeCurrent
                if ui.input(|i| i.modifiers.alt) =>
            {
                view.hovered_probe = None;
                if let Some(sim) = sim_result {
                    if sim.success {
                        if let Some((_, p_label)) =
                            find_power_probe_target(gx, gy, schematic, netlist, sim)
                        {
                            view.hovered_probe = Some(p_label.clone());
                            let p = to_screen(gx as f32, gy as f32, view);
                            let pwr_color = Color32::from_rgb(255, 140, 0);
                            // Thermometer-style power probe icon
                            painter.circle_filled(p, 4.5, pwr_color);
                            painter.line_segment(
                                [p, p + Vec2::new(0.0, -10.0)],
                                Stroke::new(2.2_f32, pwr_color),
                            );
                            painter.text(
                                p + Vec2::new(8.0, -14.0),
                                egui::Align2::LEFT_BOTTOM,
                                &p_label,
                                FontId::monospace(12.0),
                                pwr_color,
                            );
                        }
                    }
                }
            }
            EditorTool::SelectOrProbe => {
                    let shift = ui.input(|i| i.modifiers.shift);
                    let text_cid = response.interact_pointer_pos().and_then(|pos| {
                        text_hitboxes
                            .iter()
                            .find(|(_, _, r)| r.expand(4.0).contains(pos))
                            .map(|(cid, _, _)| *cid)
                    });
                    if shift {
                        if let Some(cid) = text_cid.or_else(|| hit_test_component(gx, gy, schematic))
                        {
                            if !view.selected_components.remove(&cid) {
                                view.selected_components.insert(cid);
                            }
                        } else if let Some(wid) = hit_test_wire(gx, gy, schematic) {
                            if !view.selected_wires.remove(&wid) {
                                view.selected_wires.insert(wid);
                            }
                        } else if let Some(did) = hit_test_directive(gx, gy, schematic) {
                            if !view.selected_directives.remove(&did) {
                                view.selected_directives.insert(did);
                            }
                        }
                    } else {
                        // If user clicked directly on a component's name/value/label, highlight the component
                        if let Some(cid) = text_cid {
                            view.clear_selection();
                            view.selected_components.insert(cid);
                        } else {
                            // Probe voltage if simulation is available and user clicked a net/pin
                            if let Some(sim) = sim_result {
                                if sim.success {
                                    if let Some(probe) =
                                        find_voltage_probe_target(gx, gy, schematic, netlist, sim)
                                    {
                                        return SchematicAction::ProbeTrace(probe);
                                    }
                                }
                            }
                            // Otherwise select the clicked item (or clear selection on empty space)
                            view.clear_selection();
                            if let Some(cid) = hit_test_component(gx, gy, schematic) {
                                view.selected_components.insert(cid);
                            } else if let Some(wid) = hit_test_wire(gx, gy, schematic) {
                                view.selected_wires.insert(wid);
                            } else if let Some(did) = hit_test_directive(gx, gy, schematic) {
                                view.selected_directives.insert(did);
                            }
                        }
                    }
                }
                EditorTool::ProbeCurrent => {
                    if let Some(sim) = sim_result {
                        if sim.success {
                            if let Some(probe) =
                                find_current_probe_target(gx, gy, schematic, netlist, sim)
                            {
                                return SchematicAction::ProbeTrace(probe);
                            }
                        }
                    }
                }
            }
        }
    }

    action
}

fn lookup_trace_ci(sim: &SimulationResult, key: &str) -> Option<String> {
    if sim.traces.contains_key(key) {
        return Some(key.to_string());
    }
    sim.traces
        .keys()
        .find(|k| k.eq_ignore_ascii_case(key))
        .cloned()
}

fn find_voltage_probe_target(
    gx: i32,
    gy: i32,
    schematic: &Schematic,
    netlist: &NetlistResult,
    sim: &SimulationResult,
) -> Option<String> {
    // Check wire or net point -> V(net) (case-insensitive to match ngspice lowercased nodes)
    if let Some(net) = netlist.point_to_net.get(&(gx, gy)) {
        if net != "0" {
            let key = format!("V({net})");
            if let Some(found) = lookup_trace_ci(sim, &key) {
                return Some(found);
            }
        }
    }
    if let Some(wid) = hit_test_wire(gx, gy, schematic) {
        if let Some(net) = netlist.wire_to_net.get(&wid) {
            if net != "0" {
                let key = format!("V({net})");
                if let Some(found) = lookup_trace_ci(sim, &key) {
                    return Some(found);
                }
            }
        }
    }
    None
}

fn find_current_probe_target(
    gx: i32,
    gy: i32,
    schematic: &Schematic,
    netlist: &NetlistResult,
    sim: &SimulationResult,
) -> Option<String> {
    // Match component pin/node (±1 grid unit) OR component body (±3 grid units) -> terminal/branch current
    let hit = schematic
        .components
        .iter()
        .filter(|c| !matches!(c.kind, ComponentKind::Ground | ComponentKind::NetLabel))
        .filter_map(|c| {
            let body_hit = (c.x - gx).abs() <= 3 && (c.y - gy).abs() <= 3;
            let body_dist = (c.x - gx).abs() + (c.y - gy).abs();
            let pin_hit = component_pins_world(c)
                .into_iter()
                .enumerate()
                .filter(|(_, (_, px, py))| (px - gx).abs() <= 1 && (py - gy).abs() <= 1)
                .map(|(p_idx, (p_name, px, py))| {
                    ((px - gx).abs() + (py - gy).abs(), p_idx, p_name)
                })
                .min_by_key(|(d, _, _)| *d);
            if let Some((pd, p_idx, p_name)) = pin_hit {
                Some((c, pd, Some((p_idx, p_name))))
            } else if body_hit {
                Some((c, body_dist + 2, None))
            } else {
                None
            }
        })
        .min_by_key(|(_, dist, _)| *dist);

    if let Some((comp, _, pin_info)) = hit {
        if let Some(spice_name) = netlist.comp_spice_names.get(&comp.id) {
            let clean = spice_name.trim_start_matches(['X', 'x']);
            let clean_low = clean.to_lowercase();
            let spice_low = spice_name.to_lowercase();

            let mut candidates: Vec<String> = Vec::new();
            match comp.kind {
                ComponentKind::Nmos | ComponentKind::Pmos => {
                    let prefix = match pin_info.as_ref().map(|(_, n)| n.as_str()) {
                        Some("G") => "Ig",
                        Some("S") => "Is",
                        _ => "Id",
                    };
                    candidates.push(format!("{prefix}({spice_low})"));
                    candidates.push(format!("{prefix}({clean_low})"));
                    candidates.push(format!("Id({spice_low})"));
                }
                ComponentKind::Npn | ComponentKind::Pnp => {
                    let prefix = match pin_info.as_ref().map(|(_, n)| n.as_str()) {
                        Some("B") => "Ib",
                        Some("E") => "Ie",
                        _ => "Ic",
                    };
                    candidates.push(format!("{prefix}({spice_low})"));
                    candidates.push(format!("{prefix}({clean_low})"));
                    candidates.push(format!("Ic({spice_low})"));
                }
                ComponentKind::OpAmp => {
                    let pin_lbl = match pin_info.as_ref().map(|(_, n)| n.as_str()) {
                        Some("IN+") => "in+",
                        Some("IN-") => "in-",
                        Some("V+") => "v+",
                        Some("V-") => "v-",
                        _ => "out",
                    };
                    candidates.push(format!("I({clean_low}:{pin_lbl})"));
                    candidates.push(format!("I({spice_low}:{pin_lbl})"));
                    candidates.push(format!("I({clean_low}:out)"));
                }
                ComponentKind::Custom => {
                    let pnum = pin_info.map(|(idx, _)| idx + 1).unwrap_or(1);
                    candidates.push(format!("I({spice_low}:{pnum})"));
                    candidates.push(format!("I({clean_low}:{pnum})"));
                    candidates.push(format!("I({spice_low}:1)"));
                }
                _ => {}
            }
            candidates.push(format!("I({spice_low})"));
            candidates.push(format!("I({spice_name})"));
            candidates.push(format!("I({clean_low})"));
            candidates.push(format!("I({clean})"));
            candidates.push(format!("I(@{clean_low}[i])"));

            for key in candidates {
                if let Some(found) = lookup_trace_ci(sim, &key) {
                    return Some(found);
                }
            }
        }
    }
    None
}

fn find_power_probe_target(
    gx: i32,
    gy: i32,
    schematic: &Schematic,
    netlist: &NetlistResult,
    sim: &SimulationResult,
) -> Option<(u64, String)> {
    let comp = schematic
        .components
        .iter()
        .filter(|c| !matches!(c.kind, ComponentKind::Ground | ComponentKind::NetLabel))
        .filter(|c| (c.x - gx).abs() <= 3 && (c.y - gy).abs() <= 3)
        .min_by_key(|c| (c.x - gx).abs() + (c.y - gy).abs())?;
    let spice_name = netlist.comp_spice_names.get(&comp.id)?;
    let clean_low = spice_name.trim_start_matches(['X', 'x']).to_lowercase();
    let has_current = match comp.kind {
        ComponentKind::Nmos | ComponentKind::Pmos => {
            sim.traces.contains_key(&format!("Id({clean_low})"))
        }
        ComponentKind::Npn | ComponentKind::Pnp => {
            sim.traces.contains_key(&format!("Ic({clean_low})"))
        }
        ComponentKind::OpAmp => sim.traces.contains_key(&format!("I({clean_low}:out)")),
        ComponentKind::Custom => sim.traces.contains_key(&format!("I({clean_low}:1)")),
        _ => {
            sim.traces.contains_key(&format!("I({clean_low})"))
                || sim
                    .traces
                    .contains_key(&format!("I({})", spice_name.to_lowercase()))
        }
    };
    if has_current {
        Some((comp.id, format!("P({clean_low})")))
    } else {
        None
    }
}

fn hit_test_component(gx: i32, gy: i32, schematic: &Schematic) -> Option<u64> {
    for c in &schematic.components {
        if (c.x - gx).abs() <= 2 && (c.y - gy).abs() <= 2 {
            return Some(c.id);
        }
    }
    None
}

fn hit_test_wire(gx: i32, gy: i32, schematic: &Schematic) -> Option<u64> {
    for w in &schematic.wires {
        if w.x1 == w.x2 && (gx - w.x1).abs() == 0 {
            let (lo, hi) = (w.y1.min(w.y2), w.y1.max(w.y2));
            if gy >= lo && gy <= hi {
                return Some(w.id);
            }
        } else if w.y1 == w.y2 && (gy - w.y1).abs() == 0 {
            let (lo, hi) = (w.x1.min(w.x2), w.x1.max(w.x2));
            if gx >= lo && gx <= hi {
                return Some(w.id);
            }
        }
    }
    None
}

fn hit_test_directive(gx: i32, gy: i32, schematic: &Schematic) -> Option<u64> {
    for d in &schematic.directives {
        if gx >= d.x - 1 && gx <= d.x + 8 && (gy - d.y).abs() <= 1 {
            return Some(d.id);
        }
    }
    None
}

fn draw_junctions(
    painter: &egui::Painter,
    schematic: &Schematic,
    to_screen: &dyn Fn(f32, f32) -> Pos2,
    zoom: f32,
) {
    use std::collections::HashMap;
    let mut counts: HashMap<(i32, i32), usize> = HashMap::new();
    for w in &schematic.wires {
        *counts.entry((w.x1, w.y1)).or_insert(0) += 1;
        *counts.entry((w.x2, w.y2)).or_insert(0) += 1;
    }
    for (pt, cnt) in counts {
        // Also count T-junctions where an endpoint lands on the middle of another wire
        let mut on_mid = false;
        for w in &schematic.wires {
            let is_end = (w.x1, w.y1) == pt || (w.x2, w.y2) == pt;
            if !is_end {
                let on_v = w.x1 == w.x2
                    && pt.0 == w.x1
                    && pt.1 > w.y1.min(w.y2)
                    && pt.1 < w.y1.max(w.y2);
                let on_h = w.y1 == w.y2
                    && pt.1 == w.y1
                    && pt.0 > w.x1.min(w.x2)
                    && pt.0 < w.x1.max(w.x2);
                if on_v || on_h {
                    on_mid = true;
                    break;
                }
            }
        }
        if cnt >= 3 || on_mid {
            let p = to_screen(pt.0 as f32, pt.1 as f32);
            let r = (2.8 * zoom).clamp(2.0, 5.0);
            painter.rect_filled(
                Rect::from_center_size(p, Vec2::splat(r * 2.0)),
                0.0,
                Color32::from_rgb(0, 0, 180),
            );
        }
    }
}

fn default_label_offset(c: &Component) -> (f32, f32) {
    let (rx, ry) = transform_offset(1, -1, c.rotation, c.mirror);
    (rx as f32 * 0.8, ry as f32 * 0.6)
}

fn default_value_offset(c: &Component) -> (f32, f32) {
    if c.kind == ComponentKind::NetLabel {
        return (0.0, -0.6);
    }
    let (vx, vy) = transform_offset(1, 1, c.rotation, c.mirror);
    (vx as f32 * 0.8, vy as f32 * 0.6)
}

fn draw_component(
    painter: &egui::Painter,
    c: &Component,
    to_screen: &dyn Fn(f32, f32) -> Pos2,
    zoom: f32,
    ghost: bool,
) -> (Option<Rect>, Option<Rect>) {
    let alpha = if ghost { 140 } else { 255 };
    let sym_color = Color32::from_rgba_unmultiplied(0, 0, 150, alpha);
    let stroke = Stroke::new((1.6 * zoom).clamp(1.0, 3.0), sym_color);
    let text_color = Color32::from_rgba_unmultiplied(0, 0, 0, alpha);

    let pt = |lx: f32, ly: f32| -> Pos2 {
        let (rx, ry) = transform_f(lx, ly, c.rotation, c.mirror);
        to_screen(c.x as f32 + rx, c.y as f32 + ry)
    };

    let mut ref_rect = None;
    let mut val_rect = None;

    match c.kind {
        ComponentKind::Resistor => {
            let pts = [
                pt(0.0, -2.0),
                pt(0.0, -1.2),
                pt(0.5, -0.9),
                pt(-0.5, -0.5),
                pt(0.5, -0.1),
                pt(-0.5, 0.3),
                pt(0.5, 0.7),
                pt(-0.5, 1.1),
                pt(0.0, 1.3),
                pt(0.0, 2.0),
            ];
            for w in pts.windows(2) {
                painter.line_segment([w[0], w[1]], stroke);
            }
        }
        ComponentKind::Capacitor => {
            painter.line_segment([pt(0.0, -2.0), pt(0.0, -0.3)], stroke);
            painter.line_segment([pt(-0.7, -0.3), pt(0.7, -0.3)], stroke);
            painter.line_segment([pt(-0.7, 0.3), pt(0.7, 0.3)], stroke);
            painter.line_segment([pt(0.0, 0.3), pt(0.0, 2.0)], stroke);
        }
        ComponentKind::Inductor => {
            painter.line_segment([pt(0.0, -2.0), pt(0.0, -1.2)], stroke);
            for i in 0..3 {
                let y0 = -1.2 + i as f32 * 0.8;
                let center = pt(0.0, y0 + 0.4);
                painter.circle_stroke(center, 0.4 * GRID_SIZE * zoom, stroke);
            }
            painter.line_segment([pt(0.0, 1.2), pt(0.0, 2.0)], stroke);
        }
        ComponentKind::Diode => {
            painter.line_segment([pt(0.0, -2.0), pt(0.0, -0.5)], stroke);
            painter.line_segment([pt(-0.6, -0.5), pt(0.6, -0.5)], stroke);
            painter.line_segment([pt(-0.6, -0.5), pt(0.0, 0.5)], stroke);
            painter.line_segment([pt(0.6, -0.5), pt(0.0, 0.5)], stroke);
            painter.line_segment([pt(-0.6, 0.5), pt(0.6, 0.5)], stroke);
            painter.line_segment([pt(0.0, 0.5), pt(0.0, 2.0)], stroke);
        }
        ComponentKind::VoltageSource => {
            painter.line_segment([pt(0.0, -2.0), pt(0.0, -1.0)], stroke);
            painter.circle_stroke(pt(0.0, 0.0), 1.0 * GRID_SIZE * zoom, stroke);
            painter.line_segment([pt(0.0, 1.0), pt(0.0, 2.0)], stroke);
            // '+' and '-' marks
            painter.line_segment([pt(-0.25, -0.45), pt(0.25, -0.45)], stroke);
            painter.line_segment([pt(0.0, -0.7), pt(0.0, -0.2)], stroke);
            painter.line_segment([pt(-0.25, 0.5), pt(0.25, 0.5)], stroke);
        }
        ComponentKind::CurrentSource => {
            painter.line_segment([pt(0.0, -2.0), pt(0.0, -1.0)], stroke);
            painter.circle_stroke(pt(0.0, 0.0), 1.0 * GRID_SIZE * zoom, stroke);
            painter.line_segment([pt(0.0, 1.0), pt(0.0, 2.0)], stroke);
            painter.line_segment([pt(0.0, -0.5), pt(0.0, 0.5)], stroke);
            painter.line_segment([pt(-0.3, 0.2), pt(0.0, 0.5)], stroke);
            painter.line_segment([pt(0.3, 0.2), pt(0.0, 0.5)], stroke);
        }
        ComponentKind::OpAmp => {
            // Triangle body
            let a = pt(-1.2, -1.6);
            let b = pt(-1.2, 1.6);
            let tip = pt(1.4, 0.0);
            painter.line_segment([a, b], stroke);
            painter.line_segment([b, tip], stroke);
            painter.line_segment([tip, a], stroke);
            // Pins
            painter.line_segment([pt(-2.0, -1.0), pt(-1.2, -1.0)], stroke);
            painter.line_segment([pt(-2.0, 1.0), pt(-1.2, 1.0)], stroke);
            painter.line_segment([pt(1.4, 0.0), pt(2.0, 0.0)], stroke);
            painter.line_segment([pt(0.0, -2.0), pt(0.0, -0.85)], stroke);
            painter.line_segment([pt(0.0, 2.0), pt(0.0, 0.85)], stroke);
            // '+' and '-' labels inside triangle
            let font = FontId::monospace((11.0 * zoom).clamp(7.0, 18.0));
            painter.text(pt(-0.9, -1.0), egui::Align2::LEFT_CENTER, "+", font.clone(), sym_color);
            painter.text(pt(-0.9, 1.0), egui::Align2::LEFT_CENTER, "-", font, sym_color);
        }
        ComponentKind::Nmos | ComponentKind::Pmos => {
            let is_nmos = c.kind == ComponentKind::Nmos;
            // Gate lead & oxide bar
            painter.line_segment([pt(-2.0, 1.0), pt(-0.35, 1.0)], stroke);
            painter.line_segment([pt(-0.35, -0.95), pt(-0.35, 1.0)], stroke);

            // Three separated channel segments (Drain, Bulk, Source)
            painter.line_segment([pt(0.15, -1.05), pt(0.15, -0.50)], stroke);
            painter.line_segment([pt(0.15, -0.26), pt(0.15, 0.26)], stroke);
            painter.line_segment([pt(0.15, 0.50), pt(0.15, 1.05)], stroke);

            // Drain & Source leads
            painter.line_segment([pt(0.15, -0.78), pt(2.0, -0.78)], stroke);
            painter.line_segment([pt(2.0, -0.78), pt(2.0, -2.0)], stroke);
            painter.line_segment([pt(0.15, 0.78), pt(2.0, 0.78)], stroke);
            painter.line_segment([pt(2.0, 0.78), pt(2.0, 2.0)], stroke);

            // Bulk-to-Source tie
            painter.line_segment([pt(0.15, 0.0), pt(1.15, 0.0)], stroke);
            painter.line_segment([pt(1.15, 0.0), pt(1.15, 0.78)], stroke);

            // Filled bulk arrow: inward (left) for NMOS, outward (right) for PMOS
            let arrow = if is_nmos {
                vec![pt(0.20, 0.0), pt(0.75, -0.27), pt(0.75, 0.27)]
            } else {
                vec![pt(1.02, 0.0), pt(0.47, -0.27), pt(0.47, 0.27)]
            };
            painter.add(egui::Shape::convex_polygon(arrow, sym_color, Stroke::NONE));

            // Internal body diode between Source and Drain
            painter.line_segment([pt(1.58, -0.78), pt(1.58, 0.78)], stroke);
            if is_nmos {
                let d_tri = vec![pt(1.58, -0.20), pt(1.34, 0.18), pt(1.82, 0.18)];
                painter.add(egui::Shape::convex_polygon(d_tri, sym_color, Stroke::NONE));
                painter.line_segment([pt(1.34, -0.20), pt(1.82, -0.20)], stroke);
            } else {
                let d_tri = vec![pt(1.58, 0.20), pt(1.34, -0.18), pt(1.82, -0.18)];
                painter.add(egui::Shape::convex_polygon(d_tri, sym_color, Stroke::NONE));
                painter.line_segment([pt(1.34, 0.20), pt(1.82, 0.20)], stroke);
            }
        }
        ComponentKind::Npn | ComponentKind::Pnp => {
            let is_npn = c.kind == ComponentKind::Npn;
            // Base lead & vertical base bar (slightly thicker for clarity)
            let base_stroke = Stroke::new((2.4 * zoom).clamp(1.5, 4.0), sym_color);
            painter.line_segment([pt(-2.0, 0.0), pt(-0.35, 0.0)], stroke);
            painter.line_segment([pt(-0.35, -1.0), pt(-0.35, 1.0)], base_stroke);

            // Collector & Emitter legs
            painter.line_segment([pt(-0.35, -0.40), pt(2.0, -1.45)], stroke);
            painter.line_segment([pt(2.0, -1.45), pt(2.0, -2.0)], stroke);
            painter.line_segment([pt(-0.35, 0.40), pt(2.0, 1.45)], stroke);
            painter.line_segment([pt(2.0, 1.45), pt(2.0, 2.0)], stroke);

            // Filled emitter arrow: outward for NPN, inward (toward base) for PNP
            let (ax, ay) = (-0.35_f32, 0.40_f32);
            let (bx, by) = (2.0_f32, 1.45_f32);
            let dx = bx - ax;
            let dy = by - ay;
            let len = (dx * dx + dy * dy).sqrt();
            let (ux, uy) = (dx / len, dy / len);
            let (nx, ny) = (-uy, ux);
            let half_w = 0.27_f32;

            let arrow = if is_npn {
                let tip = (ax + dx * 0.78, ay + dy * 0.78);
                let base = (ax + dx * 0.50, ay + dy * 0.50);
                vec![
                    pt(tip.0, tip.1),
                    pt(base.0 + nx * half_w, base.1 + ny * half_w),
                    pt(base.0 - nx * half_w, base.1 - ny * half_w),
                ]
            } else {
                let tip = (ax + dx * 0.22, ay + dy * 0.22);
                let base = (ax + dx * 0.50, ay + dy * 0.50);
                vec![
                    pt(tip.0, tip.1),
                    pt(base.0 + nx * half_w, base.1 + ny * half_w),
                    pt(base.0 - nx * half_w, base.1 - ny * half_w),
                ]
            };
            painter.add(egui::Shape::convex_polygon(arrow, sym_color, Stroke::NONE));
        }
        ComponentKind::Ground => {
            painter.line_segment([pt(0.0, 0.0), pt(0.0, 0.5)], stroke);
            painter.line_segment([pt(-0.7, 0.5), pt(0.7, 0.5)], stroke);
            painter.line_segment([pt(-0.7, 0.5), pt(0.0, 1.2)], stroke);
            painter.line_segment([pt(0.7, 0.5), pt(0.0, 1.2)], stroke);
        }
        ComponentKind::NetLabel => {
            painter.line_segment([pt(0.0, 0.0), pt(0.0, -0.5)], stroke);
            let font = FontId::proportional((12.0 * zoom).clamp(8.0, 20.0));
            let (vx, vy) = c.value_offset.unwrap_or_else(|| default_value_offset(c));
            val_rect = Some(painter.text(
                to_screen(c.x as f32 + vx, c.y as f32 + vy),
                egui::Align2::CENTER_BOTTOM,
                &c.value,
                font,
                text_color,
            ));
        }
        ComponentKind::Custom => {
            if let Some(sym) = &c.custom_symbol {
                for s in &sym.shapes {
                    match s {
                        SymbolShape::Line { x1, y1, x2, y2 } => {
                            painter.line_segment([pt(*x1, *y1), pt(*x2, *y2)], stroke);
                        }
                        SymbolShape::Rect { x1, y1, x2, y2 } => {
                            let r = Rect::from_two_pos(pt(*x1, *y1), pt(*x2, *y2));
                            painter.rect_stroke(r, 0.0, stroke);
                        }
                        SymbolShape::Circle { cx, cy, r } => {
                            painter.circle_stroke(pt(*cx, *cy), *r * GRID_SIZE * zoom, stroke);
                        }
                        SymbolShape::Text { x, y, text } => {
                            painter.text(
                                pt(*x, *y),
                                egui::Align2::LEFT_CENTER,
                                text,
                                FontId::proportional((11.5 * zoom).clamp(8.0, 18.0)),
                                text_color,
                            );
                        }
                    }
                }
            }
        }
    }

    // Draw pin dots
    for (_, px, py) in component_pins_world(c) {
        let p = to_screen(px as f32, py as f32);
        painter.rect_filled(
            Rect::from_center_size(p, Vec2::splat((3.0 * zoom).clamp(2.0, 5.0))),
            0.0,
            sym_color,
        );
    }

    // Draw RefDes & Value text
    if c.kind.has_refdes() {
        let font = FontId::proportional((11.5 * zoom).clamp(8.0, 18.0));
        let (lx, ly) = c.label_offset.unwrap_or_else(|| default_label_offset(c));
        let (vx, vy) = c.value_offset.unwrap_or_else(|| default_value_offset(c));
        if !c.refdes.is_empty() {
            ref_rect = Some(painter.text(
                to_screen(c.x as f32 + lx, c.y as f32 + ly),
                egui::Align2::LEFT_CENTER,
                &c.refdes,
                font.clone(),
                text_color,
            ));
        }
        if !c.value.is_empty() {
            val_rect = Some(painter.text(
                to_screen(c.x as f32 + vx, c.y as f32 + vy),
                egui::Align2::LEFT_CENTER,
                &c.value,
                font,
                text_color,
            ));
        }
    }
    (ref_rect, val_rect)
}

fn transform_f(lx: f32, ly: f32, rotation: i32, mirror: bool) -> (f32, f32) {
    let mx = if mirror { -lx } else { lx };
    let my = ly;
    let r = ((rotation % 360) + 360) % 360;
    match r {
        90 => (-my, mx),
        180 => (-mx, -my),
        270 => (my, -mx),
        _ => (mx, my),
    }
}

/// Move components/wires/directives by `(dx, dy)` while orthogonally stretching
/// any connected wires so electrical connections remain intact.
pub fn drag_with_wires(
    schematic: &mut Schematic,
    comp_ids: &HashSet<u64>,
    wire_ids: &HashSet<u64>,
    dir_ids: &HashSet<u64>,
    dx: i32,
    dy: i32,
) {
    if dx == 0 && dy == 0 {
        return;
    }

    let mut moved_pins: HashSet<(i32, i32)> = HashSet::new();
    let mut unmoved_pins: HashSet<(i32, i32)> = HashSet::new();
    for c in &schematic.components {
        if comp_ids.contains(&c.id) {
            for (_, px, py) in component_pins_world(c) {
                moved_pins.insert((px, py));
            }
        } else {
            for (_, px, py) in component_pins_world(c) {
                unmoved_pins.insert((px, py));
            }
        }
    }
    for w in &schematic.wires {
        if wire_ids.contains(&w.id) {
            moved_pins.insert((w.x1, w.y1));
            moved_pins.insert((w.x2, w.y2));
        }
    }

    for c in &mut schematic.components {
        if comp_ids.contains(&c.id) {
            c.x += dx;
            c.y += dy;
        }
    }
    for d in &mut schematic.directives {
        if dir_ids.contains(&d.id) {
            d.x += dx;
            d.y += dy;
        }
    }

    let mut extra_wires: Vec<(i32, i32, i32, i32)> = Vec::new();
    let mut t_bridged: HashSet<(i32, i32)> = HashSet::new();

    for w in &mut schematic.wires {
        if wire_ids.contains(&w.id) {
            w.x1 += dx;
            w.y1 += dy;
            w.x2 += dx;
            w.y2 += dy;
            continue;
        }
        let m1 = moved_pins.contains(&(w.x1, w.y1));
        let m2 = moved_pins.contains(&(w.x2, w.y2));
        if m1 && m2 {
            w.x1 += dx;
            w.y1 += dy;
            w.x2 += dx;
            w.y2 += dy;
        } else if m1 {
            let was_horiz = w.y1 == w.y2;
            let p_new = (w.x1 + dx, w.y1 + dy);
            let q = (w.x2, w.y2);
            if p_new.0 == q.0 || p_new.1 == q.1 {
                w.x1 = p_new.0;
                w.y1 = p_new.1;
            } else if was_horiz {
                w.x1 = p_new.0;
                w.y1 = q.1;
                extra_wires.push((p_new.0, p_new.1, p_new.0, q.1));
            } else {
                w.x1 = q.0;
                w.y1 = p_new.1;
                extra_wires.push((p_new.0, p_new.1, q.0, p_new.1));
            }
        } else if m2 {
            let was_horiz = w.y1 == w.y2;
            let p_new = (w.x2 + dx, w.y2 + dy);
            let q = (w.x1, w.y1);
            if p_new.0 == q.0 || p_new.1 == q.1 {
                w.x2 = p_new.0;
                w.y2 = p_new.1;
            } else if was_horiz {
                w.x2 = p_new.0;
                w.y2 = q.1;
                extra_wires.push((p_new.0, p_new.1, p_new.0, q.1));
            } else {
                w.x2 = q.0;
                w.y2 = p_new.1;
                extra_wires.push((p_new.0, p_new.1, q.0, p_new.1));
            }
        } else {
            // Check if a moved pin touched the interior of this unmoved orthogonal wire (T-junction)
            for &(px, py) in &moved_pins {
                if t_bridged.contains(&(px, py)) {
                    continue;
                }
                let on_interior = (w.x1 == w.x2
                    && px == w.x1
                    && py > w.y1.min(w.y2)
                    && py < w.y1.max(w.y2))
                    || (w.y1 == w.y2
                        && py == w.y1
                        && px > w.x1.min(w.x2)
                        && px < w.x1.max(w.x2));
                if on_interior {
                    t_bridged.insert((px, py));
                    let p_new = (px + dx, py + dy);
                    if px == p_new.0 || py == p_new.1 {
                        extra_wires.push((px, py, p_new.0, p_new.1));
                    } else {
                        extra_wires.push((px, py, p_new.0, py));
                        extra_wires.push((p_new.0, py, p_new.0, p_new.1));
                    }
                }
            }
        }
    }

    // Preserve direct pin-to-pin connections between moved and stationary components
    for &(px, py) in &moved_pins {
        if unmoved_pins.contains(&(px, py)) && !t_bridged.contains(&(px, py)) {
            t_bridged.insert((px, py));
            let p_new = (px + dx, py + dy);
            if px == p_new.0 || py == p_new.1 {
                extra_wires.push((px, py, p_new.0, p_new.1));
            } else {
                extra_wires.push((px, py, p_new.0, py));
                extra_wires.push((p_new.0, py, p_new.0, p_new.1));
            }
        }
    }

    schematic.wires.retain(|w| w.x1 != w.x2 || w.y1 != w.y2);
    for (x1, y1, x2, y2) in extra_wires {
        schematic.add_wire(x1, y1, x2, y2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drag_directly_connected_components_keeps_connection() {
        let mut sch = Schematic::new_empty("direct_pin_drag.asc");
        // R1 at (10, 10) rot 90 -> pins (8, 10) and (12, 10)
        let r1 = sch.add_component(ComponentKind::Resistor, "R1".into(), "1k".into(), 10, 10);
        sch.components.last_mut().unwrap().rotation = 90;
        // C1 at (12, 12) rot 0 -> pins (12, 10) and (12, 14) -> pin (12, 10) directly touches R1!
        let _c1 = sch.add_component(ComponentKind::Capacitor, "C1".into(), "100n".into(), 12, 12);

        let mut cset = HashSet::new();
        cset.insert(r1);
        drag_with_wires(&mut sch, &cset, &HashSet::new(), &HashSet::new(), -2, -2);

        let net = sch.to_spice_netlist();
        // R1 pin 2 moved to (10, 8), C1 pin 1 stayed at (12, 10) -> must still share the same net
        assert_eq!(
            net.point_to_net[&(10, 8)],
            net.point_to_net[&(12, 10)],
            "directly connected components must remain connected after stretch/drag"
        );
    }

    #[test]
    fn test_drag_with_wires_keeps_connections() {
        let mut sch = Schematic::default_ltspice_demo();
        let r1_id = sch
            .components
            .iter()
            .find(|c| c.refdes == "R1")
            .expect("R1 exists")
            .id;
        let mut cset = HashSet::new();
        cset.insert(r1_id);

        drag_with_wires(&mut sch, &cset, &HashSet::new(), &HashSet::new(), 1, -2);
        let net = sch.to_spice_netlist();
        // V(in) and V(out) should still be connected through R1 (netlist should have R1 between out and in)
        assert!(
            net.lines
                .iter()
                .any(|l| l.starts_with("R1 ") && l.contains("in") && l.contains("out")),
            "R1 should remain connected between in and out after drag: {:?}",
            net.lines
        );
    }
}
