use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::hdl::HdlBlock;
use crate::settings::ModelIndex;
use crate::symbol_editor::CustomSymbol;

pub const GRID_SIZE: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ComponentKind {
    VoltageSource,
    CurrentSource,
    Resistor,
    Capacitor,
    Inductor,
    Diode,
    Nmos,
    Pmos,
    Npn,
    Pnp,
    OpAmp,
    Ground,
    NetLabel,
    Custom,
}

impl ComponentKind {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::VoltageSource => "Voltage Source",
            Self::CurrentSource => "Current Source",
            Self::Resistor => "Resistor",
            Self::Capacitor => "Capacitor",
            Self::Inductor => "Inductor",
            Self::Diode => "Diode",
            Self::Nmos => "NMOS MOSFET",
            Self::Pmos => "PMOS MOSFET",
            Self::Npn => "NPN BJT",
            Self::Pnp => "PNP BJT",
            Self::OpAmp => "Op-Amp (5-pin)",
            Self::Ground => "Ground (0)",
            Self::NetLabel => "Net Label",
            Self::Custom => "Custom Symbol",
        }
    }

    pub fn prefix(self) -> &'static str {
        match self {
            Self::VoltageSource => "V",
            Self::CurrentSource => "I",
            Self::Resistor => "R",
            Self::Capacitor => "C",
            Self::Inductor => "L",
            Self::Diode => "D",
            Self::Nmos | Self::Pmos => "M",
            Self::Npn | Self::Pnp => "Q",
            Self::OpAmp => "U",
            Self::Ground => "GND",
            Self::NetLabel => "NET",
            Self::Custom => "X",
        }
    }

    pub fn default_value(self) -> &'static str {
        match self {
            Self::VoltageSource => "5",
            Self::CurrentSource => "1m",
            Self::Resistor => "10k",
            Self::Capacitor => "1u",
            Self::Inductor => "1m",
            Self::Diode => "D",
            Self::Nmos => "NMOS",
            Self::Pmos => "PMOS",
            Self::Npn => "NPN",
            Self::Pnp => "PNP",
            Self::OpAmp => "OPAMP",
            Self::Ground => "0",
            Self::NetLabel => "net1",
            Self::Custom => "CUSTOM",
        }
    }

    pub fn has_refdes(self) -> bool {
        !matches!(self, Self::Ground | Self::NetLabel)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    pub id: u64,
    pub kind: ComponentKind,
    pub refdes: String,
    pub value: String,
    /// Grid units (1 unit = 16 px at zoom 1.0)
    pub x: i32,
    pub y: i32,
    /// Degrees: 0, 90, 180, 270
    pub rotation: i32,
    pub mirror: bool,
    #[serde(default)]
    pub params: HashMap<String, String>,
    #[serde(default)]
    pub custom_symbol: Option<CustomSymbol>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hdl: Option<HdlBlock>,
    #[serde(default)]
    pub label_offset: Option<(f32, f32)>,
    #[serde(default)]
    pub value_offset: Option<(f32, f32)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wire {
    pub id: u64,
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Directive {
    pub id: u64,
    pub x: i32,
    pub y: i32,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schematic {
    pub name: String,
    pub components: Vec<Component>,
    pub wires: Vec<Wire>,
    pub directives: Vec<Directive>,
    pub next_id: u64,
}

#[derive(Debug, Clone)]
pub struct PinDef {
    pub name: &'static str,
    pub dx: i32,
    pub dy: i32,
}

pub fn local_pins(kind: ComponentKind) -> Vec<PinDef> {
    match kind {
        ComponentKind::VoltageSource
        | ComponentKind::CurrentSource
        | ComponentKind::Resistor
        | ComponentKind::Capacitor
        | ComponentKind::Inductor
        | ComponentKind::Diode => vec![
            PinDef { name: "1", dx: 0, dy: -2 },
            PinDef { name: "2", dx: 0, dy: 2 },
        ],
        ComponentKind::Nmos | ComponentKind::Pmos => vec![
            PinDef { name: "D", dx: 2, dy: -2 },
            PinDef { name: "G", dx: -2, dy: 1 },
            PinDef { name: "S", dx: 2, dy: 2 },
        ],
        ComponentKind::Npn | ComponentKind::Pnp => vec![
            PinDef { name: "C", dx: 2, dy: -2 },
            PinDef { name: "B", dx: -2, dy: 0 },
            PinDef { name: "E", dx: 2, dy: 2 },
        ],
        ComponentKind::OpAmp => vec![
            PinDef { name: "IN+", dx: -2, dy: -1 },
            PinDef { name: "IN-", dx: -2, dy: 1 },
            PinDef { name: "V+", dx: 0, dy: -2 },
            PinDef { name: "V-", dx: 0, dy: 2 },
            PinDef { name: "OUT", dx: 2, dy: 0 },
        ],
        ComponentKind::Ground | ComponentKind::NetLabel | ComponentKind::Custom => {
            vec![PinDef { name: "1", dx: 0, dy: 0 }]
        }
    }
}

pub fn transform_offset(dx: i32, dy: i32, rotation: i32, mirror: bool) -> (i32, i32) {
    let mx = if mirror { -dx } else { dx };
    let my = dy;
    let r = ((rotation % 360) + 360) % 360;
    match r {
        90 => (-my, mx),
        180 => (-mx, -my),
        270 => (my, -mx),
        _ => (mx, my),
    }
}

pub fn component_pins_world(comp: &Component) -> Vec<(String, i32, i32)> {
    if comp.kind == ComponentKind::Custom {
        if let Some(sym) = &comp.custom_symbol {
            let mut pins = sym.pins.clone();
            pins.sort_by_key(|p| p.order);
            return pins
                .into_iter()
                .map(|p| {
                    let (ox, oy) = transform_offset(p.x, p.y, comp.rotation, comp.mirror);
                    (p.name, comp.x + ox, comp.y + oy)
                })
                .collect();
        }
    }
    local_pins(comp.kind)
        .into_iter()
        .map(|p| {
            let (ox, oy) = transform_offset(p.dx, p.dy, comp.rotation, comp.mirror);
            (p.name.to_string(), comp.x + ox, comp.y + oy)
        })
        .collect()
}

pub struct NetlistResult {
    pub lines: Vec<String>,
    pub point_to_net: HashMap<(i32, i32), String>,
    pub wire_to_net: HashMap<u64, String>,
    pub comp_spice_names: HashMap<u64, String>,
    pub warnings: Vec<String>,
}

struct UnionFind {
    parent: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
        }
    }

    fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    fn union(&mut self, a: usize, b: usize) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra != rb {
            self.parent[rb] = ra;
        }
    }
}

