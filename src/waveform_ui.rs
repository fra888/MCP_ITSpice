use egui::{Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::ngspice::SimulationResult;

pub const TRACE_COLORS: [Color32; 7] = [
    Color32::from_rgb(0, 255, 0),     // Bright green (LTspice trace 1)
    Color32::from_rgb(0, 128, 255),   // Bright blue (LTspice trace 2)
    Color32::from_rgb(255, 50, 50),   // Red
    Color32::from_rgb(0, 255, 255),   // Cyan
    Color32::from_rgb(255, 0, 255),   // Magenta
    Color32::from_rgb(255, 255, 0),   // Yellow
    Color32::from_rgb(255, 140, 0),   // Orange
];

pub const CURSOR_COLORS: [Color32; 6] = [
    Color32::from_rgb(255, 230, 80),  // C1: Yellow
    Color32::from_rgb(80, 235, 255),  // C2: Cyan
    Color32::from_rgb(255, 165, 60),  // C3: Orange
    Color32::from_rgb(255, 110, 255), // C4: Magenta
    Color32::from_rgb(140, 255, 120), // C5: Light Green
    Color32::from_rgb(235, 235, 235), // C6+: White
];

pub fn is_current_trace(name: &str) -> bool {
    if let Some(open) = name.find('(') {
        let prefix = &name[..open];
        prefix.starts_with('I') || prefix.starts_with('i')
    } else {
        false
    }
}

pub fn is_power_trace(name: &str) -> bool {
    name.starts_with("P(") || name.starts_with("p(")
}

pub fn trace_unit(name: &str) -> &'static str {
    if is_power_trace(name) {
        "W"
    } else if is_current_trace(name) {
        "A"
    } else {
        "V"
    }
}

#[derive(Debug, Clone)]
pub struct GraphXAxis {
    pub x_values: Vec<f64>,
    pub x_unit: String,
    pub y_unit: Option<String>,
    pub x_range: Option<(f64, f64)>,
}

pub struct WaveformViewState {
    pub active_traces: Vec<String>,
    pub extra_graphs: Vec<Vec<String>>,
    pub focused_graph: usize,
    pub graph_x_overrides: std::collections::HashMap<usize, GraphXAxis>,
    pub x_range: Option<(f64, f64)>,
    pub y_range: Option<(f64, f64)>,
    pub y_range_i: Option<(f64, f64)>,
    pub drag_start: Option<Pos2>,
    pub cursors: Vec<(String, f64)>,
    pub dragging_cursor: Option<usize>,
    pub context_menu_pos: Option<(String, f64)>,
}

impl Default for WaveformViewState {
    fn default() -> Self {
        Self {
            active_traces: Vec::new(),
            extra_graphs: Vec::new(),
            focused_graph: 0,
            graph_x_overrides: std::collections::HashMap::new(),
            x_range: None,
            y_range: None,
            y_range_i: None,
            drag_start: None,
            cursors: Vec::new(),
            dragging_cursor: None,
            context_menu_pos: None,
        }
    }
}

impl WaveformViewState {
    pub fn reset_zoom(&mut self) {
        self.x_range = None;
        self.y_range = None;
        self.y_range_i = None;
        for gx in self.graph_x_overrides.values_mut() {
            gx.x_range = None;
        }
    }

    pub fn set_graph_xaxis(
        &mut self,
        graph_idx: usize,
        x_values: Vec<f64>,
        x_unit: String,
        y_unit: Option<String>,
    ) {
        self.graph_x_overrides.insert(
            graph_idx,
            GraphXAxis {
                x_values,
                x_unit,
                y_unit,
                x_range: None,
            },
        );
    }

    pub fn add_graph(&mut self) {
        self.extra_graphs.push(Vec::new());
        self.focused_graph = self.extra_graphs.len();
    }

    pub fn zoom_by(&mut self, factor: f64, sim: Option<&SimulationResult>) {
        let Some(sim) = sim else {
            return;
        };
        if !sim.success || sim.x_values.is_empty() {
            return;
        }
        let x_min_data = sim
            .x_values
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let x_max_data = sim
            .x_values
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        let (x0, x1) = self.x_range.unwrap_or((x_min_data, x_max_data));
        let xc = 0.5 * (x0 + x1);
        let xh = 0.5 * (x1 - x0).max(1e-15) * factor;
        self.x_range = Some((xc - xh, xc + xh));

        if let Some((y0, y1)) = self.y_range {
            let yc = 0.5 * (y0 + y1);
            let yh = 0.5 * (y1 - y0).max(1e-12) * factor;
            self.y_range = Some((yc - yh, yc + yh));
        }
    }

    pub fn toggle_trace(&mut self, name: &str) {
        let target = if self.focused_graph > 0 && self.focused_graph <= self.extra_graphs.len() {
            &mut self.extra_graphs[self.focused_graph - 1]
        } else {
            &mut self.active_traces
        };
        if let Some(pos) = target.iter().position(|t| t == name) {
            target.remove(pos);
        } else {
            target.push(name.to_string());
        }
    }
}