fn point_on_segment(px: i32, py: i32, w: &Wire) -> bool {
    if w.x1 == w.x2 && px == w.x1 {
        let (lo, hi) = (w.y1.min(w.y2), w.y1.max(w.y2));
        py >= lo && py <= hi
    } else if w.y1 == w.y2 && py == w.y1 {
        let (lo, hi) = (w.x1.min(w.x2), w.x1.max(w.x2));
        px >= lo && px <= hi
    } else {
        false
    }
}

impl Schematic {
    pub fn new_empty(name: &str) -> Self {
        Self {
            name: name.to_string(),
            components: Vec::new(),
            wires: Vec::new(),
            directives: Vec::new(),
            next_id: 1,
        }
    }

    pub fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn next_refdes(&self, kind: ComponentKind) -> String {
        if !kind.has_refdes() {
            return String::new();
        }
        let prefix = kind.prefix();
        let existing: HashSet<&str> = self
            .components
            .iter()
            .map(|c| c.refdes.as_str())
            .collect();
        for i in 1..1000 {
            let candidate = format!("{prefix}{i}");
            if !existing.contains(candidate.as_str()) {
                return candidate;
            }
        }
        format!("{prefix}{}", self.next_id)
    }

    pub fn add_component(
        &mut self,
        kind: ComponentKind,
        refdes: String,
        value: String,
        x: i32,
        y: i32,
    ) -> u64 {
        let id = self.alloc_id();
        self.components.push(Component {
            id,
            kind,
            refdes,
            value,
            x,
            y,
            rotation: 0,
            mirror: false,
            params: HashMap::new(),
            custom_symbol: None,
            hdl: None,
            label_offset: None,
            value_offset: None,
        });
        id
    }

    pub fn add_wire(&mut self, x1: i32, y1: i32, x2: i32, y2: i32) -> u64 {
        let id = self.alloc_id();
        self.wires.push(Wire { id, x1, y1, x2, y2 });
        id
    }

    pub fn add_directive(&mut self, x: i32, y: i32, text: String) -> u64 {
        let id = self.alloc_id();
        self.directives.push(Directive { id, x, y, text });
        id
    }

    /// Cuts any wire that passes between the two pads of any 2-pin component on the schematic.
    pub fn split_wires_for_all_components(&mut self) {
        let comp_ids: Vec<u64> = self.components.iter().map(|c| c.id).collect();
        for cid in comp_ids {
            self.split_wires_for_component(cid);
        }
    }

    /// Cuts any wire that passes between the two pads of a 2-pin component so the component
    /// is inserted in series rather than shorted out.
    pub fn split_wires_for_component(&mut self, comp_id: u64) {
        let Some(comp) = self.components.iter().find(|c| c.id == comp_id).cloned() else {
            return;
        };
        let pins = component_pins_world(&comp);
        if pins.len() != 2 {
            return;
        }
        let (_, x1, y1) = pins[0];
        let (_, x2, y2) = pins[1];

        let mut new_wires = Vec::with_capacity(self.wires.len());
        let old_wires = std::mem::take(&mut self.wires);

        if x1 == x2 {
            let px = x1;
            let lo_y = y1.min(y2);
            let hi_y = y1.max(y2);
            for w in old_wires {
                if w.x1 == w.x2 && w.x1 == px {
                    let w_lo = w.y1.min(w.y2);
                    let w_hi = w.y1.max(w.y2);
                    if w_lo < hi_y && w_hi > lo_y {
                        if w_lo < lo_y {
                            let id = self.alloc_id();
                            new_wires.push(Wire {
                                id,
                                x1: px,
                                y1: w_lo,
                                x2: px,
                                y2: lo_y,
                            });
                        }
                        if w_hi > hi_y {
                            let id = self.alloc_id();
                            new_wires.push(Wire {
                                id,
                                x1: px,
                                y1: hi_y,
                                x2: px,
                                y2: w_hi,
                            });
                        }
                        continue;
                    }
                }
                new_wires.push(w);
            }
        } else if y1 == y2 {
            let py = y1;
            let lo_x = x1.min(x2);
            let hi_x = x1.max(x2);
            for w in old_wires {
                if w.y1 == w.y2 && w.y1 == py {
                    let w_lo = w.x1.min(w.x2);
                    let w_hi = w.x1.max(w.x2);
                    if w_lo < hi_x && w_hi > lo_x {
                        if w_lo < lo_x {
                            let id = self.alloc_id();
                            new_wires.push(Wire {
                                id,
                                x1: w_lo,
                                y1: py,
                                x2: lo_x,
                                y2: py,
                            });
                        }
                        if w_hi > hi_x {
                            let id = self.alloc_id();
                            new_wires.push(Wire {
                                id,
                                x1: hi_x,
                                y1: py,
                                x2: w_hi,
                                y2: py,
                            });
                        }
                        continue;
                    }
                }
                new_wires.push(w);
            }
        } else {
            new_wires = old_wires;
        }

        self.wires = new_wires;
    }

    /// Builds the default RC low-pass filter schematic with a sine wave supply.
    pub fn default_ltspice_demo() -> Self {
        let mut s = Self::new_empty("Draft1.asc");

        // V1 = SINE(0 5 1k) at (10, 12) -> pins (10, 10) and (10, 14)
        s.add_component(
            ComponentKind::VoltageSource,
            "V1".into(),
            "SINE(0 5 1k)".into(),
            10,
            12,
        );
        s.add_component(ComponentKind::Ground, String::new(), "0".into(), 10, 15);
        s.add_wire(10, 14, 10, 15);
        s.add_component(ComponentKind::NetLabel, String::new(), "in".into(), 10, 9);
        s.add_wire(10, 10, 10, 9);

        // R1 = 1k horizontal (rotation 90) at (14, 10) -> pins (16, 10) and (12, 10)
        s.add_component(ComponentKind::Resistor, "R1".into(), "1k".into(), 14, 10);
        s.components.last_mut().unwrap().rotation = 90;
        s.add_wire(10, 10, 12, 10);

        // C1 = 100n at (18, 12) -> pins (18, 10) and (18, 14)
        s.add_component(ComponentKind::Capacitor, "C1".into(), "100n".into(), 18, 12);
        s.add_wire(16, 10, 18, 10);
        s.add_component(ComponentKind::NetLabel, String::new(), "out".into(), 18, 9);
        s.add_wire(18, 10, 18, 9);
        s.add_component(ComponentKind::Ground, String::new(), "0".into(), 18, 15);
        s.add_wire(18, 14, 18, 15);

        // Directive .tran 5m (5 cycles of 1kHz sine)
        s.add_directive(9, 18, ".tran 5m".into());

        s
    }