/// Returns `true` if the user clicked or dragged inside the waveform pane.
pub fn draw_waveform_pane(
    ui: &mut Ui,
    sim: Option<&SimulationResult>,
    view: &mut WaveformViewState,
) -> bool {
    let available = ui.available_size();
    let (rect, response) = ui.allocate_exact_size(available, Sense::click_and_drag());
    let interacted = response.clicked() || response.drag_started();
    let painter = ui.painter_at(rect);

    // Classic LTspice black background
    painter.rect_filled(rect, 0.0, Color32::BLACK);

    let Some(sim) = sim else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "No simulation data. Click Run (▶) to simulate, then click a wire/node on the schematic to probe.",
            FontId::proportional(14.0),
            Color32::from_rgb(160, 160, 160),
        );
        return interacted;
    };

    if !sim.success {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            sim.error.as_deref().unwrap_or("Simulation error"),
            FontId::monospace(13.0),
            Color32::from_rgb(255, 90, 90),
        );
        return interacted;
    }

    if sim.x_values.is_empty() {
        return interacted;
    }

    // Keep only traces that exist in this simulation result
    view.active_traces.retain(|t| sim.traces.contains_key(t));
    for g in &mut view.extra_graphs {
        g.retain(|t| sim.traces.contains_key(t));
    }
    if view.focused_graph > view.extra_graphs.len() {
        view.focused_graph = view.extra_graphs.len();
    }
    let all_traces: Vec<String> = std::iter::once(&view.active_traces)
        .chain(view.extra_graphs.iter())
        .flatten()
        .cloned()
        .collect();
    view.cursors.retain(|(t, _)| all_traces.contains(t));

    // Check if any graph has both V and secondary (I / P) traces (needs right margin for secondary axis)
    let any_dual_axis = std::iter::once(&view.active_traces)
        .chain(view.extra_graphs.iter())
        .any(|g| {
            g.iter().any(|t| !is_current_trace(t) && !is_power_trace(t))
                && g.iter().any(|t| is_current_trace(t) || is_power_trace(t))
        });

    let left_margin = 56.0;
    let right_margin = if any_dual_axis { 56.0 } else { 16.0 };
    let top_margin = 24.0;
    let bottom_margin = 24.0;

    let plot_rect = Rect::from_min_max(
        Pos2::new(rect.left() + left_margin, rect.top() + top_margin),
        Pos2::new(
            (rect.right() - right_margin).max(rect.left() + left_margin + 20.0),
            (rect.bottom() - bottom_margin).max(rect.top() + top_margin + 20.0),
        ),
    );

    // Top-right "+ Graph" button in the legend bar
    let new_graph_btn_rect = Rect::from_min_size(
        Pos2::new(rect.right() - 72.0, rect.top() + 3.0),
        Vec2::new(66.0, 18.0),
    );
    painter.rect_filled(new_graph_btn_rect, 3.0, Color32::from_rgb(40, 44, 52));
    painter.rect_stroke(
        new_graph_btn_rect,
        3.0,
        Stroke::new(1.0_f32, Color32::from_rgb(100, 110, 130)),
    );
    painter.text(
        new_graph_btn_rect.center(),
        egui::Align2::CENTER_CENTER,
        "+ Graph",
        FontId::proportional(11.0),
        Color32::from_rgb(220, 230, 245),
    );
    let mut add_new_graph_clicked = false;
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            if new_graph_btn_rect.expand(2.0).contains(pos) {
                add_new_graph_clicked = true;
            }
        }
    }

    // Compute X bounds
    let x_min_data = sim
        .x_values
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    let x_max_data = sim
        .x_values
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);

    let (x_min, x_max) = view.x_range.unwrap_or((x_min_data, x_max_data));
    let x_span = (x_max - x_min).max(1e-15);
    let x_unit = if sim.x_name == "frequency" { "Hz" } else { "s" };

    // Build list of graphs: Graph 0 is always present (same graph for V & I by default),
    // plus any user-generated extra_graphs stacked below.
    let num_graphs = 1 + view.extra_graphs.len();
    let gap = if view.graph_x_overrides.is_empty() {
        22.0_f32
    } else {
        36.0_f32
    };
    let total_gap = gap * (num_graphs.saturating_sub(1) as f32);
    let graph_h = ((plot_rect.height() - total_gap) / num_graphs as f32).max(24.0);

    let mut graph_rects: Vec<Rect> = Vec::with_capacity(num_graphs);
    for g_idx in 0..num_graphs {
        let top = plot_rect.top() + g_idx as f32 * (graph_h + gap);
        let bot = if g_idx + 1 == num_graphs {
            plot_rect.bottom()
        } else {
            (top + graph_h).min(plot_rect.bottom())
        };
        graph_rects.push(Rect::from_min_max(
            Pos2::new(plot_rect.left(), top),
            Pos2::new(plot_rect.right(), bot),
        ));
    }

    // Legend & trace removal/cursor attachment across graphs
    let mut remove_from_graph: Option<(usize, usize)> = None;
    let mut close_extra_graph: Option<usize> = None;
    let mut attach_cursor_trace: Option<String> = None;

    for g_idx in 0..num_graphs {
        let g_traces = if g_idx == 0 {
            &view.active_traces
        } else {
            &view.extra_graphs[g_idx - 1]
        };
        let g_rect = graph_rects[g_idx];
        let legend_y = if g_idx == 0 {
            rect.top() + 12.0
        } else {
            g_rect.top() - 10.0
        };
        let legend_w = if g_idx == 0 {
            (g_rect.width() - 60.0).max(40.0)
        } else {
            (g_rect.width() - 24.0).max(40.0)
        };
        let legend_count = g_traces.len().max(1);
        for (idx, trace_name) in g_traces.iter().enumerate() {
            let color = TRACE_COLORS[idx % TRACE_COLORS.len()];
            let frac = (idx as f32 + 0.5) / legend_count as f32;
            let lx = g_rect.left() + legend_w * frac;
            let mut badge = String::new();
            for (c_idx, (ct, _)) in view.cursors.iter().enumerate() {
                if ct == trace_name {
                    badge.push_str(&format!(" [{}]", c_idx + 1));
                }
            }
            let label_str = format!("{trace_name}{badge}");
            let text_rect = painter.text(
                Pos2::new(lx, legend_y),
                egui::Align2::CENTER_CENTER,
                &label_str,
                FontId::proportional(13.0),
                color,
            );
            if response.clicked() && !add_new_graph_clicked {
                if let Some(pos) = response.interact_pointer_pos() {
                    if text_rect.expand(6.0).contains(pos) {
                        attach_cursor_trace = Some(trace_name.clone());
                    }
                }
            }
            if response.secondary_clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    if text_rect.expand(6.0).contains(pos) {
                        remove_from_graph = Some((g_idx, idx));
                    }
                }
            }
        }
        if g_idx > 0 {
            let close_rect = Rect::from_min_size(
                Pos2::new(g_rect.right() - 18.0, g_rect.top() - 18.0),
                Vec2::new(16.0, 16.0),
            );
            painter.text(
                close_rect.center(),
                egui::Align2::CENTER_CENTER,
                "×",
                FontId::proportional(13.0),
                Color32::from_rgb(200, 120, 120),
            );
            if response.clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    if close_rect.expand(3.0).contains(pos) {
                        close_extra_graph = Some(g_idx - 1);
                    }
                }
            }
        }
    }

    if let Some((g_idx, t_idx)) = remove_from_graph {
        if g_idx == 0 {
            view.active_traces.remove(t_idx);
        } else if let Some(g) = view.extra_graphs.get_mut(g_idx - 1) {
            g.remove(t_idx);
        }
    }
    if let Some(eg_idx) = close_extra_graph {
        if eg_idx < view.extra_graphs.len() {
            view.extra_graphs.remove(eg_idx);
            let closed_g = eg_idx + 1;
            let old_overrides = std::mem::take(&mut view.graph_x_overrides);
            for (k, v) in old_overrides {
                if k < closed_g {
                    view.graph_x_overrides.insert(k, v);
                } else if k > closed_g {
                    view.graph_x_overrides.insert(k - 1, v);
                }
            }
            if view.focused_graph > view.extra_graphs.len() {
                view.focused_graph = view.extra_graphs.len();
            }
        }
    }
    if add_new_graph_clicked {
        view.add_graph();
    }

    // Focus graph on click inside its plot area
    if response.clicked() && !add_new_graph_clicked && close_extra_graph.is_none() {
        if let Some(pos) = response.interact_pointer_pos() {
            for (g_idx, gr) in graph_rects.iter().enumerate() {
                if gr.contains(pos) {
                    view.focused_graph = g_idx;
                    break;
                }
            }
        }
    }

    // Draw each graph (same graph for V & I with dual Y-axes; supports per-graph X-axis & unit overrides)
    let mut graph_y_bounds: Vec<(Option<(f64, f64)>, Option<(f64, f64)>)> =
        Vec::with_capacity(num_graphs);
    let mut graph_x_meta: Vec<(f64, f64, String, Option<String>)> =
        Vec::with_capacity(num_graphs);
    for g_idx in 0..num_graphs {
        let g_rect = graph_rects[g_idx];
        let g_traces = if g_idx == 0 {
            &view.active_traces
        } else {
            &view.extra_graphs[g_idx - 1]
        };
        let (g_x_vals, g_x_min, g_x_max, g_x_unit, g_y_unit_ov): (
            &[f64],
            f64,
            f64,
            String,
            Option<String>,
        ) = if let Some(gx) = view.graph_x_overrides.get(&g_idx) {
            if !gx.x_values.is_empty() {
                let lo = gx
                    .x_values
                    .iter()
                    .copied()
                    .fold(f64::INFINITY, f64::min);
                let hi = gx
                    .x_values
                    .iter()
                    .copied()
                    .fold(f64::NEG_INFINITY, f64::max);
                let (xm0, xm1) = gx.x_range.unwrap_or((lo, hi));
                (
                    gx.x_values.as_slice(),
                    xm0,
                    xm1,
                    gx.x_unit.clone(),
                    gx.y_unit.clone(),
                )
            } else {
                (&sim.x_values, x_min, x_max, gx.x_unit.clone(), gx.y_unit.clone())
            }
        } else {
            (&sim.x_values, x_min, x_max, x_unit.to_string(), None)
        };
        let g_x_span = (g_x_max - g_x_min).max(1e-15);

        let v_tr: Vec<(usize, String)> = g_traces
            .iter()
            .enumerate()
            .filter(|(_, t)| !is_current_trace(t) && !is_power_trace(t))
            .map(|(i, t)| (i, t.clone()))
            .collect();
        let i_tr: Vec<(usize, String)> = g_traces
            .iter()
            .enumerate()
            .filter(|(_, t)| is_current_trace(t) || is_power_trace(t))
            .map(|(i, t)| (i, t.clone()))
            .collect();

        let v_bounds = if !v_tr.is_empty() || i_tr.is_empty() {
            let y_override = if g_idx == 0 { view.y_range } else { None };
            Some(compute_y_bounds(sim, g_x_vals, &v_tr, g_x_min, g_x_max, y_override))
        } else {
            None
        };
        let i_bounds = if !i_tr.is_empty() {
            let y_override = if g_idx == 0 && v_tr.is_empty() {
                view.y_range_i
            } else {
                None
            };
            Some(compute_y_bounds(sim, g_x_vals, &i_tr, g_x_min, g_x_max, y_override))
        } else {
            None
        };

        let draw_x_labels = g_idx + 1 == num_graphs
            || view.graph_x_overrides.contains_key(&g_idx)
            || view.graph_x_overrides.contains_key(&(g_idx + 1));

        draw_dual_axis_graph(
            &painter,
            g_rect,
            sim,
            g_x_vals,
            &v_tr,
            &i_tr,
            g_x_min,
            g_x_span,
            &g_x_unit,
            g_y_unit_ov.as_deref(),
            v_bounds,
            i_bounds,
            draw_x_labels,
            num_graphs > 1 && view.focused_graph == g_idx,
        );
        graph_y_bounds.push((v_bounds, i_bounds));
        graph_x_meta.push((g_x_min, g_x_span, g_x_unit, g_y_unit_ov));
    }

    let (y_min, y_max) = graph_y_bounds[0]
        .0
        .or(graph_y_bounds[0].1)
        .unwrap_or((0.0, 5.0));
    let y_span = (y_max - y_min).max(1e-12);
    let active_sub_rect = graph_rects
        .get(view.focused_graph)
        .copied()
        .unwrap_or(plot_rect);
    let active_is_current = graph_y_bounds
        .get(view.focused_graph)
        .is_some_and(|(v, i)| v.is_none() && i.is_some());
    let plot_painter = painter.with_clip_rect(plot_rect);

    // Handle attaching Cursor 1 / Cursor 2 when clicking a trace name in the legend
    if let Some(tr_name) = attach_cursor_trace {
        let mid_x = x_min + 0.5 * x_span;
        let shift = ui.input(|i| i.modifiers.shift);
        if shift || (!view.cursors.is_empty() && view.cursors.len() < 2) {
            let x = view
                .cursors
                .get(1)
                .map(|(_, x)| *x)
                .unwrap_or(x_min + 0.65 * x_span);
            if view.cursors.len() >= 2 {
                view.cursors[1] = (tr_name, x);
            } else {
                view.cursors.push((tr_name, x));
            }
        } else if view.cursors.is_empty() {
            view.cursors.push((tr_name, mid_x));
        } else {
            let x = view.cursors[0].1;
            view.cursors[0] = (tr_name, x);
        }
    }

    // Pressing Esc clears cursors
    if response.hovered() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        view.cursors.clear();
        view.dragging_cursor = None;
    }

    // Cursor dragging (check proximity to any cursor before starting zoom box)
    let cursor_sxs: Vec<f32> = view
        .cursors
        .iter()
        .map(|(_, cx)| plot_rect.left() + (((cx - x_min) / x_span) as f32) * plot_rect.width())
        .collect();

    if response.drag_started_by(egui::PointerButton::Primary) && !ui.input(|i| i.modifiers.alt) {
        if let Some(pos) = response.interact_pointer_pos() {
            if plot_rect.contains(pos) {
                let nearest = cursor_sxs
                    .iter()
                    .enumerate()
                    .filter(|(_, sx)| (pos.x - **sx).abs() <= 10.0)
                    .min_by(|(_, a), (_, b)| {
                        (pos.x - **a)
                            .abs()
                            .partial_cmp(&(pos.x - **b).abs())
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(idx, _)| idx);
                if let Some(idx) = nearest {
                    view.dragging_cursor = Some(idx);
                } else {
                    view.drag_start = Some(pos);
                }
            }
        }
    }
    if let Some(c_idx) = view.dragging_cursor {
        if let Some(cur) = response.hover_pos() {
            let xf = ((cur.x - plot_rect.left()) / plot_rect.width()).clamp(0.0, 1.0) as f64;
            let new_x = x_min + xf * x_span;
            if let Some((_, ref mut x)) = view.cursors.get_mut(c_idx) {
                *x = new_x;
            }
        }
        if response.drag_stopped_by(egui::PointerButton::Primary) {
            view.dragging_cursor = None;
        }
    }

    // Interactive zoom box & crosshair
    if let Some(start) = view.drag_start {
        if let Some(cur) = response.hover_pos() {
            let box_rect = Rect::from_two_pos(start, cur).intersect(active_sub_rect);
            painter.rect_stroke(
                box_rect,
                0.0,
                Stroke::new(1.0_f32, Color32::WHITE),
            );
        }
        if response.drag_stopped_by(egui::PointerButton::Primary) {
            if let Some(end) = response.interact_pointer_pos() {
                let box_rect = Rect::from_two_pos(start, end).intersect(active_sub_rect);
                if box_rect.width() > 10.0 && box_rect.height() > 10.0 {
                    let nx0 = x_min
                        + ((box_rect.left() - active_sub_rect.left()) / active_sub_rect.width()) as f64
                            * x_span;
                    let nx1 = x_min
                        + ((box_rect.right() - active_sub_rect.left()) / active_sub_rect.width()) as f64
                            * x_span;
                    let ny1 = y_min
                        + ((active_sub_rect.bottom() - box_rect.top()) / active_sub_rect.height()) as f64
                            * y_span;
                    let ny0 = y_min
                        + ((active_sub_rect.bottom() - box_rect.bottom()) / active_sub_rect.height()) as f64
                            * y_span;
                    view.x_range = Some((nx0, nx1));
                    if active_is_current {
                        view.y_range_i = Some((ny0, ny1));
                    } else {
                        view.y_range = Some((ny0, ny1));
                    }
                }
            }
            view.drag_start = None;
        }
    }

    // Scroll-wheel zoom around cursor & pan with middle-mouse / Alt+drag
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll.abs() > 0.1 {
            let factor = if scroll > 0.0 { 0.85 } else { 1.0 / 0.85 };
            let xf = response
                .hover_pos()
                .map(|p| ((p.x - plot_rect.left()) / plot_rect.width()).clamp(0.0, 1.0) as f64)
                .unwrap_or(0.5);
            let anchor_x = x_min + xf * x_span;
            let nx0 = anchor_x - (anchor_x - x_min) * factor;
            let nx1 = anchor_x + (x_max - anchor_x) * factor;
            view.x_range = Some((nx0, nx1));
        }
    }
    if response.dragged_by(egui::PointerButton::Middle)
        || (response.dragged_by(egui::PointerButton::Primary) && ui.input(|i| i.modifiers.alt))
    {
        let delta = response.drag_delta();
        let dx = -(delta.x / active_sub_rect.width().max(1.0)) as f64 * x_span;
        let dy = (delta.y / active_sub_rect.height().max(1.0)) as f64 * y_span;
        view.x_range = Some((x_min + dx, x_max + dx));
        if active_is_current {
            view.y_range_i = Some((y_min + dy, y_max + dy));
        } else {
            view.y_range = Some((y_min + dy, y_max + dy));
        }
    }

    // Right-click on plot: record click position and open context menu (Add Fixed Cursor / New Graph / Reset Zoom)
    if response.secondary_clicked() {
        if remove_from_graph.is_some() {
            view.context_menu_pos = None;
        } else if let Some(pos) = response.interact_pointer_pos() {
            let mut matched = false;
            for (g_idx, gr) in graph_rects.iter().enumerate() {
                if gr.contains(pos) {
                    let g_traces = if g_idx == 0 {
                        &view.active_traces
                    } else {
                        &view.extra_graphs[g_idx - 1]
                    };
                    if !g_traces.is_empty() {
                        let xf = ((pos.x - gr.left()) / gr.width()).clamp(0.0, 1.0) as f64;
                        let click_x = x_min + xf * x_span;
                        let (vb, ib) = graph_y_bounds[g_idx];
                        let closest_trace = g_traces
                            .iter()
                            .filter_map(|t| {
                                let yv = sample_trace_at(sim, t, click_x)?;
                                let (gy0, gy1) = if is_current_trace(t) {
                                    ib.unwrap_or((y_min, y_max))
                                } else {
                                    vb.unwrap_or((y_min, y_max))
                                };
                                let gys = (gy1 - gy0).max(1e-15);
                                let sy = gr.bottom() - (((yv - gy0) / gys) as f32) * gr.height();
                                Some((t.clone(), (sy - pos.y).abs()))
                            })
                            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                            .map(|(t, _)| t)
                            .unwrap_or_else(|| g_traces[0].clone());
                        view.context_menu_pos = Some((closest_trace, click_x));
                        matched = true;
                        break;
                    }
                }
            }
            if !matched {
                view.context_menu_pos = None;
                view.reset_zoom();
            }
        }
    }
    response.context_menu(|ui| {
        let Some((ref tr_name, click_x)) = view.context_menu_pos.clone() else {
            ui.close_menu();
            return;
        };
        let next_num = view.cursors.len() + 1;
        if ui
            .button(format!(
                "Add Fixed Cursor C{next_num} on {tr_name} ({})",
                format_si_precise(click_x, x_unit)
            ))
            .clicked()
        {
            view.cursors.push((tr_name.clone(), click_x));
            view.context_menu_pos = None;
            ui.close_menu();
        }
        if !view.cursors.is_empty() {
            if ui.button("Move C1 here").clicked() {
                view.cursors[0] = (tr_name.clone(), click_x);
                view.context_menu_pos = None;
                ui.close_menu();
            }
        }
        if view.cursors.len() >= 2 {
            if ui.button("Move C2 here").clicked() {
                view.cursors[1] = (tr_name.clone(), click_x);
                view.context_menu_pos = None;
                ui.close_menu();
            }
        }
        if !view.cursors.is_empty() {
            ui.separator();
            if ui.button("Clear All Cursors (Esc)").clicked() {
                view.cursors.clear();
                view.dragging_cursor = None;
                view.context_menu_pos = None;
                ui.close_menu();
            }
        }
        ui.separator();
        if ui.button("New Graph").clicked() {
            view.add_graph();
            view.context_menu_pos = None;
            ui.close_menu();
        }
        if ui.button("Reset Zoom").clicked() {
            view.reset_zoom();
            view.context_menu_pos = None;
            ui.close_menu();
        }
    });

    // Draw attached / fixed Waveform Cursors on the graph containing their trace
    let mut cursor_vals: Vec<Option<f64>> = Vec::with_capacity(view.cursors.len());
    for (idx, cursor) in view.cursors.iter().enumerate() {
        let color = CURSOR_COLORS[idx % CURSOR_COLORS.len()];
        let badge = format!("{}", idx + 1);
        let is_cur = is_current_trace(&cursor.0) || is_power_trace(&cursor.0);
        let g_idx = (0..num_graphs)
            .find(|&gi| {
                let gt = if gi == 0 {
                    &view.active_traces
                } else {
                    &view.extra_graphs[gi - 1]
                };
                gt.contains(&cursor.0)
            })
            .unwrap_or(0);
        let c_rect = graph_rects[g_idx];
        let (vb, ib) = graph_y_bounds[g_idx];
        let (cy_min, cy_max) = if is_cur {
            ib.unwrap_or((y_min, y_max))
        } else {
            vb.unwrap_or((y_min, y_max))
        };
        let cy_span = (cy_max - cy_min).max(1e-12);
        let val = draw_cursor(
            &plot_painter,
            c_rect,
            plot_rect,
            sim,
            Some(cursor),
            &badge,
            color,
            x_min,
            x_span,
            cy_min,
            cy_span,
        );
        cursor_vals.push(val);
    }

    // Draw temporal distance indicator between Cursor 1 and Cursor 2 on the plot
    if view.cursors.len() >= 2 {
        let x1 = view.cursors[0].1;
        let x2 = view.cursors[1].1;
        let dt = (x2 - x1).abs();
        let sx1 = plot_rect.left() + (((x1 - x_min) / x_span) as f32) * plot_rect.width();
        let sx2 = plot_rect.left() + (((x2 - x_min) / x_span) as f32) * plot_rect.width();
        let (left_sx, right_sx) = if sx1 <= sx2 { (sx1, sx2) } else { (sx2, sx1) };
        if right_sx - left_sx >= 4.0 {
            let bar_y = plot_rect.top() + 20.0;
            let bar_color = Color32::from_rgb(240, 240, 240);
            plot_painter.line_segment(
                [Pos2::new(left_sx, bar_y), Pos2::new(right_sx, bar_y)],
                Stroke::new(1.2, bar_color),
            );
            plot_painter.line_segment(
                [Pos2::new(left_sx, bar_y - 4.0), Pos2::new(left_sx, bar_y + 4.0)],
                Stroke::new(1.2, bar_color),
            );
            plot_painter.line_segment(
                [Pos2::new(right_sx, bar_y - 4.0), Pos2::new(right_sx, bar_y + 4.0)],
                Stroke::new(1.2, bar_color),
            );
            let dist_label = if x_unit == "s" {
                let freq = if dt > 1e-15 {
                    format!(" ({})", format_si_precise(1.0 / dt, "Hz"))
                } else {
                    String::new()
                };
                format!("Δt = {}{freq}", format_si_precise(dt, "s"))
            } else {
                format!("Δf = {}", format_si_precise(dt, x_unit))
            };
            plot_painter.text(
                Pos2::new(0.5 * (left_sx + right_sx), bar_y - 3.0),
                egui::Align2::CENTER_BOTTOM,
                dist_label,
                FontId::monospace(11.0),
                bar_color,
            );
        }
    }

    // Cursor Readout Panel (bottom-right of plot)
    if !view.cursors.is_empty() {
        let mut lines: Vec<(String, Color32)> = Vec::new();
        for (idx, ((tr, cx), cy_opt)) in view.cursors.iter().zip(cursor_vals.iter()).enumerate() {
            if let Some(cy) = cy_opt {
                let color = CURSOR_COLORS[idx % CURSOR_COLORS.len()];
                let unit = trace_unit(tr);
                lines.push((
                    format!(
                        "C{} [{tr}]: x = {}, y = {}",
                        idx + 1,
                        format_si_precise(*cx, x_unit),
                        format_si_precise(*cy, unit)
                    ),
                    color,
                ));
            }
        }
        if view.cursors.len() >= 2 {
            if let (Some(y1), Some(y2)) = (cursor_vals[0], cursor_vals[1]) {
                let x1 = view.cursors[0].1;
                let x2 = view.cursors[1].1;
                let u1 = trace_unit(&view.cursors[0].0);
                let u2 = trace_unit(&view.cursors[1].0);
                let dt = (x2 - x1).abs();
                let dy = y2 - y1;
                let dist_label = if x_unit == "s" { "Δt" } else { "Δf" };
                let freq_str = if x_unit == "s" && dt > 1e-15 {
                    format!(" ({})", format_si_precise(1.0 / dt, "Hz"))
                } else {
                    String::new()
                };
                let dy_str = if u1 == u2 {
                    format!(", Δ{u1} = {}", format_si_precise(dy, u1))
                } else {
                    String::new()
                };
                lines.push((
                    format!(
                        "C1↔C2: {dist_label} = {}{freq_str}{dy_str}",
                        format_si_precise(dt, x_unit),
                    ),
                    Color32::WHITE,
                ));
            }
        }

        if !lines.is_empty() {
            let box_w = 360.0_f32;
            let box_h = 12.0 + lines.len() as f32 * 16.0;
            let box_rect = Rect::from_min_size(
                Pos2::new(
                    (plot_rect.right() - box_w - 8.0).max(plot_rect.left() + 4.0),
                    (plot_rect.bottom() - box_h - 8.0).max(plot_rect.top() + 4.0),
                ),
                Vec2::new(box_w.min(plot_rect.width() - 8.0), box_h),
            );
            painter.rect_filled(
                box_rect,
                3.0,
                Color32::from_rgba_unmultiplied(15, 15, 20, 220),
            );
            painter.rect_stroke(
                box_rect,
                3.0,
                Stroke::new(1.0_f32, Color32::from_rgb(120, 120, 120)),
            );
            for (i, (txt, col)) in lines.iter().enumerate() {
                painter.text(
                    Pos2::new(box_rect.left() + 8.0, box_rect.top() + 7.0 + i as f32 * 16.0),
                    egui::Align2::LEFT_TOP,
                    txt,
                    FontId::monospace(11.0),
                    *col,
                );
            }
        }
    }

    // Hover crosshair & instantaneous trace value readout (per-graph X-axis & units aware)
    if let Some(pos) = response.hover_pos() {
        if plot_rect.contains(pos) && view.drag_start.is_none() && view.dragging_cursor.is_none() {
            let hovered_g = graph_rects
                .iter()
                .position(|gr| gr.contains(pos))
                .unwrap_or(0);
            let h_rect = graph_rects[hovered_g];
            let (hx_min, hx_span, ref hx_unit, ref hy_unit_ov) = graph_x_meta[hovered_g];
            let hx_vals: &[f64] = view
                .graph_x_overrides
                .get(&hovered_g)
                .filter(|gx| !gx.x_values.is_empty())
                .map(|gx| gx.x_values.as_slice())
                .unwrap_or(&sim.x_values);
            let h_traces = if hovered_g == 0 {
                &view.active_traces
            } else {
                &view.extra_graphs[hovered_g - 1]
            };

            let xf = ((pos.x - h_rect.left()) / h_rect.width()).clamp(0.0, 1.0) as f64;
            let xv = hx_min + xf * hx_span;

            let cross_color = Color32::from_rgba_unmultiplied(180, 180, 180, 90);
            painter.line_segment(
                [
                    Pos2::new(pos.x, h_rect.top()),
                    Pos2::new(pos.x, h_rect.bottom()),
                ],
                Stroke::new(1.0_f32, cross_color),
            );
            let mut readout = format!("x = {}", format_si_precise(xv, hx_unit));
            for tr_name in h_traces {
                if let Some(yv) = sample_trace_at_x(sim, hx_vals, tr_name, xv) {
                    let u = hy_unit_ov.as_deref().unwrap_or_else(|| trace_unit(tr_name));
                    readout.push_str(&format!("  |  {tr_name} = {}", format_si_precise(yv, u)));
                }
            }
            painter.text(
                Pos2::new(h_rect.right() - 6.0, h_rect.top() + 6.0),
                egui::Align2::RIGHT_TOP,
                readout,
                FontId::monospace(11.0),
                Color32::from_rgb(220, 220, 220),
            );
        }
    }

    interacted
}

fn sample_trace_at(sim: &SimulationResult, trace_name: &str, x: f64) -> Option<f64> {
    sample_trace_at_x(sim, &sim.x_values, trace_name, x)
}

fn sample_trace_at_x(
    sim: &SimulationResult,
    x_values: &[f64],
    trace_name: &str,
    x: f64,
) -> Option<f64> {
    let tr = sim.traces.get(trace_name)?;
    let n = x_values.len().min(tr.real.len());
    if n == 0 {
        return None;
    }
    let idx = x_values[..n].partition_point(|&xv| xv < x);
    if idx == 0 {
        return Some(tr.real[0]);
    }
    if idx >= n {
        return Some(tr.real[n - 1]);
    }
    let x0 = x_values[idx - 1];
    let x1 = x_values[idx];
    let y0 = tr.real[idx - 1];
    let y1 = tr.real[idx];
    let dx = x1 - x0;
    if dx.abs() < 1e-18 {
        Some(y1)
    } else {
        let t = (x - x0) / dx;
        Some(y0 + t * (y1 - y0))
    }
}

fn compute_y_bounds(
    sim: &SimulationResult,
    x_values: &[f64],
    traces: &[(usize, String)],
    x_min: f64,
    x_max: f64,
    y_range_override: Option<(f64, f64)>,
) -> (f64, f64) {
    let (mut y_min, mut y_max) = if let Some(yr) = y_range_override {
        yr
    } else {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for (_, name) in traces {
            if let Some(tr) = sim.traces.get(name) {
                for (&xv, &yv) in x_values.iter().zip(tr.real.iter()) {
                    if xv >= x_min && xv <= x_max && yv.is_finite() {
                        lo = lo.min(yv);
                        hi = hi.max(yv);
                    }
                }
            }
        }
        if !lo.is_finite() || !hi.is_finite() {
            (0.0, 5.0)
        } else if (hi - lo).abs() < 1e-12 {
            let pad = (lo.abs() * 0.1).max(1e-3);
            (lo - pad, hi + pad)
        } else {
            let pad = (hi - lo) * 0.08;
            (lo - pad, hi + pad)
        }
    };
    if (y_max - y_min).abs() < 1e-15 {
        y_min -= 0.5;
        y_max += 0.5;
    }
    (y_min, y_max)
}