    pub fn to_spice_netlist(&self) -> NetlistResult {
        self.to_spice_netlist_with_models(None)
    }

    pub fn to_spice_netlist_with_models(&self, model_index: Option<&ModelIndex>) -> NetlistResult {
        let mut points: Vec<(i32, i32)> = Vec::new();
        let mut point_idx: HashMap<(i32, i32), usize> = HashMap::new();

        let mut get_or_insert = |pt: (i32, i32)| -> usize {
            if let Some(&idx) = point_idx.get(&pt) {
                idx
            } else {
                let idx = points.len();
                points.push(pt);
                point_idx.insert(pt, idx);
                idx
            }
        };

        for w in &self.wires {
            get_or_insert((w.x1, w.y1));
            get_or_insert((w.x2, w.y2));
        }
        for c in &self.components {
            for (_, px, py) in component_pins_world(c) {
                get_or_insert((px, py));
            }
        }

        let mut uf = UnionFind::new(points.len());

        // Connect wire endpoints.
        for w in &self.wires {
            let a = point_idx[&(w.x1, w.y1)];
            let b = point_idx[&(w.x2, w.y2)];
            uf.union(a, b);
        }

        // Connect any point (pin or T-junction) that lies on a wire segment.
        for (idx, &(px, py)) in points.iter().enumerate() {
            for w in &self.wires {
                if point_on_segment(px, py, w) {
                    let a = point_idx[&(w.x1, w.y1)];
                    uf.union(idx, a);
                }
            }
        }

        // Merge points that share the same NetLabel or Ground.
        let mut label_roots: HashMap<String, usize> = HashMap::new();
        for c in &self.components {
            let pins = component_pins_world(c);
            if pins.is_empty() {
                continue;
            }
            let idx = point_idx[&(pins[0].1, pins[0].2)];
            match c.kind {
                ComponentKind::Ground => {
                    if let Some(&root) = label_roots.get("0") {
                        uf.union(root, idx);
                    } else {
                        label_roots.insert("0".to_string(), idx);
                    }
                }
                ComponentKind::NetLabel => {
                    let lbl = c.value.trim().to_string();
                    if !lbl.is_empty() {
                        if let Some(&root) = label_roots.get(&lbl) {
                            uf.union(root, idx);
                        } else {
                            label_roots.insert(lbl, idx);
                        }
                    }
                }
                _ => {}
            }
        }

        // Assign canonical net names to each union-find root.
        let mut root_names: HashMap<usize, String> = HashMap::new();
        for (lbl, idx) in &label_roots {
            let r = uf.find(*idx);
            root_names.entry(r).or_insert_with(|| lbl.clone());
        }

        let mut auto_idx = 1usize;
        for i in 0..points.len() {
            let r = uf.find(i);
            if !root_names.contains_key(&r) {
                let name = format!("n{auto_idx:03}");
                auto_idx += 1;
                root_names.insert(r, name);
            }
        }

        let mut point_to_net = HashMap::new();
        for (i, &pt) in points.iter().enumerate() {
            let r = uf.find(i);
            point_to_net.insert(pt, root_names[&r].clone());
        }

        let mut wire_to_net = HashMap::new();
        for w in &self.wires {
            if let Some(net) = point_to_net.get(&(w.x1, w.y1)) {
                wire_to_net.insert(w.id, net.clone());
            }
        }

        let mut lines = vec![format!("* {}", self.name)];
        let mut comp_spice_names = HashMap::new();
        let mut warnings = Vec::new();
        let mut needs_opamp_subckt = false;
        let mut used_models: HashSet<String> = HashSet::new();

        for c in &self.components {
            if !c.kind.has_refdes() {
                continue;
            }
            let pins = component_pins_world(c);
            let pin_nets: Vec<String> = pins
                .iter()
                .map(|(_, px, py)| {
                    point_to_net
                        .get(&(*px, *py))
                        .cloned()
                        .unwrap_or_else(|| "0".to_string())
                })
                .collect();

            // Skip completely unconnected floating spare parts (like spare U2 if all pins are isolated).
            let ref_clean = c.refdes.trim();
            if ref_clean.is_empty() {
                continue;
            }

            let val_spice = normalize_spice_value(&c.value);
            match c.kind {
                ComponentKind::VoltageSource => {
                    let spice_val = translate_ltspice_source(&val_spice);
                    let name = ensure_prefix(ref_clean, 'V');
                    comp_spice_names.insert(c.id, name.clone());
                    if let Some(expr) = spice_val.strip_prefix("__B_EXPR__:") {
                        lines.push(format!("B{name} {} {} {expr}", pin_nets[0], pin_nets[1]));
                    } else {
                        lines.push(format!("{name} {} {} {spice_val}", pin_nets[0], pin_nets[1]));
                    }
                }
                ComponentKind::CurrentSource => {
                    let spice_val = translate_ltspice_source(&val_spice);
                    let name = ensure_prefix(ref_clean, 'I');
                    comp_spice_names.insert(c.id, name.clone());
                    let sense_node = format!("_isense_{name}");
                    lines.push(format!("V_sense_{name} {} {sense_node} 0", pin_nets[0]));
                    lines.push(format!("{name} {sense_node} {} {spice_val}", pin_nets[1]));
                }
                ComponentKind::Resistor => {
                    let name = ensure_prefix(ref_clean, 'R');
                    if let Some(pn) = c.params.get("pn").filter(|s| !s.trim().is_empty()) {
                        if let Some(idx) = model_index {
                            let key = pn.trim().to_ascii_lowercase();
                            if idx.subckts.contains_key(&key) {
                                used_models.insert(format!("SUBCKT:{pn}"));
                                comp_spice_names.insert(c.id, format!("X{name}"));
                                let sense_node = format!("_isense_{name}");
                                lines.push(format!("V_sense_{name} {} {sense_node} 0", pin_nets[0]));
                                lines.push(format!("X{name} {sense_node} {} {pn}", pin_nets[1]));
                                continue;
                            } else if idx.models.contains_key(&key) {
                                used_models.insert(format!("R:{pn}"));
                                comp_spice_names.insert(c.id, name.clone());
                                lines.push(format!(
                                    "{name} {} {} {val_spice} {pn}",
                                    pin_nets[0], pin_nets[1]
                                ));
                                continue;
                            }
                        }
                    }
                    comp_spice_names.insert(c.id, name.clone());
                    lines.push(format!("{name} {} {} {val_spice}", pin_nets[0], pin_nets[1]));
                }
                ComponentKind::Capacitor => {
                    let name = ensure_prefix(ref_clean, 'C');
                    if let Some(pn) = c.params.get("pn").filter(|s| !s.trim().is_empty()) {
                        if let Some(idx) = model_index {
                            let key = pn.trim().to_ascii_lowercase();
                            if idx.subckts.contains_key(&key) {
                                used_models.insert(format!("SUBCKT:{pn}"));
                                comp_spice_names.insert(c.id, format!("X{name}"));
                                let sense_node = format!("_isense_{name}");
                                lines.push(format!("V_sense_{name} {} {sense_node} 0", pin_nets[0]));
                                lines.push(format!("X{name} {sense_node} {} {pn}", pin_nets[1]));
                                continue;
                            } else if idx.models.contains_key(&key) {
                                used_models.insert(format!("C:{pn}"));
                                comp_spice_names.insert(c.id, name.clone());
                                lines.push(format!(
                                    "{name} {} {} {val_spice} {pn}",
                                    pin_nets[0], pin_nets[1]
                                ));
                                continue;
                            }
                        }
                    }
                    let esr = c.params.get("esr").map(|s| normalize_spice_value(s)).filter(|s| !s.is_empty());
                    let esl = c.params.get("esl").map(|s| normalize_spice_value(s)).filter(|s| !s.is_empty());
                    let rpar = c.params.get("rpar").map(|s| normalize_spice_value(s)).filter(|s| !s.is_empty());
                    let cpar = c.params.get("cpar").map(|s| normalize_spice_value(s)).filter(|s| !s.is_empty());

                    if esr.is_some() || esl.is_some() || rpar.is_some() || cpar.is_some() {
                        let sub_name = format!("ITSPICE_CAP_{}", c.id);
                        comp_spice_names.insert(c.id, format!("X{name}"));
                        let sense_node = format!("_isense_{name}");
                        lines.push(format!("V_sense_{name} {} {sense_node} 0", pin_nets[0]));
                        lines.push(format!("X{name} {sense_node} {} {sub_name}", pin_nets[1]));
                        lines.push(format!(".subckt {sub_name} p1 p2"));
                        let mut cur_node = "p1".to_string();
                        if let Some(l_val) = esl {
                            lines.push(format!("Lser {cur_node} n_esl {l_val}"));
                            cur_node = "n_esl".to_string();
                        }
                        if let Some(r_val) = esr {
                            lines.push(format!("Rser {cur_node} n_esr {r_val}"));
                            cur_node = "n_esr".to_string();
                        }
                        lines.push(format!("Cmain {cur_node} p2 {val_spice}"));
                        if let Some(rp_val) = rpar {
                            lines.push(format!("Rpar p1 p2 {rp_val}"));
                        }
                        if let Some(cp_val) = cpar {
                            lines.push(format!("Cpar p1 p2 {cp_val}"));
                        }
                        lines.push(format!(".ends {sub_name}"));
                    } else {
                        comp_spice_names.insert(c.id, name.clone());
                        lines.push(format!("{name} {} {} {val_spice}", pin_nets[0], pin_nets[1]));
                    }
                }
                ComponentKind::Inductor => {
                    let name = ensure_prefix(ref_clean, 'L');
                    comp_spice_names.insert(c.id, name.clone());
                    lines.push(format!("{name} {} {} {val_spice}", pin_nets[0], pin_nets[1]));
                }
                ComponentKind::Diode => {
                    let name = ensure_prefix(ref_clean, 'D');
                    let model = if val_spice.is_empty() { "D".to_string() } else { val_spice };
                    used_models.insert(format!("D:{model}"));
                    comp_spice_names.insert(c.id, name.clone());
                    lines.push(format!("{name} {} {} {model}", pin_nets[0], pin_nets[1]));
                }
                ComponentKind::Nmos => {
                    let name = ensure_prefix(ref_clean, 'M');
                    let model = if val_spice.is_empty() { "NMOS".to_string() } else { val_spice };
                    used_models.insert(format!("NMOS:{model}"));
                    comp_spice_names.insert(c.id, name.clone());
                    let nd = format!("_isense_d_{name}");
                    let ng = format!("_isense_g_{name}");
                    let ns = format!("_isense_s_{name}");
                    lines.push(format!("V_sense_d_{name} {} {nd} 0", pin_nets[0]));
                    lines.push(format!("V_sense_g_{name} {} {ng} 0", pin_nets[1]));
                    lines.push(format!("V_sense_s_{name} {} {ns} 0", pin_nets[2]));
                    lines.push(format!("{name} {nd} {ng} {ns} {ns} {model}"));
                }
                ComponentKind::Pmos => {
                    let name = ensure_prefix(ref_clean, 'M');
                    let model = if val_spice.is_empty() { "PMOS".to_string() } else { val_spice };
                    used_models.insert(format!("PMOS:{model}"));
                    comp_spice_names.insert(c.id, name.clone());
                    let nd = format!("_isense_d_{name}");
                    let ng = format!("_isense_g_{name}");
                    let ns = format!("_isense_s_{name}");
                    lines.push(format!("V_sense_d_{name} {} {nd} 0", pin_nets[0]));
                    lines.push(format!("V_sense_g_{name} {} {ng} 0", pin_nets[1]));
                    lines.push(format!("V_sense_s_{name} {} {ns} 0", pin_nets[2]));
                    lines.push(format!("{name} {nd} {ng} {ns} {ns} {model}"));
                }
                ComponentKind::Npn => {
                    let name = ensure_prefix(ref_clean, 'Q');
                    let model = if val_spice.is_empty() { "NPN".to_string() } else { val_spice };
                    used_models.insert(format!("NPN:{model}"));
                    comp_spice_names.insert(c.id, name.clone());
                    let nc = format!("_isense_c_{name}");
                    let nb = format!("_isense_b_{name}");
                    let ne = format!("_isense_e_{name}");
                    lines.push(format!("V_sense_c_{name} {} {nc} 0", pin_nets[0]));
                    lines.push(format!("V_sense_b_{name} {} {nb} 0", pin_nets[1]));
                    lines.push(format!("V_sense_e_{name} {} {ne} 0", pin_nets[2]));
                    lines.push(format!("{name} {nc} {nb} {ne} {model}"));
                }
                ComponentKind::Pnp => {
                    let name = ensure_prefix(ref_clean, 'Q');
                    let model = if val_spice.is_empty() { "PNP".to_string() } else { val_spice };
                    used_models.insert(format!("PNP:{model}"));
                    comp_spice_names.insert(c.id, name.clone());
                    let nc = format!("_isense_c_{name}");
                    let nb = format!("_isense_b_{name}");
                    let ne = format!("_isense_e_{name}");
                    lines.push(format!("V_sense_c_{name} {} {nc} 0", pin_nets[0]));
                    lines.push(format!("V_sense_b_{name} {} {nb} 0", pin_nets[1]));
                    lines.push(format!("V_sense_e_{name} {} {ne} 0", pin_nets[2]));
                    lines.push(format!("{name} {nc} {nb} {ne} {model}"));
                }
                ComponentKind::OpAmp => {
                    let name = format!("X{ref_clean}");
                    comp_spice_names.insert(c.id, name.clone());
                    needs_opamp_subckt = true;
                    let tags = ["inp", "inn", "vp", "vn", "out"];
                    let mut sensed_nets = Vec::with_capacity(5);
                    for (idx, tag) in tags.iter().enumerate() {
                        let snode = format!("_isense_{tag}_{ref_clean}");
                        lines.push(format!("V_sense_{tag}_{ref_clean} {} {snode} 0", pin_nets[idx]));
                        sensed_nets.push(snode);
                    }
                    lines.push(format!(
                        "{name} {} {} {} {} {} ITSPICE_OPAMP",
                        sensed_nets[0], sensed_nets[1], sensed_nets[2], sensed_nets[3], sensed_nets[4]
                    ));
                }
                ComponentKind::Custom => {
                    if let Some(hdl_blk) = &c.hdl {
                        let name = ensure_prefix(ref_clean, 'X');
                        let sub_name = format!("ITSPICE_HDL_{}", c.id);
                        comp_spice_names.insert(c.id, name.clone());
                        let mut sensed_nets = Vec::with_capacity(pin_nets.len());
                        for (idx, pnet) in pin_nets.iter().enumerate() {
                            let pnum = idx + 1;
                            let snode = format!("_isense_p{pnum}_{name}");
                            lines.push(format!("V_sense_p{pnum}_{name} {pnet} {snode} 0"));
                            sensed_nets.push(snode);
                        }
                        for rnet in hdl_blk.extra_ref_nets() {
                            sensed_nets.push(rnet);
                        }
                        lines.push(format!("{name} {} {sub_name}", sensed_nets.join(" ")));
                        match hdl_blk.synthesize_subckt(&sub_name) {
                            Ok(sub_lines) => lines.extend(sub_lines),
                            Err(e) => warnings.push(format!(
                                "HDL synthesis failed for '{}' ({}): {e}",
                                c.refdes, hdl_blk.module_name
                            )),
                        }
                    } else {
                        let prefix_ch = c
                            .custom_symbol
                            .as_ref()
                            .and_then(|s| s.prefix.chars().next())
                            .unwrap_or('X');
                        let name = ensure_prefix(ref_clean, prefix_ch);
                        comp_spice_names.insert(c.id, name.clone());
                        let model_or_subckt = if val_spice.is_empty() {
                            c.custom_symbol
                                .as_ref()
                                .map(|s| s.default_value.clone())
                                .unwrap_or_else(|| "CUSTOM".into())
                        } else {
                            val_spice
                        };
                        used_models.insert(format!("SUBCKT:{model_or_subckt}"));
                        let mut sensed_nets = Vec::with_capacity(pin_nets.len());
                        for (idx, pnet) in pin_nets.iter().enumerate() {
                            let pnum = idx + 1;
                            let snode = format!("_isense_p{pnum}_{name}");
                            lines.push(format!("V_sense_p{pnum}_{name} {pnet} {snode} 0"));
                            sensed_nets.push(snode);
                        }
                        lines.push(format!("{name} {} {model_or_subckt}", sensed_nets.join(" ")));
                    }
                }
                ComponentKind::Ground | ComponentKind::NetLabel => {}
            }
        }

        // Check if user directives already define models or analyses.
        let directive_text_all: String = self
            .directives
            .iter()
            .map(|d| d.text.to_lowercase())
            .collect::<Vec<_>>()
            .join("\n");

        let mut included_files: HashSet<String> = HashSet::new();
        for spec in used_models {
            if let Some((kind, mname)) = spec.split_once(':') {
                let pattern = format!(".model {}", mname.to_lowercase());
                if !directive_text_all.contains(&pattern) {
                    if let Some(path) = model_index.and_then(|idx| idx.find_model_file(mname)) {
                        let p_str = path.display().to_string();
                        if included_files.insert(p_str.clone()) {
                            lines.push(format!(".include \"{p_str}\""));
                        }
                        continue;
                    }
                    match kind {
                        "D" => lines.push(format!(".model {mname} D(IS=1e-14 RS=0.1 N=1.0)")),
                        "NMOS" => {
                            if mname.eq_ignore_ascii_case("AO6408") {
                                lines.push(".model AO6408 NMOS(VTO=1.2 KP=20 RD=0.02 RS=0.02)".into());
                            } else {
                                lines.push(format!(".model {mname} NMOS(VTO=1.5 KP=2.0)"));
                            }
                        }
                        "PMOS" => lines.push(format!(".model {mname} PMOS(VTO=-1.5 KP=1.0)")),
                        "NPN" => lines.push(format!(".model {mname} NPN(BF=150 IS=1e-14)")),
                        "PNP" => lines.push(format!(".model {mname} PNP(BF=100 IS=1e-14)")),
                        _ => {}
                    }
                }
            }
        }

        if needs_opamp_subckt {
            lines.push(".subckt ITSPICE_OPAMP inp inm vp vm out".into());
            lines.push("Rin_p inp 0 1e9".into());
            lines.push("Rin_m inm 0 1e9".into());
            lines.push("Rsup_p vp 0 1e9".into());
            lines.push("Rsup_m vm 0 1e9".into());
            lines.push(
                "B_out n_int 0 V = 0.5*(V(vp)+V(vm)) + 0.5*(V(vp)-V(vm))*tanh(1000*(V(inp)-V(inm)))"
                    .into(),
            );
            lines.push("Rout n_int out 10".into());
            lines.push("Cout out 0 10p".into());
            lines.push(".ends ITSPICE_OPAMP".into());
        }

        let mut has_analysis = false;
        for d in &self.directives {
            for raw_line in d.text.lines() {
                let trimmed = raw_line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let low = trimmed.to_lowercase();
                if low.starts_with(".tran")
                    || low.starts_with(".ac")
                    || low.starts_with(".dc")
                    || low.starts_with(".op")
                {
                    has_analysis = true;
                    // ngspice `.tran <tstep> <tstop> [tstart [tmax]] [uic]`
                    if low.starts_with(".tran ") {
                        let mut nums: Vec<&str> = Vec::new();
                        let mut has_uic = false;
                        for tok in trimmed.split_whitespace().skip(1) {
                            let tlow = tok.to_ascii_lowercase();
                            if tlow == "uic" {
                                has_uic = true;
                            } else if !matches!(
                                tlow.as_str(),
                                "startup" | "steady" | "nodiscard" | "step"
                            ) {
                                nums.push(tok);
                            }
                        }
                        if nums.len() == 1 {
                            let tstop = nums[0];
                            if let Some(tstop_val) = parse_si_number(tstop) {
                                let tstep = tstop_val / 2000.0;
                                let uic_str = if has_uic { " uic" } else { "" };
                                lines.push(format!(".tran {tstep:.6e} {tstop}{uic_str}"));
                                continue;
                            }
                        } else if nums.len() >= 2 {
                            let tstop = nums[1];
                            let tstep = if nums[0] == "0" {
                                parse_si_number(tstop)
                                    .map(|v| format!("{:.6e}", v / 2000.0))
                                    .unwrap_or_else(|| "1u".to_string())
                            } else {
                                nums[0].to_string()
                            };
                            let mut out = format!(".tran {tstep} {tstop}");
                            for extra in &nums[2..] {
                                out.push(' ');
                                out.push_str(extra);
                            }
                            if has_uic {
                                out.push_str(" uic");
                            }
                            lines.push(out);
                            continue;
                        }
                    }
                }
                lines.push(trimmed.to_string());
            }
        }

        if !has_analysis {
            warnings.push("No analysis directive (.tran, .ac, .dc, .op) found; defaulting to .op".into());
            lines.push(".op".into());
        }

        // Ensure component currents (R, C, D) are saved alongside node voltages unless user specified .save
        let has_save = lines
            .iter()
            .any(|l| l.trim_start().to_ascii_lowercase().starts_with(".save"));
        if !has_save {
            let mut save_items: Vec<String> = Vec::new();
            for spice_name in comp_spice_names.values() {
                let lower = spice_name.to_ascii_lowercase();
                if lower.starts_with('r') || lower.starts_with('c') {
                    save_items.push(format!("@{lower}[i]"));
                } else if lower.starts_with('d') {
                    save_items.push(format!("@{lower}[id]"));
                }
            }
            if !save_items.is_empty() {
                save_items.sort();
                save_items.dedup();
                lines.push(format!(".save all {}", save_items.join(" ")));
            }
        }

        lines.push(".end".into());

        NetlistResult {
            lines,
            point_to_net,
            wire_to_net,
            comp_spice_names,
            warnings,
        }
    }

    pub fn to_asc(&self) -> String {
        let mut out = String::new();
        out.push_str("Version 4\nSHEET 1 880 680\n");
        for w in &self.wires {
            out.push_str(&format!(
                "WIRE {} {} {} {}\n",
                w.x1 * 16,
                w.y1 * 16,
                w.x2 * 16,
                w.y2 * 16
            ));
        }
        for c in &self.components {
            match c.kind {
                ComponentKind::Ground => {
                    out.push_str(&format!("FLAG {} {} 0\n", c.x * 16, c.y * 16));
                }
                ComponentKind::NetLabel => {
                    out.push_str(&format!("FLAG {} {} {}\n", c.x * 16, c.y * 16, c.value));
                }
                _ => {
                    let sym = match c.kind {
                        ComponentKind::VoltageSource => "voltage",
                        ComponentKind::CurrentSource => "current",
                        ComponentKind::Resistor => "res",
                        ComponentKind::Capacitor => "cap",
                        ComponentKind::Inductor => "ind",
                        ComponentKind::Diode => "diode",
                        ComponentKind::Nmos => "nmos",
                        ComponentKind::Pmos => "pmos",
                        ComponentKind::Npn => "npn",
                        ComponentKind::Pnp => "pnp",
                        ComponentKind::OpAmp => "Opamps\\\\opamp2",
                        ComponentKind::Custom => c
                            .custom_symbol
                            .as_ref()
                            .map(|s| s.name.as_str())
                            .unwrap_or("custom"),
                        ComponentKind::Ground | ComponentKind::NetLabel => unreachable!(),
                    };
                    let orient = format!(
                        "{}{}",
                        if c.mirror { "M" } else { "R" },
                        ((c.rotation % 360) + 360) % 360
                    );
                    out.push_str(&format!("SYMBOL {sym} {} {} {orient}\n", c.x * 16, c.y * 16));
                    if !c.refdes.is_empty() {
                        out.push_str(&format!("SYMATTR InstName {}\n", c.refdes));
                    }
                    if !c.value.is_empty() {
                        out.push_str(&format!("SYMATTR Value {}\n", c.value));
                    }
                }
            }
        }
        for d in &self.directives {
            let escaped = d.text.replace('\n', "\\n");
            out.push_str(&format!("TEXT {} {} Left 2 !{escaped}\n", d.x * 16, d.y * 16));
        }
        out
    }