#[allow(clippy::too_many_arguments)]
fn draw_dual_axis_graph(
    painter: &egui::Painter,
    sub_rect: Rect,
    sim: &SimulationResult,
    x_values: &[f64],
    v_traces: &[(usize, String)],
    i_traces: &[(usize, String)],
    x_min: f64,
    x_span: f64,
    x_unit: &str,
    y_unit_override: Option<&str>,
    v_bounds: Option<(f64, f64)>,
    i_bounds: Option<(f64, f64)>,
    draw_x_labels: bool,
    is_focused: bool,
) {
    let border_color = if is_focused {
        Color32::from_rgb(120, 180, 255)
    } else {
        Color32::from_rgb(130, 130, 130)
    };
    painter.rect_stroke(sub_rect, 0.0, Stroke::new(1.0_f32, border_color));

    let grid_color = Color32::from_rgb(48, 48, 48);
    let label_color = Color32::from_rgb(200, 200, 200);

    let x_ticks = 10;
    for i in 0..=x_ticks {
        let frac = i as f64 / x_ticks as f64;
        let xv = x_min + frac * x_span;
        let sx = sub_rect.left() + (frac as f32) * sub_rect.width();
        draw_dashed_line(
            painter,
            Pos2::new(sx, sub_rect.top()),
            Pos2::new(sx, sub_rect.bottom()),
            grid_color,
        );
        if draw_x_labels {
            painter.text(
                Pos2::new(sx, sub_rect.bottom() + 4.0),
                egui::Align2::CENTER_TOP,
                format_si(xv, x_unit),
                FontId::proportional(11.0),
                label_color,
            );
        }
    }

    // Left Y-axis (V if v_bounds is Some, else A or W)
    let sec_unit = if i_traces.iter().all(|(_, t)| is_power_trace(t)) && !i_traces.is_empty() {
        "W"
    } else {
        "A"
    };
    let (left_y_min, left_y_max, left_unit) = if let Some((vy0, vy1)) = v_bounds {
        (vy0, vy1, y_unit_override.unwrap_or("V"))
    } else if let Some((iy0, iy1)) = i_bounds {
        (iy0, iy1, y_unit_override.unwrap_or(sec_unit))
    } else {
        (0.0, 5.0, y_unit_override.unwrap_or("V"))
    };
    let left_y_span = (left_y_max - left_y_min).max(1e-15);
    let y_ticks = 8;
    for i in 0..=y_ticks {
        let frac = i as f64 / y_ticks as f64;
        let yv = left_y_min + frac * left_y_span;
        let sy = sub_rect.bottom() - (frac as f32) * sub_rect.height();
        draw_dashed_line(
            painter,
            Pos2::new(sub_rect.left(), sy),
            Pos2::new(sub_rect.right(), sy),
            grid_color,
        );
        painter.text(
            Pos2::new(sub_rect.left() - 4.0, sy),
            egui::Align2::RIGHT_CENTER,
            format_si(yv, left_unit),
            FontId::proportional(11.0),
            label_color,
        );
    }

    // Right Y-axis for Current (A) or Power (W) when sharing the same graph with V
    if v_bounds.is_some() {
        if let Some((iy0, iy1)) = i_bounds {
            let right_y_span = (iy1 - iy0).max(1e-15);
            let i_label_color = if sec_unit == "W" {
                Color32::from_rgb(255, 175, 80)
            } else {
                Color32::from_rgb(130, 195, 255)
            };
            for i in 0..=y_ticks {
                let frac = i as f64 / y_ticks as f64;
                let yv = iy0 + frac * right_y_span;
                let sy = sub_rect.bottom() - (frac as f32) * sub_rect.height();
                painter.text(
                    Pos2::new(sub_rect.right() + 4.0, sy),
                    egui::Align2::LEFT_CENTER,
                    format_si(yv, sec_unit),
                    FontId::proportional(11.0),
                    i_label_color,
                );
            }
        }
    }

    let sub_painter = painter.with_clip_rect(sub_rect);
    if let Some((vy0, vy1)) = v_bounds {
        draw_trace_group(&sub_painter, sub_rect, sim, x_values, v_traces, x_min, x_span, vy0, vy1);
    }
    if let Some((iy0, iy1)) = i_bounds {
        draw_trace_group(&sub_painter, sub_rect, sim, x_values, i_traces, x_min, x_span, iy0, iy1);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_trace_group(
    sub_painter: &egui::Painter,
    sub_rect: Rect,
    sim: &SimulationResult,
    x_values: &[f64],
    traces: &[(usize, String)],
    x_min: f64,
    x_span: f64,
    y_min: f64,
    y_max: f64,
) {
    let y_span = (y_max - y_min).max(1e-15);
    for (color_idx, trace_name) in traces {
        let Some(tr) = sim.traces.get(trace_name) else {
            continue;
        };
        let color = TRACE_COLORS[color_idx % TRACE_COLORS.len()];
        let stroke = Stroke::new(1.5_f32, color);

        let n = x_values.len().min(tr.real.len());
        if n < 2 {
            continue;
        }
        let x_max = x_min + x_span;
        let start_idx = x_values[..n]
            .partition_point(|&x| x < x_min)
            .saturating_sub(1);
        let end_idx = (x_values[..n].partition_point(|&x| x <= x_max) + 1).min(n);
        let vis_count = end_idx.saturating_sub(start_idx);
        if vis_count < 2 {
            continue;
        }

        let max_pts = (sub_rect.width() as usize * 2).max(1000);
        if vis_count <= max_pts {
            let mut prev: Option<Pos2> = None;
            for i in start_idx..end_idx {
                let xv = x_values[i];
                let yv = tr.real[i];
                if !xv.is_finite() || !yv.is_finite() {
                    prev = None;
                    continue;
                }
                let sx = sub_rect.left() + (((xv - x_min) / x_span) as f32) * sub_rect.width();
                let sy = sub_rect.bottom() - (((yv - y_min) / y_span) as f32) * sub_rect.height();
                let cur = Pos2::new(sx, sy);
                if let Some(p) = prev {
                    sub_painter.line_segment([p, cur], stroke);
                }
                prev = Some(cur);
            }
        } else {
            let cols = (sub_rect.width() as usize).max(1);
            let mut prev_x: Option<f32> = None;
            let mut prev_hi = 0.0_f32;
            let mut prev_lo = 0.0_f32;
            for col in 0..cols {
                let c_lo = start_idx + (col * vis_count) / cols;
                let c_hi = (start_idx + ((col + 1) * vis_count) / cols).min(end_idx);
                if c_lo >= c_hi {
                    continue;
                }
                let mut y_lo = f64::INFINITY;
                let mut y_hi = f64::NEG_INFINITY;
                for i in c_lo..c_hi {
                    let yv = tr.real[i];
                    if yv.is_finite() {
                        y_lo = y_lo.min(yv);
                        y_hi = y_hi.max(yv);
                    }
                }
                if !y_lo.is_finite() {
                    prev_x = None;
                    continue;
                }
                let sx = sub_rect.left() + (col as f32 + 0.5);
                let sy_hi =
                    sub_rect.bottom() - (((y_hi - y_min) / y_span) as f32) * sub_rect.height();
                let sy_lo =
                    sub_rect.bottom() - (((y_lo - y_min) / y_span) as f32) * sub_rect.height();
                sub_painter.line_segment([Pos2::new(sx, sy_hi), Pos2::new(sx, sy_lo)], stroke);
                if let Some(px) = prev_x {
                    sub_painter.line_segment(
                        [
                            Pos2::new(px, (prev_hi + prev_lo) * 0.5),
                            Pos2::new(sx, (sy_hi + sy_lo) * 0.5),
                        ],
                        stroke,
                    );
                }
                prev_x = Some(sx);
                prev_hi = sy_hi;
                prev_lo = sy_lo;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_cursor(
    painter: &egui::Painter,
    sub_rect: Rect,
    full_rect: Rect,
    sim: &SimulationResult,
    cursor: Option<&(String, f64)>,
    badge: &str,
    color: Color32,
    x_min: f64,
    x_span: f64,
    y_min: f64,
    y_span: f64,
) -> Option<f64> {
    let (trace_name, cx) = cursor?;
    let cy = sample_trace_at(sim, trace_name, *cx)?;
    let sx = full_rect.left() + (((*cx - x_min) / x_span) as f32) * full_rect.width();
    let sy = sub_rect.bottom() - (((cy - y_min) / y_span) as f32) * sub_rect.height();

    draw_dashed_line(
        painter,
        Pos2::new(sx, full_rect.top()),
        Pos2::new(sx, full_rect.bottom()),
        color,
    );
    draw_dashed_line(
        painter,
        Pos2::new(sub_rect.left(), sy),
        Pos2::new(sub_rect.right(), sy),
        color,
    );
    painter.circle_stroke(Pos2::new(sx, sy), 4.5, Stroke::new(1.5_f32, color));
    painter.text(
        Pos2::new(sx + 4.0, full_rect.top() + 4.0),
        egui::Align2::LEFT_TOP,
        badge,
        FontId::monospace(11.0),
        color,
    );
    Some(cy)
}

fn draw_dashed_line(painter: &egui::Painter, a: Pos2, b: Pos2, color: Color32) {
    let delta = b - a;
    let len = delta.length();
    if len < 1.0 {
        return;
    }
    let dir = delta / len;
    let dash = 3.0;
    let gap = 4.0;
    let mut t = 0.0;
    let stroke = Stroke::new(1.0, color);
    while t < len {
        let p0 = a + dir * t;
        let p1 = a + dir * (t + dash).min(len);
        painter.line_segment([p0, p1], stroke);
        t += dash + gap;
    }
}

pub fn format_si_precise(val: f64, unit: &str) -> String {
    let abs = val.abs();
    if abs == 0.0 {
        return format!("0{unit}");
    }
    let (scale, prefix) = si_scale_prefix(abs);
    let scaled = val * scale;
    if scaled.abs() >= 100.0 {
        format!("{scaled:.1}{prefix}{unit}")
    } else if scaled.abs() >= 10.0 {
        format!("{scaled:.2}{prefix}{unit}")
    } else {
        format!("{scaled:.3}{prefix}{unit}")
    }
}

fn si_scale_prefix(abs: f64) -> (f64, &'static str) {
    if abs >= 1e9 {
        (1e-9, "G")
    } else if abs >= 1e6 {
        (1e-6, "M")
    } else if abs >= 1e3 {
        (1e-3, "k")
    } else if abs >= 1.0 {
        (1.0, "")
    } else if abs >= 1e-3 {
        (1e3, "m")
    } else if abs >= 1e-6 {
        (1e6, "µ")
    } else if abs >= 1e-9 {
        (1e9, "n")
    } else {
        (1e12, "p")
    }
}

pub fn format_si(val: f64, unit: &str) -> String {
    let abs = val.abs();
    if abs == 0.0 {
        return format!("0{unit}");
    }
    let (scale, prefix) = si_scale_prefix(abs);
    let scaled = val * scale;
    if scaled.abs() >= 100.0 || (scaled - scaled.round()).abs() < 1e-3 {
        format!("{scaled:.0}{prefix}{unit}")
    } else {
        format!("{scaled:.1}{prefix}{unit}")
    }
}