    pub fn from_asc(name: &str, content: &str) -> Self {
        let mut sch = Self::new_empty(name);
        let mut last_comp_idx: Option<usize> = None;

        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }
            match parts[0] {
                "WIRE" if parts.len() >= 5 => {
                    let x1 = parts[1].parse::<i32>().unwrap_or(0) / 16;
                    let y1 = parts[2].parse::<i32>().unwrap_or(0) / 16;
                    let x2 = parts[3].parse::<i32>().unwrap_or(0) / 16;
                    let y2 = parts[4].parse::<i32>().unwrap_or(0) / 16;
                    sch.add_wire(x1, y1, x2, y2);
                    last_comp_idx = None;
                }
                "FLAG" if parts.len() >= 4 => {
                    let x = parts[1].parse::<i32>().unwrap_or(0) / 16;
                    let y = parts[2].parse::<i32>().unwrap_or(0) / 16;
                    let lbl = parts[3];
                    if lbl == "0" {
                        sch.add_component(ComponentKind::Ground, String::new(), "0".into(), x, y);
                    } else {
                        sch.add_component(ComponentKind::NetLabel, String::new(), lbl.into(), x, y);
                    }
                    last_comp_idx = None;
                }
                "SYMBOL" if parts.len() >= 5 => {
                    let sym_low = parts[1].to_lowercase();
                    let x = parts[2].parse::<i32>().unwrap_or(0) / 16;
                    let y = parts[3].parse::<i32>().unwrap_or(0) / 16;
                    let orient = parts[4];
                    let mirror = orient.starts_with('M');
                    let rotation = orient[1..].parse::<i32>().unwrap_or(0);
                    let kind = if sym_low.contains("res") {
                        ComponentKind::Resistor
                    } else if sym_low.contains("cap") {
                        ComponentKind::Capacitor
                    } else if sym_low.contains("ind") {
                        ComponentKind::Inductor
                    } else if sym_low.contains("diode") {
                        ComponentKind::Diode
                    } else if sym_low.contains("voltage") {
                        ComponentKind::VoltageSource
                    } else if sym_low.contains("current") {
                        ComponentKind::CurrentSource
                    } else if sym_low.contains("nmos") {
                        ComponentKind::Nmos
                    } else if sym_low.contains("pmos") {
                        ComponentKind::Pmos
                    } else if sym_low.contains("npn") {
                        ComponentKind::Npn
                    } else if sym_low.contains("pnp") {
                        ComponentKind::Pnp
                    } else if sym_low.contains("opamp") {
                        ComponentKind::OpAmp
                    } else {
                        ComponentKind::Resistor
                    };
                    let refdes = sch.next_refdes(kind);
                    let val = kind.default_value().to_string();
                    sch.add_component(kind, refdes, val, x, y);
                    if let Some(c) = sch.components.last_mut() {
                        c.rotation = rotation;
                        c.mirror = mirror;
                    }
                    last_comp_idx = Some(sch.components.len() - 1);
                }
                "SYMATTR" if parts.len() >= 3 => {
                    if let Some(idx) = last_comp_idx {
                        let attr = parts[1];
                        let val = parts[2..].join(" ");
                        if attr == "InstName" {
                            sch.components[idx].refdes = val;
                        } else if attr == "Value" {
                            sch.components[idx].value = val;
                        }
                    }
                }
                "TEXT" if parts.len() >= 6 => {
                    let x = parts[1].parse::<i32>().unwrap_or(0) / 16;
                    let y = parts[2].parse::<i32>().unwrap_or(0) / 16;
                    let rest = parts[5..].join(" ");
                    if let Some(dir) = rest.strip_prefix('!') {
                        sch.add_directive(x, y, dir.replace("\\n", "\n"));
                    }
                    last_comp_idx = None;
                }
                _ => {}
            }
        }
        sch
    }
}

fn ensure_prefix(refdes: &str, prefix: char) -> String {
    if refdes
        .chars()
        .next()
        .is_some_and(|ch| ch.eq_ignore_ascii_case(&prefix))
    {
        refdes.to_string()
    } else {
        format!("{prefix}{refdes}")
    }
}

fn normalize_spice_value(v: &str) -> String {
    v.trim().replace('µ', "u").replace('μ', "u")
}

/// Translates LTspice `SINE(Voffset Vamp Freq Td Theta Phi Ncycles)` when Ncycles > 0
/// into a genuine gated sine expression for ngspice, or standard `SIN(...)` otherwise.
fn translate_ltspice_source(val: &str) -> String {
    let upper = val.to_ascii_uppercase();
    if upper.starts_with("SINE(") && val.ends_with(')') {
        let inner = &val[5..val.len() - 1];
        let args: Vec<&str> = inner.split_whitespace().collect();
        if args.len() >= 7 {
            let voff = parse_si_number(args[0]).unwrap_or(0.0);
            let vamp = parse_si_number(args[1]).unwrap_or(1.0);
            let freq = parse_si_number(args[2]).unwrap_or(1e3);
            let td = parse_si_number(args[3]).unwrap_or(0.0);
            let theta = parse_si_number(args[4]).unwrap_or(0.0);
            let phi_deg = parse_si_number(args[5]).unwrap_or(0.0);
            let n = parse_si_number(args[6]).unwrap_or(0.0);
            if n > 0.0 && freq > 0.0 {
                let t_end = td + n / freq;
                let phi_rad = phi_deg.to_radians();
                let omega = 2.0 * std::f64::consts::PI * freq;
                return format!(
                    "__B_EXPR__:V = {voff} + ((time >= {td:.9e}) && (time <= {t_end:.9e})) ? ({vamp} * sin({omega:.9e} * (time - {td:.9e}) + {phi_rad:.9e}) * exp(-({theta:.9e}) * (time - {td:.9e}))) : 0"
                );
            }
            return format!("SIN({})", args[..6].join(" "));
        }
        return format!("SIN({inner})");
    }
    val.to_string()
}

pub fn parse_si_number(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let lower = s.to_ascii_lowercase();
    let (num_part, mult) = if let Some(n) = lower.strip_suffix("meg") {
        (n, 1e6)
    } else if let Some(n) = lower.strip_suffix('g') {
        (n, 1e9)
    } else if let Some(n) = lower.strip_suffix('k') {
        (n, 1e3)
    } else if let Some(n) = lower.strip_suffix('m') {
        (n, 1e-3)
    } else if let Some(n) = lower.strip_suffix('u') {
        (n, 1e-6)
    } else if let Some(n) = lower.strip_suffix('n') {
        (n, 1e-9)
    } else if let Some(n) = lower.strip_suffix('p') {
        (n, 1e-12)
    } else if let Some(n) = lower.strip_suffix('f') {
        (n, 1e-15)
    } else {
        (lower.as_str(), 1.0)
    };
    num_part.trim().parse::<f64>().ok().map(|v| v * mult)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ngspice::NgSpiceEngine;

    #[test]
    fn test_default_demo_netlist_simulates_in_ngspice() {
        let sch = Schematic::default_ltspice_demo();
        let net = sch.to_spice_netlist();
        let engine_mutex = NgSpiceEngine::global().expect("libngspice should load");
        let engine = engine_mutex.lock().unwrap();
        let res = engine.run_netlist(&net.lines);
        assert!(res.success, "demo sim failed: {:?}\n{:#?}\nlogs:{:#?}", res.error, net.lines, res.log);
        assert!(!res.x_values.is_empty(), "empty x_values; plot={} logs:{:#?}\nnetlist:{:#?}", res.plot_name, res.log, net.lines);
        // Should have V(in) and V(out) sine waves
        let vin = res.traces.get("V(in)").expect("V(in) should exist");
        let vout = res.traces.get("V(out)").expect("V(out) should exist");
        let max_in = vin.real.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let max_out = vout.real.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        assert!(max_in > 4.5, "expected ~5V sine peak on V(in), got {max_in}");
        assert!(max_out > 2.5 && max_out < 5.0, "expected attenuated RC sine on V(out), got {max_out}");
        let keys: Vec<&String> = res.traces.keys().collect();
        assert!(res.traces.contains_key("I(c1)"), "missing I(c1) in {keys:?}\nnetlist:\n{}", net.lines.join("\n"));
        assert!(res.traces.contains_key("I(r1)"), "missing I(r1) in {keys:?}\nnetlist:\n{}", net.lines.join("\n"));
    }

    #[test]
    fn test_place_resistor_splits_wire() {
        let mut sch = Schematic::new_empty("split_test.asc");
        // Horizontal wire from (0, 10) to (10, 10)
        sch.add_wire(0, 10, 10, 10);
        // Place horizontal resistor (rotation 90) centered at (5, 10) -> pins at (3, 10) and (7, 10)
        let rid = sch.add_component(ComponentKind::Resistor, "R1".into(), "1k".into(), 5, 10);
        sch.components.last_mut().unwrap().rotation = 90;
        sch.split_wires_for_component(rid);

        assert_eq!(sch.wires.len(), 2);
        let net = sch.to_spice_netlist();
        let n_left = &net.point_to_net[&(0, 10)];
        let n_right = &net.point_to_net[&(10, 10)];
        assert_ne!(n_left, n_right, "wire should be cut between resistor pads");
    }

    #[test]
    fn test_draw_wire_across_component_cuts_between_pins() {
        let mut sch = Schematic::new_empty("wire_cut_test.asc");
        sch.add_component(ComponentKind::Resistor, "R1".into(), "1k".into(), 5, 10);
        sch.components.last_mut().unwrap().rotation = 90; // pins at (3, 10) and (7, 10)
        sch.add_wire(0, 10, 10, 10);
        sch.split_wires_for_all_components();

        assert_eq!(sch.wires.len(), 2);
        let net = sch.to_spice_netlist();
        assert_ne!(
            net.point_to_net[&(0, 10)],
            net.point_to_net[&(10, 10)],
            "wire drawn across resistor should be cut between its two pins"
        );
    }

    #[test]
    fn test_model_index_includes_user_model_file() {
        let dir = std::env::temp_dir().join("mcp_itspice_model_test");
        let _ = std::fs::create_dir_all(&dir);
        let lib_path = dir.join("custom_mos.lib");
        std::fs::write(
            &lib_path,
            ".model MY_CUSTOM_NMOS NMOS(VTO=1.1 KP=15)\n",
        )
        .unwrap();

        let index = ModelIndex::scan(&[dir.clone()]);
        assert!(index.find_model_file("MY_CUSTOM_NMOS").is_some());

        let mut sch = Schematic::new_empty("user_model_test.asc");
        sch.add_component(
            ComponentKind::Nmos,
            "M1".into(),
            "MY_CUSTOM_NMOS".into(),
            10,
            10,
        );
        let net = sch.to_spice_netlist_with_models(Some(&index));
        assert!(
            net.lines
                .iter()
                .any(|l| l.starts_with(".include ") && l.contains("custom_mos.lib")),
            "expected .include for custom_mos.lib in {:?}",
            net.lines
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_capacitor_parasitics_simulate_in_ngspice() {
        let mut sch = Schematic::new_empty("cap_par.asc");
        sch.add_component(ComponentKind::VoltageSource, "V1".into(), "5".into(), 0, 2);
        sch.add_component(ComponentKind::Ground, String::new(), "0".into(), 0, 5);
        sch.add_wire(0, 4, 0, 5);
        let cid = sch.add_component(ComponentKind::Capacitor, "C1".into(), "10u".into(), 4, 2);
        let cap = sch.components.last_mut().unwrap();
        cap.params.insert("esr".into(), "0.5".into());
        cap.params.insert("esl".into(), "5n".into());
        cap.params.insert("rpar".into(), "100k".into());
        sch.add_wire(0, 0, 4, 0);
        sch.add_component(ComponentKind::Ground, String::new(), "0".into(), 4, 5);
        sch.add_wire(4, 4, 4, 5);
        sch.add_directive(0, 8, ".op".into());

        let net = sch.to_spice_netlist();
        assert!(net.lines.iter().any(|l| l.starts_with(".subckt ITSPICE_CAP_")));
        let _ = cid;
        let engine = NgSpiceEngine::global().unwrap().lock().unwrap();
        let res = engine.run_netlist(&net.lines);
        assert!(res.success, "sim failed: {:?}", res.error);
        assert!(res.traces.contains_key("I(c1)"), "expected I(c1) for subcircuit cap, got {:?}", res.traces.keys().collect::<Vec<_>>());
    }
}
