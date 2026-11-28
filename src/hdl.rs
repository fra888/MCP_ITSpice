use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::symbol_editor::{CustomSymbol, SymbolPin, SymbolShape};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HdlLanguage {
    Verilog,
    Vhdl,
}

impl HdlLanguage {
    pub fn label(self) -> &'static str {
        match self {
            Self::Verilog => "Verilog",
            Self::Vhdl => "VHDL",
        }
    }

    pub fn short_tag(self) -> &'static str {
        match self {
            Self::Verilog => "V",
            Self::Vhdl => "VHDL",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HdlPortDir {
    Input,
    Output,
    Inout,
}

impl HdlPortDir {
    pub fn label(self) -> &'static str {
        match self {
            Self::Input => "in",
            Self::Output => "out",
            Self::Inout => "inout",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HdlPort {
    pub name: String,
    pub dir: HdlPortDir,
    /// Bit width (1 for scalar, >1 for vectors like [3:0])
    pub width: usize,
    pub msb: i32,
    pub lsb: i32,
}

impl HdlPort {
    pub fn expanded_pin_names(&self) -> Vec<String> {
        if self.width <= 1 {
            vec![self.name.clone()]
        } else if self.msb >= self.lsb {
            (self.lsb..=self.msb)
                .map(|i| format!("{}[{i}]", self.name))
                .collect()
        } else {
            (self.msb..=self.lsb)
                .rev()
                .map(|i| format!("{}[{i}]", self.name))
                .collect()
        }
    }

    pub fn expanded_spice_names(&self) -> Vec<String> {
        if self.width <= 1 {
            vec![sanitize_spice_id(&self.name)]
        } else if self.msb >= self.lsb {
            (self.lsb..=self.msb)
                .map(|i| format!("{}_{i}", sanitize_spice_id(&self.name)))
                .collect()
        } else {
            (self.msb..=self.lsb)
                .rev()
                .map(|i| format!("{}_{i}", sanitize_spice_id(&self.name)))
                .collect()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HdlBackend {
    #[default]
    Auto,
    BuiltInXspice,
    VerilatorCosim,
}

impl HdlBackend {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto (Built-in XSPICE / Verilator)",
            Self::BuiltInXspice => "Built-in XSPICE Synthesizer",
            Self::VerilatorCosim => "Verilator d_cosim (external)",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HdlPortConfig {
    /// Reference net name (empty, "0", or "gnd" => ground 0)
    #[serde(default)]
    pub ref_net: String,
    /// Optional custom input low threshold V_IL (V, relative to ref_net)
    #[serde(default)]
    pub v_thresh_low: Option<f64>,
    /// Optional custom input high threshold V_IH (V, relative to ref_net)
    #[serde(default)]
    pub v_thresh_high: Option<f64>,
    /// Optional custom output logic-0 voltage V_low (V, relative to ref_net)
    #[serde(default)]
    pub v_low: Option<f64>,
    /// Optional custom output logic-1 voltage V_high (V, relative to ref_net)
    #[serde(default)]
    pub v_high: Option<f64>,
}

impl HdlPortConfig {
    pub fn is_default(&self) -> bool {
        let r = self.ref_net.trim();
        (r.is_empty() || r == "0" || r.eq_ignore_ascii_case("gnd"))
            && self.v_thresh_low.is_none()
            && self.v_thresh_high.is_none()
            && self.v_low.is_none()
            && self.v_high.is_none()
    }

    pub fn normalized_ref_net(&self) -> Option<String> {
        let r = self.ref_net.trim();
        if r.is_empty() || r == "0" || r.eq_ignore_ascii_case("gnd") {
            None
        } else {
            Some(r.to_string())
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HdlBlock {
    pub language: HdlLanguage,
    pub module_name: String,
    pub source: String,
    #[serde(default)]
    pub source_file: Option<String>,
    #[serde(default)]
    pub ports: Vec<HdlPort>,
    #[serde(default = "default_v_low")]
    pub v_low: f64,
    #[serde(default = "default_v_high")]
    pub v_high: f64,
    #[serde(default = "default_v_il")]
    pub v_thresh_low: f64,
    #[serde(default = "default_v_ih")]
    pub v_thresh_high: f64,
    #[serde(default = "default_t_edge")]
    pub t_rise: String,
    #[serde(default = "default_t_edge")]
    pub t_fall: String,
    #[serde(default)]
    pub backend: HdlBackend,
    /// Per-port (or per-pin) reference net & threshold/level overrides
    #[serde(default)]
    pub port_configs: HashMap<String, HdlPortConfig>,
    /// Physical analog reference input pins drawn on the bottom of the HDL symbol
    #[serde(default)]
    pub ref_pins: Vec<String>,
    /// Optional user-customized symbol (overrides auto-generated IC box)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_symbol_override: Option<CustomSymbol>,
}

fn default_v_low() -> f64 {
    0.0
}
fn default_v_high() -> f64 {
    5.0
}
fn default_v_il() -> f64 {
    1.5
}
fn default_v_ih() -> f64 {
    3.5
}
fn default_t_edge() -> String {
    "5n".into()
}

impl Default for HdlBlock {
    fn default() -> Self {
        let mut b = Self {
            language: HdlLanguage::Verilog,
            module_name: "logic_gate".into(),
            source: verilog_templates()[0].1.to_string(),
            source_file: None,
            ports: Vec::new(),
            v_low: 0.0,
            v_high: 5.0,
            v_thresh_low: 1.5,
            v_thresh_high: 3.5,
            t_rise: "5n".into(),
            t_fall: "5n".into(),
            backend: HdlBackend::Auto,
            port_configs: HashMap::new(),
            ref_pins: Vec::new(),
            custom_symbol_override: None,
        };
        b.refresh_metadata();
        b
    }
}

impl HdlBlock {
    /// Returns true if `name` matches one of this block's wired analog reference pins.
    pub fn find_ref_pin(&self, name: &str) -> Option<String> {
        let t = name.trim();
        if t.is_empty() {
            return None;
        }
        self.ref_pins
            .iter()
            .find(|rp| rp.eq_ignore_ascii_case(t))
            .cloned()
    }

    /// Deterministic list of external non-GND reference nets used by per-port configs
    /// that are NOT already physical `ref_pins` on the symbol.
    pub fn extra_ref_nets(&self) -> Vec<String> {
        let mut refs: Vec<String> = self
            .port_configs
            .values()
            .filter_map(|cfg| cfg.normalized_ref_net())
            .filter(|rn| self.find_ref_pin(rn).is_none())
            .collect();
        refs.sort();
        refs.dedup();
        refs
    }

    /// Resolve effective `(ref_net, v_il, v_ih, v_low, v_high)` for a given port/pin.
    pub fn effective_port_settings(
        &self,
        port_name: &str,
        pin_name: &str,
    ) -> (Option<String>, f64, f64, f64, f64) {
        let cfg = self
            .port_configs
            .get(pin_name)
            .or_else(|| self.port_configs.get(port_name))
            .or_else(|| {
                self.port_configs
                    .iter()
                    .find(|(k, _)| {
                        k.eq_ignore_ascii_case(pin_name) || k.eq_ignore_ascii_case(port_name)
                    })
                    .map(|(_, v)| v)
            });
        if let Some(c) = cfg {
            (
                c.normalized_ref_net(),
                c.v_thresh_low.unwrap_or(self.v_thresh_low),
                c.v_thresh_high.unwrap_or(self.v_thresh_high),
                c.v_low.unwrap_or(self.v_low),
                c.v_high.unwrap_or(self.v_high),
            )
        } else {
            (
                None,
                self.v_thresh_low,
                self.v_thresh_high,
                self.v_low,
                self.v_high,
            )
        }
    }

    pub fn refresh_metadata(&mut self) {
        if let Ok(parsed) = parse_hdl(self.language, &self.source) {
            if !parsed.module_name.is_empty() {
                self.module_name = parsed.module_name;
            }
            self.ports = parsed.ports;
        }
    }

    pub fn expanded_pins(&self) -> Vec<(String, HdlPortDir)> {
        let mut out = Vec::new();
        for p in &self.ports {
            for name in p.expanded_pin_names() {
                out.push((name, p.dir));
            }
        }
        out
    }

    /// Return the custom symbol override (synced with current pins) if present,
    /// otherwise auto-generate the default IC-style symbol.
    pub fn build_symbol(&self) -> CustomSymbol {
        if let Some(mut sym) = self.custom_symbol_override.clone() {
            sym.name = self.module_name.clone();
            sym.default_value = self.module_name.clone();
            self.sync_symbol_pins(&mut sym);
            return sym;
        }
        self.build_default_symbol()
    }

    /// Expected ordered pin list matching the synthesized `.subckt` header:
    /// `[digital_inputs..., digital_outputs..., wired_ref_pins...]`.
    pub fn expected_symbol_pin_names(&self) -> Vec<String> {
        let mut left_pins: Vec<String> = Vec::new();
        let mut right_pins: Vec<String> = Vec::new();
        for p in &self.ports {
            let names = p.expanded_pin_names();
            match p.dir {
                HdlPortDir::Input => left_pins.extend(names),
                HdlPortDir::Output | HdlPortDir::Inout => right_pins.extend(names),
            }
        }
        let mut all = left_pins;
        all.extend(right_pins);
        all.extend(self.ref_pins.iter().cloned());
        all
    }

    /// Synchronize a custom symbol's pins with the block's current ports & ref_pins,
    /// preserving custom `(x, y)` positions for pins whose names still exist.
    pub fn sync_symbol_pins(&self, sym: &mut CustomSymbol) {
        let expected = self.expected_symbol_pin_names();
        let default_sym = self.build_default_symbol();
        let old_positions: HashMap<String, (i32, i32)> = sym
            .pins
            .iter()
            .map(|p| (p.name.clone(), (p.x, p.y)))
            .collect();
        let def_positions: HashMap<String, (i32, i32)> = default_sym
            .pins
            .iter()
            .map(|p| (p.name.clone(), (p.x, p.y)))
            .collect();

        sym.pins = expected
            .into_iter()
            .enumerate()
            .map(|(i, name)| {
                let (x, y) = old_positions
                    .get(&name)
                    .or_else(|| def_positions.get(&name))
                    .copied()
                    .unwrap_or((0, i as i32 * 2));
                SymbolPin {
                    name,
                    order: i + 1,
                    x,
                    y,
                }
            })
            .collect();
    }

    /// Generate the default IC-style schematic symbol for this HDL block.
    pub fn build_default_symbol(&self) -> CustomSymbol {
        let mut left_pins: Vec<String> = Vec::new();
        let mut right_pins: Vec<String> = Vec::new();
        for p in &self.ports {
            let names = p.expanded_pin_names();
            match p.dir {
                HdlPortDir::Input => left_pins.extend(names),
                HdlPortDir::Output | HdlPortDir::Inout => right_pins.extend(names),
            }
        }

        let max_side = left_pins.len().max(right_pins.len()).max(1) as i32;
        let spacing = 2i32;
        let extra_bottom = if self.ref_pins.is_empty() { 0 } else { 1 };
        let half_h = ((max_side - 1) * spacing / 2 + 2 + extra_bottom).max(2);
        let min_w_for_refs = if self.ref_pins.len() > 2 {
            ((self.ref_pins.len() as i32) * 2 / 2 + 1).max(4)
        } else {
            4
        };
        let half_w = min_w_for_refs;

        let mut shapes = vec![SymbolShape::Rect {
            x1: -half_w as f32,
            y1: -half_h as f32,
            x2: half_w as f32,
            y2: half_h as f32,
        }];

        // Header badge inside the box
        let tag = format!("[{}]", self.language.short_tag());
        shapes.push(SymbolShape::Text {
            x: -half_w as f32 + 0.4,
            y: -half_h as f32 + 0.7,
            text: format!("{} {tag}", self.module_name),
        });

        let mut pins = Vec::new();
        let mut order = 1usize;

        // Place input pins on left (x = -half_w - 1)
        let left_start_y = -((left_pins.len() as i32 - 1) * spacing) / 2;
        for (i, pname) in left_pins.iter().enumerate() {
            let py = left_start_y + (i as i32) * spacing;
            let px = -half_w - 1;
            shapes.push(SymbolShape::Line {
                x1: px as f32,
                y1: py as f32,
                x2: -half_w as f32,
                y2: py as f32,
            });
            shapes.push(SymbolShape::Text {
                x: -half_w as f32 + 0.3,
                y: py as f32,
                text: pname.clone(),
            });
            pins.push(SymbolPin {
                name: pname.clone(),
                order,
                x: px,
                y: py,
            });
            order += 1;
        }

        // Place output/inout pins on right (x = half_w + 1)
        let right_start_y = -((right_pins.len() as i32 - 1) * spacing) / 2;
        for (i, pname) in right_pins.iter().enumerate() {
            let py = right_start_y + (i as i32) * spacing;
            let px = half_w + 1;
            shapes.push(SymbolShape::Line {
                x1: half_w as f32,
                y1: py as f32,
                x2: px as f32,
                y2: py as f32,
            });
            let txt_x = (half_w as f32 - (pname.len() as f32 * 0.42).min(2.6)).max(0.2);
            shapes.push(SymbolShape::Text {
                x: txt_x,
                y: py as f32,
                text: pname.clone(),
            });
            pins.push(SymbolPin {
                name: pname.clone(),
                order,
                x: px,
                y: py,
            });
            order += 1;
        }

        // Place wired analog reference pins along the bottom edge (y = half_h + 1)
        if !self.ref_pins.is_empty() {
            let n_ref = self.ref_pins.len() as i32;
            let ref_start_x = -((n_ref - 1) * spacing) / 2;
            for (i, rname) in self.ref_pins.iter().enumerate() {
                let px = ref_start_x + (i as i32) * spacing;
                let py = half_h + 1;
                shapes.push(SymbolShape::Line {
                    x1: px as f32,
                    y1: half_h as f32,
                    x2: px as f32,
                    y2: py as f32,
                });
                shapes.push(SymbolShape::Text {
                    x: px as f32 - (rname.len() as f32 * 0.22),
                    y: half_h as f32 - 0.55,
                    text: rname.clone(),
                });
                pins.push(SymbolPin {
                    name: rname.clone(),
                    order,
                    x: px,
                    y: py,
                });
                order += 1;
            }
        }

        CustomSymbol {
            name: self.module_name.clone(),
            prefix: "U".into(),
            default_value: self.module_name.clone(),
            pins,
            shapes,
        }
    }

    /// Synthesize this HDL block into an ngspice `.subckt` definition.
    pub fn synthesize_subckt(&self, subckt_name: &str) -> Result<Vec<String>, String> {
        match self.backend {
            HdlBackend::BuiltInXspice => synthesize_to_xspice(self, subckt_name),
            HdlBackend::VerilatorCosim => synthesize_to_verilator_cosim(self, subckt_name),
            HdlBackend::Auto => match synthesize_to_xspice(self, subckt_name) {
                Ok(lines) => Ok(lines),
                Err(builtin_err) => {
                    if self.language == HdlLanguage::Verilog && has_verilator_cosim_tools() {
                        synthesize_to_verilator_cosim(self, subckt_name)
                    } else {
                        Err(builtin_err)
                    }
                }
            },
        }
    }
}

pub fn verilog_templates() -> &'static [(&'static str, &'static str)] {
    &[
        (
            "Logic Gate (AND / XOR)",
            "module logic_gate(\n    input a,\n    input b,\n    output y_and,\n    output y_xor\n);\n    assign y_and = a & b;\n    assign y_xor = a ^ b;\nendmodule\n",
        ),
        (
            "2:1 Multiplexer",
            "module mux2(\n    input a,\n    input b,\n    input sel,\n    output y\n);\n    assign y = sel ? b : a;\nendmodule\n",
        ),
        (
            "D Flip-Flop (Async Reset)",
            "module dff_rst(\n    input clk,\n    input rst,\n    input d,\n    output reg q\n);\n    always @(posedge clk or posedge rst) begin\n        if (rst)\n            q <= 1'b0;\n        else\n            q <= d;\n    end\nendmodule\n",
        ),
        (
            "4-bit Counter",
            "module counter4(\n    input clk,\n    input rst,\n    output reg [3:0] q\n);\n    always @(posedge clk or posedge rst) begin\n        if (rst)\n            q <= 4'b0000;\n        else\n            q <= q + 1;\n    end\nendmodule\n",
        ),
        (
            "Clock Divider (/2 & /4)",
            "module clk_div(\n    input clk,\n    input rst,\n    output reg [1:0] cnt,\n    output div2,\n    output div4\n);\n    always @(posedge clk or posedge rst) begin\n        if (rst)\n            cnt <= 2'b00;\n        else\n            cnt <= cnt + 1;\n    end\n    assign div2 = cnt[0];\n    assign div4 = cnt[1];\nendmodule\n",
        ),
        (
            "FSM (4-State Controller)",
            "module fsm_ctrl(\n    input clk,\n    input rst,\n    input start,\n    output reg [1:0] state,\n    output done\n);\n    // States: 00=IDLE, 01=LOAD, 10=RUN, 11=DONE\n    always @(posedge clk or posedge rst) begin\n        if (rst)\n            state <= 2'b00;\n        else begin\n            case (state)\n                2'b00: state <= start ? 2'b01 : 2'b00;\n                2'b01: state <= 2'b10;\n                2'b10: state <= 2'b11;\n                2'b11: state <= 2'b00;\n                default: state <= 2'b00;\n            endcase\n        end\n    end\n    assign done = (state == 2'b11);\nendmodule\n",
        ),
    ]
}

pub fn vhdl_templates() -> &'static [(&'static str, &'static str)] {
    &[
        (
            "Logic Gate (AND / XOR)",
            "library ieee;\nuse ieee.std_logic_1164.all;\n\nentity logic_gate is\n    port (\n        a     : in  std_logic;\n        b     : in  std_logic;\n        y_and : out std_logic;\n        y_xor : out std_logic\n    );\nend entity;\n\narchitecture rtl of logic_gate is\nbegin\n    y_and <= a and b;\n    y_xor <= a xor b;\nend architecture;\n",
        ),
        (
            "2:1 Multiplexer",
            "library ieee;\nuse ieee.std_logic_1164.all;\n\nentity mux2 is\n    port (\n        a   : in  std_logic;\n        b   : in  std_logic;\n        sel : in  std_logic;\n        y   : out std_logic\n    );\nend entity;\n\narchitecture rtl of mux2 is\nbegin\n    y <= b when sel = '1' else a;\nend architecture;\n",
        ),
        (
            "D Flip-Flop (Async Reset)",
            "library ieee;\nuse ieee.std_logic_1164.all;\n\nentity dff_rst is\n    port (\n        clk : in  std_logic;\n        rst : in  std_logic;\n        d   : in  std_logic;\n        q   : out std_logic\n    );\nend entity;\n\narchitecture rtl of dff_rst is\nbegin\n    process(clk, rst)\n    begin\n        if rst = '1' then\n            q <= '0';\n        elsif rising_edge(clk) then\n            q <= d;\n        end if;\n    end process;\nend architecture;\n",
        ),
        (
            "4-bit Counter",
            "library ieee;\nuse ieee.std_logic_1164.all;\nuse ieee.numeric_std.all;\n\nentity counter4 is\n    port (\n        clk : in  std_logic;\n        rst : in  std_logic;\n        q   : out std_logic_vector(3 downto 0)\n    );\nend entity;\n\narchitecture rtl of counter4 is\n    signal cnt : std_logic_vector(3 downto 0);\nbegin\n    process(clk, rst)\n    begin\n        if rst = '1' then\n            cnt <= \"0000\";\n        elsif rising_edge(clk) then\n            cnt <= cnt + 1;\n        end if;\n    end process;\n    q <= cnt;\nend architecture;\n",
        ),
        (
            "FSM (4-State Controller)",
            "library ieee;\nuse ieee.std_logic_1164.all;\n\nentity fsm_ctrl is\n    port (\n        clk   : in  std_logic;\n        rst   : in  std_logic;\n        start : in  std_logic;\n        state : out std_logic_vector(1 downto 0);\n        done  : out std_logic\n    );\nend entity;\n\narchitecture rtl of fsm_ctrl is\n    signal st : std_logic_vector(1 downto 0);\nbegin\n    -- States: \"00\"=IDLE, \"01\"=LOAD, \"10\"=RUN, \"11\"=DONE\n    process(clk, rst)\n    begin\n        if rst = '1' then\n            st <= \"00\";\n        elsif rising_edge(clk) then\n            case st is\n                when \"00\" =>\n                    if start = '1' then\n                        st <= \"01\";\n                    else\n                        st <= \"00\";\n                    end if;\n                when \"01\" =>\n                    st <= \"10\";\n                when \"10\" =>\n                    st <= \"11\";\n                when others =>\n                    st <= \"00\";\n            end case;\n        end if;\n    end process;\n    state <= st;\n    done  <= '1' when st = \"11\" else '0';\nend architecture;\n",
        ),
    ]
}

// ============================================================================
// Parsed HDL IR
// ============================================================================

#[derive(Debug, Clone)]
pub struct ParsedHdl {
    pub module_name: String,
    pub ports: Vec<HdlPort>,
    pub signals: HashMap<String, usize>, // signal name (lowercased for VHDL) -> width
    pub comb_assigns: Vec<CombAssign>,
    pub seq_blocks: Vec<SeqBlock>,
}

#[derive(Debug, Clone)]
pub struct CombAssign {
    pub target: LhsTarget,
    pub expr: Expr,
}

#[derive(Debug, Clone)]
pub struct SeqBlock {
    pub clk: String,
    pub clk_posedge: bool,
    pub async_rst: Option<(String, bool)>, // (signal, active_high)
    pub stmts: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub enum LhsTarget {
    Signal(String),
    Bit(String, usize),
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Assign {
        target: LhsTarget,
        expr: Expr,
    },
    If {
        cond: Expr,
        then_body: Vec<Stmt>,
        else_body: Vec<Stmt>,
    },
}

#[derive(Debug, Clone)]
pub enum Expr {
    Const { val: u64, width: usize },
    Ident(String),
    BitSelect(Box<Expr>, usize),
    Slice(Box<Expr>, usize, usize), // (expr, hi, lo)
    Concat(Vec<Expr>),
    Unary(UnaryOp, Box<Expr>),
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    BitNot,
    LogNot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    And,
    Or,
    Xor,
    Nand,
    Nor,
    Xnor,
    LogAnd,
    LogOr,
    Eq,
    Neq,
    Add,
    Sub,
}

pub fn parse_hdl(lang: HdlLanguage, source: &str) -> Result<ParsedHdl, String> {
    match lang {
        HdlLanguage::Verilog => parse_verilog(source),
        HdlLanguage::Vhdl => parse_vhdl(source),
    }
}

fn sanitize_spice_id(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "n".into()
    } else {
        out
    }
}

// ============================================================================
// Verilog Parser
// ============================================================================

fn strip_verilog_comments(src: &str) -> String {
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                if bytes[i] == b'\n' {
                    out.push('\n');
                }
                i += 1;
            }
            i = (i + 2).min(bytes.len());
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

fn parse_verilog(source: &str) -> Result<ParsedHdl, String> {
    let clean = strip_verilog_comments(source);
    let mod_pos = find_keyword(&clean, "module")
        .ok_or_else(|| "Verilog: missing 'module' declaration".to_string())?;
    let after_mod = clean[mod_pos + 6..].trim_start();

    let name_end = after_mod
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .unwrap_or(after_mod.len());
    let module_name = after_mod[..name_end].to_string();
    if module_name.is_empty() {
        return Err("Verilog: missing module name".into());
    }

    let rest = after_mod[name_end..].trim_start();
    // Skip optional #( parameter ... )
    let rest = if rest.starts_with('#') {
        let r = rest[1..].trim_start();
        if let Some(close) = find_matching_delim(r, '(', ')') {
            r[close + 1..].trim_start()
        } else {
            r
        }
    } else {
        rest
    };

    let (port_list_str, after_header) = if rest.starts_with('(') {
        let close = find_matching_delim(rest, '(', ')')
            .ok_or_else(|| "Verilog: unclosed '(' in module header".to_string())?;
        let semi = rest[close + 1..]
            .find(';')
            .ok_or_else(|| "Verilog: missing ';' after module header".to_string())?;
        (&rest[1..close], &rest[close + 1 + semi + 1..])
    } else {
        let semi = rest
            .find(';')
            .ok_or_else(|| "Verilog: missing ';' after module name".to_string())?;
        ("", &rest[semi + 1..])
    };

    let body = if let Some(end_pos) = find_keyword(after_header, "endmodule") {
        &after_header[..end_pos]
    } else {
        after_header
    };

    let mut ports: Vec<HdlPort> = Vec::new();
    let mut signals: HashMap<String, usize> = HashMap::new();
    let mut non_ansi_order: Vec<String> = Vec::new();

    // Parse port header (supports ANSI and non-ANSI)
    let mut cur_dir: Option<HdlPortDir> = None;
    let mut cur_range: (i32, i32) = (0, 0);
    for raw_item in split_top_level(port_list_str, ',') {
        let item = raw_item.trim();
        if item.is_empty() {
            continue;
        }
        if let Some((dir, msb, lsb, names)) = parse_verilog_port_decl(item, cur_dir, cur_range) {
            cur_dir = Some(dir);
            cur_range = (msb, lsb);
            let width = (msb - lsb).unsigned_abs() as usize + 1;
            for n in names {
                signals.insert(n.clone(), width);
                ports.push(HdlPort {
                    name: n,
                    dir,
                    width,
                    msb,
                    lsb,
                });
            }
        } else {
            let ident = item.trim().to_string();
            if !ident.is_empty() {
                non_ansi_order.push(ident);
            }
        }
    }

    let mut comb_assigns: Vec<CombAssign> = Vec::new();
    let mut seq_blocks: Vec<SeqBlock> = Vec::new();

    // Parse body statements
    let mut idx = 0usize;
    let body_bytes = body.as_bytes();
    while idx < body_bytes.len() {
        while idx < body_bytes.len() && body_bytes[idx].is_ascii_whitespace() {
            idx += 1;
        }
        if idx >= body_bytes.len() {
            break;
        }
        let rem = &body[idx..];

        // always block
        if starts_with_keyword(rem, "always") {
            let (consumed, parsed_always) = parse_verilog_always(rem)?;
            match parsed_always {
                AlwaysKind::Comb(stmts) => {
                    lower_comb_stmts_to_assigns(&stmts, &mut comb_assigns);
                }
                AlwaysKind::Seq(sb) => seq_blocks.push(sb),
            }
            idx += consumed;
            continue;
        }

        // Other statements end with ';'
        let Some(semi) = rem.find(';') else {
            break;
        };
        let stmt_str = rem[..semi].trim();
        idx += semi + 1;
        if stmt_str.is_empty() {
            continue;
        }

        if starts_with_keyword(stmt_str, "input")
            || starts_with_keyword(stmt_str, "output")
            || starts_with_keyword(stmt_str, "inout")
        {
            if let Some((dir, msb, lsb, names)) = parse_verilog_port_decl(stmt_str, None, (0, 0)) {
                let width = (msb - lsb).unsigned_abs() as usize + 1;
                for n in names {
                    signals.insert(n.clone(), width);
                    if let Some(p) = ports.iter_mut().find(|p| p.name == n) {
                        p.dir = dir;
                        p.width = width;
                        p.msb = msb;
                        p.lsb = lsb;
                    } else {
                        ports.push(HdlPort {
                            name: n,
                            dir,
                            width,
                            msb,
                            lsb,
                        });
                    }
                }
            }
        } else if starts_with_keyword(stmt_str, "wire") || starts_with_keyword(stmt_str, "reg") {
            let after_kw = stmt_str
                .split_once(char::is_whitespace)
                .map(|x| x.1.trim())
                .unwrap_or("");
            let (msb, lsb, rest_decl) = parse_verilog_range(after_kw);
            let width = (msb - lsb).unsigned_abs() as usize + 1;
            for part in split_top_level(rest_decl, ',') {
                let p = part.trim();
                if let Some((lhs_s, rhs_s)) = p.split_once('=') {
                    let name = lhs_s.trim().to_string();
                    if !name.is_empty() {
                        signals.insert(name.clone(), width);
                        let expr = parse_expr(rhs_s.trim(), HdlLanguage::Verilog)?;
                        comb_assigns.push(CombAssign {
                            target: LhsTarget::Signal(name),
                            expr,
                        });
                    }
                } else if !p.is_empty() {
                    signals.insert(p.to_string(), width);
                }
            }
        } else if starts_with_keyword(stmt_str, "assign") {
            let after = stmt_str[6..].trim();
            if let Some((lhs_s, rhs_s)) = after.split_once('=') {
                let target = parse_lhs_target(lhs_s.trim())?;
                let expr = parse_expr(rhs_s.trim(), HdlLanguage::Verilog)?;
                comb_assigns.push(CombAssign { target, expr });
            }
        } else if let Some(gate_assign) = try_parse_verilog_gate(stmt_str)? {
            comb_assigns.push(gate_assign);
        }
    }

    // Reorder non-ANSI ports to match header order
    if !non_ansi_order.is_empty() {
        let mut ordered = Vec::new();
        for name in &non_ansi_order {
            if let Some(p) = ports.iter().find(|p| &p.name == name) {
                ordered.push(p.clone());
            }
        }
        if ordered.len() == ports.len() {
            ports = ordered;
        }
    }

    if ports.is_empty() {
        return Err("Verilog: no input/output ports found in module".into());
    }

    Ok(ParsedHdl {
        module_name,
        ports,
        signals,
        comb_assigns,
        seq_blocks,
    })
}

fn parse_verilog_port_decl(
    s: &str,
    prev_dir: Option<HdlPortDir>,
    prev_range: (i32, i32),
) -> Option<(HdlPortDir, i32, i32, Vec<String>)> {
    let mut rem = s.trim();
    let dir = if starts_with_keyword(rem, "input") {
        rem = rem[5..].trim_start();
        HdlPortDir::Input
    } else if starts_with_keyword(rem, "output") {
        rem = rem[6..].trim_start();
        HdlPortDir::Output
    } else if starts_with_keyword(rem, "inout") {
        rem = rem[5..].trim_start();
        HdlPortDir::Inout
    } else {
        let d = prev_dir?;
        let (msb, lsb, rest) = parse_verilog_range(rem);
        let (msb, lsb) = if rest.len() != rem.len() {
            (msb, lsb)
        } else {
            prev_range
        };
        let names: Vec<String> = rest
            .split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect();
        return if names.is_empty() {
            None
        } else {
            Some((d, msb, lsb, names))
        };
    };

    // Optional wire / reg / logic / signed qualifiers
    loop {
        if starts_with_keyword(rem, "wire") {
            rem = rem[4..].trim_start();
        } else if starts_with_keyword(rem, "reg") {
            rem = rem[3..].trim_start();
        } else if starts_with_keyword(rem, "logic") {
            rem = rem[5..].trim_start();
        } else if starts_with_keyword(rem, "signed") {
            rem = rem[6..].trim_start();
        } else {
            break;
        }
    }

    let (msb, lsb, rest) = parse_verilog_range(rem);
    let names: Vec<String> = rest
        .split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect();
    if names.is_empty() {
        None
    } else {
        Some((dir, msb, lsb, names))
    }
}

fn parse_verilog_range(s: &str) -> (i32, i32, &str) {
    let s = s.trim_start();
    if s.starts_with('[') {
        if let Some(close) = s.find(']') {
            let inside = &s[1..close];
            if let Some((a, b)) = inside.split_once(':') {
                let msb = a.trim().parse::<i32>().unwrap_or(0);
                let lsb = b.trim().parse::<i32>().unwrap_or(0);
                return (msb, lsb, s[close + 1..].trim_start());
            }
        }
    }
    (0, 0, s)
}

fn try_parse_verilog_gate(stmt: &str) -> Result<Option<CombAssign>, String> {
    let gate_ops = [
        ("and", BinaryOp::And),
        ("or", BinaryOp::Or),
        ("nand", BinaryOp::Nand),
        ("nor", BinaryOp::Nor),
        ("xor", BinaryOp::Xor),
        ("xnor", BinaryOp::Xnor),
    ];
    let Some(open) = stmt.find('(') else {
        return Ok(None);
    };
    let Some(close) = stmt.rfind(')') else {
        return Ok(None);
    };
    let head = stmt[..open].trim();
    let gate_kw = head.split_whitespace().next().unwrap_or("");
    let args: Vec<&str> = split_top_level(&stmt[open + 1..close], ',')
        .into_iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if args.len() < 2 {
        return Ok(None);
    }

    if gate_kw == "not" {
        let target = parse_lhs_target(args[0])?;
        let inp = parse_expr(args[1], HdlLanguage::Verilog)?;
        return Ok(Some(CombAssign {
            target,
            expr: Expr::Unary(UnaryOp::BitNot, Box::new(inp)),
        }));
    }
    if gate_kw == "buf" {
        let target = parse_lhs_target(args[0])?;
        let inp = parse_expr(args[1], HdlLanguage::Verilog)?;
        return Ok(Some(CombAssign { target, expr: inp }));
    }

    for (kw, op) in gate_ops {
        if gate_kw == kw && args.len() >= 3 {
            let target = parse_lhs_target(args[0])?;
            let mut acc = parse_expr(args[1], HdlLanguage::Verilog)?;
            for a in &args[2..] {
                let rhs = parse_expr(a, HdlLanguage::Verilog)?;
                acc = Expr::Binary(op, Box::new(acc), Box::new(rhs));
            }
            return Ok(Some(CombAssign { target, expr: acc }));
        }
    }
    Ok(None)
}

enum AlwaysKind {
    Comb(Vec<Stmt>),
    Seq(SeqBlock),
}

fn parse_verilog_always(rem: &str) -> Result<(usize, AlwaysKind), String> {
    // rem starts with "always"
    let mut pos = 6;
    let bytes = rem.as_bytes();
    while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
        pos += 1;
    }
    let mut clk: Option<(String, bool)> = None;
    let mut async_rst: Option<(String, bool)> = None;

    if pos < bytes.len() && bytes[pos] == b'@' {
        pos += 1;
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if pos < bytes.len() && bytes[pos] == b'*' {
            pos += 1;
        } else if pos < bytes.len() && bytes[pos] == b'(' {
            let close = find_matching_delim(&rem[pos..], '(', ')')
                .ok_or_else(|| "Verilog: unclosed sensitivity list".to_string())?;
            let sens = &rem[pos + 1..pos + close];
            pos += close + 1;
            // Parse posedge / negedge items
            let normalized = sens.replace(" or ", ",");
            for part in normalized.split(',') {
                let p = part.trim();
                if starts_with_keyword(p, "posedge") {
                    let sig = p[7..].trim().to_string();
                    if clk.is_none() {
                        clk = Some((sig, true));
                    } else {
                        async_rst = Some((sig, true));
                    }
                } else if starts_with_keyword(p, "negedge") {
                    let sig = p[7..].trim().to_string();
                    if clk.is_none() {
                        clk = Some((sig, false));
                    } else {
                        async_rst = Some((sig, false));
                    }
                }
            }
        }
    }

    let (body_consumed, stmts) = parse_verilog_stmt_block(&rem[pos..])?;
    pos += body_consumed;

    if let Some((clk_sig, clk_posedge)) = clk {
        Ok((
            pos,
            AlwaysKind::Seq(SeqBlock {
                clk: clk_sig,
                clk_posedge,
                async_rst,
                stmts,
            }),
        ))
    } else {
        Ok((pos, AlwaysKind::Comb(stmts)))
    }
}

fn parse_verilog_stmt_block(s: &str) -> Result<(usize, Vec<Stmt>), String> {
    let mut pos = 0;
    let bytes = s.as_bytes();
    while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
        pos += 1;
    }
    if pos >= bytes.len() {
        return Ok((pos, Vec::new()));
    }
    if starts_with_keyword(&s[pos..], "begin") {
        pos += 5;
        let mut stmts = Vec::new();
        loop {
            while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
                pos += 1;
            }
            if pos >= bytes.len() {
                break;
            }
            if starts_with_keyword(&s[pos..], "end") {
                pos += 3;
                break;
            }
            let (c, mut sub) = parse_verilog_single_stmt(&s[pos..])?;
            if c == 0 {
                break;
            }
            pos += c;
            stmts.append(&mut sub);
        }
        Ok((pos, stmts))
    } else {
        parse_verilog_single_stmt(s)
    }
}

fn parse_verilog_single_stmt(s: &str) -> Result<(usize, Vec<Stmt>), String> {
    let mut pos = 0;
    let bytes = s.as_bytes();
    while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
        pos += 1;
    }
    if pos >= bytes.len() {
        return Ok((pos, Vec::new()));
    }
    let rem = &s[pos..];
    if starts_with_keyword(rem, "if") {
        pos += 2;
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if pos >= bytes.len() || bytes[pos] != b'(' {
            return Err("Verilog: expected '(' after 'if'".into());
        }
        let close = find_matching_delim(&s[pos..], '(', ')')
            .ok_or_else(|| "Verilog: unclosed '(' in if condition".to_string())?;
        let cond_str = &s[pos + 1..pos + close];
        let cond = parse_expr(cond_str.trim(), HdlLanguage::Verilog)?;
        pos += close + 1;

        let (c_then, then_body) = parse_verilog_stmt_block(&s[pos..])?;
        pos += c_then;

        let mut look = pos;
        while look < bytes.len() && bytes[look].is_ascii_whitespace() {
            look += 1;
        }
        let else_body = if look < bytes.len() && starts_with_keyword(&s[look..], "else") {
            pos = look + 4;
            let (c_else, eb) = parse_verilog_stmt_block(&s[pos..])?;
            pos += c_else;
            eb
        } else {
            Vec::new()
        };

        return Ok((
            pos,
            vec![Stmt::If {
                cond,
                then_body,
                else_body,
            }],
        ));
    }

    if starts_with_keyword(rem, "case") {
        pos += 4;
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if pos >= bytes.len() || bytes[pos] != b'(' {
            return Err("Verilog: expected '(' after 'case'".into());
        }
        let close = find_matching_delim(&s[pos..], '(', ')')
            .ok_or_else(|| "Verilog: unclosed '(' in case expression".to_string())?;
        let scrutinee = parse_expr(s[pos + 1..pos + close].trim(), HdlLanguage::Verilog)?;
        pos += close + 1;

        let mut arms: Vec<(Option<Expr>, Vec<Stmt>)> = Vec::new();
        loop {
            while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
                pos += 1;
            }
            if pos >= bytes.len() {
                break;
            }
            if starts_with_keyword(&s[pos..], "endcase") {
                pos += 7;
                break;
            }
            let Some(colon_rel) = s[pos..].find(':') else {
                break;
            };
            let pat_str = s[pos..pos + colon_rel].trim();
            pos += colon_rel + 1;
            let (c_arm, arm_stmts) = parse_verilog_stmt_block(&s[pos..])?;
            pos += c_arm;
            if pat_str == "default" {
                arms.push((None, arm_stmts));
            } else {
                let pat_expr = parse_expr(pat_str, HdlLanguage::Verilog)?;
                arms.push((Some(pat_expr), arm_stmts));
            }
        }

        // Lower case arms into nested if/else
        let mut acc_else: Vec<Stmt> = Vec::new();
        for (pat_opt, body) in arms.into_iter().rev() {
            if let Some(pat) = pat_opt {
                let cond = Expr::Binary(BinaryOp::Eq, Box::new(scrutinee.clone()), Box::new(pat));
                acc_else = vec![Stmt::If {
                    cond,
                    then_body: body,
                    else_body: acc_else,
                }];
            } else {
                acc_else = body;
            }
        }
        return Ok((pos, acc_else));
    }

    // Simple assignment ending with ';'
    let Some(semi) = rem.find(';') else {
        return Ok((s.len(), Vec::new()));
    };
    let line = rem[..semi].trim();
    pos += semi + 1;
    if let Some((lhs, rhs)) = split_assignment(line, HdlLanguage::Verilog) {
        let target = parse_lhs_target(lhs.trim())?;
        let expr = parse_expr(rhs.trim(), HdlLanguage::Verilog)?;
        Ok((pos, vec![Stmt::Assign { target, expr }]))
    } else {
        Ok((pos, Vec::new()))
    }
}

// ============================================================================
// VHDL Parser
// ============================================================================

fn strip_vhdl_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    for line in src.lines() {
        if let Some(pos) = line.find("--") {
            out.push_str(&line[..pos]);
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

fn parse_vhdl(source: &str) -> Result<ParsedHdl, String> {
    let clean = strip_vhdl_comments(source);
    let low = clean.to_ascii_lowercase();

    let ent_pos = find_keyword(&low, "entity")
        .ok_or_else(|| "VHDL: missing 'entity' declaration".to_string())?;
    let after_ent = clean[ent_pos + 6..].trim_start();
    let name_end = after_ent
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .unwrap_or(after_ent.len());
    let module_name = after_ent[..name_end].to_string();
    if module_name.is_empty() {
        return Err("VHDL: missing entity name".into());
    }

    // Find port (...) inside entity
    let ent_low = low[ent_pos..].to_string();
    let port_pos = find_keyword(&ent_low, "port")
        .ok_or_else(|| "VHDL: missing 'port (...)' in entity".to_string())?;
    let after_port = clean[ent_pos + port_pos + 4..].trim_start();
    let open = after_port
        .find('(')
        .ok_or_else(|| "VHDL: missing '(' after 'port'".to_string())?;
    let close = find_matching_delim(&after_port[open..], '(', ')')
        .ok_or_else(|| "VHDL: unclosed '(' in port list".to_string())?;
    let port_body = &after_port[open + 1..open + close];

    let mut ports = Vec::new();
    let mut signals: HashMap<String, usize> = HashMap::new();

    for entry in port_body.split(';') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let Some((lhs, rhs)) = entry.split_once(':') else {
            continue;
        };
        let rhs_trim = rhs.trim();
        let rhs_low = rhs_trim.to_ascii_lowercase();
        let (dir, type_str) = if starts_with_keyword(&rhs_low, "inout") {
            (HdlPortDir::Inout, rhs_trim[5..].trim())
        } else if starts_with_keyword(&rhs_low, "in") {
            (HdlPortDir::Input, rhs_trim[2..].trim())
        } else if starts_with_keyword(&rhs_low, "out") {
            (HdlPortDir::Output, rhs_trim[3..].trim())
        } else if starts_with_keyword(&rhs_low, "buffer") {
            (HdlPortDir::Output, rhs_trim[6..].trim())
        } else {
            continue;
        };
        let (msb, lsb) = parse_vhdl_type_range(type_str);
        let width = (msb - lsb).unsigned_abs() as usize + 1;
        for name in lhs.split(',') {
            let n = name.trim().to_string();
            if !n.is_empty() {
                signals.insert(n.to_ascii_lowercase(), width);
                ports.push(HdlPort {
                    name: n,
                    dir,
                    width,
                    msb,
                    lsb,
                });
            }
        }
    }

    if ports.is_empty() {
        return Err("VHDL: no ports parsed from entity".into());
    }

    let mut comb_assigns = Vec::new();
    let mut seq_blocks = Vec::new();

    // Parse architecture declarations & body
    if let Some(arch_pos) = find_keyword(&low, "architecture") {
        let arch_str = &clean[arch_pos..];
        let arch_low = &low[arch_pos..];
        if let Some(begin_pos) = find_keyword(arch_low, "begin") {
            // Parse signal declarations between `is` and `begin`
            let decls_start = find_keyword(&arch_low[..begin_pos], "is")
                .map(|p| p + 2)
                .unwrap_or(0);
            let decls = &arch_str[decls_start..begin_pos];
            for stmt in decls.split(';') {
                let st = stmt.trim();
                let st_low = st.to_ascii_lowercase();
                if starts_with_keyword(&st_low, "signal") {
                    if let Some((lhs, rhs)) = st[6..].split_once(':') {
                        let (msb, lsb) = parse_vhdl_type_range(rhs.trim());
                        let width = (msb - lsb).unsigned_abs() as usize + 1;
                        for n in lhs.split(',') {
                            let nm = n.trim().to_ascii_lowercase();
                            if !nm.is_empty() {
                                signals.insert(nm, width);
                            }
                        }
                    }
                }
            }

            // Parse architecture body between begin and end architecture
            let body = &arch_str[begin_pos + 5..];
            parse_vhdl_arch_body(body, &mut comb_assigns, &mut seq_blocks)?;
        }
    }

    Ok(ParsedHdl {
        module_name,
        ports,
        signals,
        comb_assigns,
        seq_blocks,
    })
}

fn parse_vhdl_type_range(type_str: &str) -> (i32, i32) {
    let low = type_str.to_ascii_lowercase();
    if let (Some(open), Some(close)) = (low.find('('), low.rfind(')')) {
        let inside = &low[open + 1..close];
        if let Some((a, b)) = inside.split_once("downto") {
            let msb = a.trim().parse::<i32>().unwrap_or(0);
            let lsb = b.trim().parse::<i32>().unwrap_or(0);
            return (msb, lsb);
        }
        if let Some((a, b)) = inside.split_once("to") {
            let lsb = a.trim().parse::<i32>().unwrap_or(0);
            let msb = b.trim().parse::<i32>().unwrap_or(0);
            return (msb, lsb);
        }
    }
    (0, 0)
}

fn parse_vhdl_arch_body(
    body: &str,
    comb_assigns: &mut Vec<CombAssign>,
    seq_blocks: &mut Vec<SeqBlock>,
) -> Result<(), String> {
    let mut pos = 0usize;
    let bytes = body.as_bytes();
    while pos < bytes.len() {
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if pos >= bytes.len() {
            break;
        }
        let rem = &body[pos..];
        let rem_low = rem.to_ascii_lowercase();

        if starts_with_keyword(&rem_low, "end") {
            break;
        }

        // Optional label: `proc_lbl : process(...)`
        let proc_offset = if starts_with_keyword(&rem_low, "process") {
            Some(0)
        } else if let Some(colon) = rem.find(':') {
            let before = rem[..colon].trim();
            if !before.is_empty() && before.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                let after = rem[colon + 1..].trim_start();
                if starts_with_keyword(&after.to_ascii_lowercase(), "process") {
                    Some(rem.len() - after.len())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some(poff) = proc_offset {
            let proc_rem = &rem[poff..];
            let proc_low = proc_rem.to_ascii_lowercase();
            let end_proc = find_vhdl_end_process(&proc_low)
                .ok_or_else(|| "VHDL: missing 'end process;'".to_string())?;
            let proc_src = &proc_rem[..end_proc.0];
            parse_vhdl_process(proc_src, comb_assigns, seq_blocks)?;
            pos += poff + end_proc.1;
            continue;
        }

        let Some(semi) = rem.find(';') else {
            break;
        };
        let stmt = rem[..semi].trim();
        pos += semi + 1;
        if let Some((lhs, rhs)) = stmt.split_once("<=") {
            let target = parse_lhs_target(&lhs.trim().to_ascii_lowercase())?;
            let expr = parse_vhdl_when_else(rhs.trim())?;
            comb_assigns.push(CombAssign { target, expr });
        }
    }
    Ok(())
}

fn find_vhdl_end_process(low: &str) -> Option<(usize, usize)> {
    let mut search = 0;
    while let Some(e) = find_keyword(&low[search..], "end") {
        let abs = search + e;
        let after = low[abs + 3..].trim_start();
        if starts_with_keyword(after, "process") {
            let semi_rel = after.find(';')?;
            let consumed = (low.len() - after.len()) + semi_rel + 1;
            return Some((abs, consumed));
        }
        search = abs + 3;
    }
    None
}

fn parse_vhdl_when_else(rhs: &str) -> Result<Expr, String> {
    let low = rhs.to_ascii_lowercase();
    if let Some(w_pos) = find_keyword(&low, "when") {
        let val_str = rhs[..w_pos].trim();
        let after_when = &rhs[w_pos + 4..];
        let after_low = &low[w_pos + 4..];
        if let Some(e_pos) = find_keyword(after_low, "else") {
            let cond_str = after_when[..e_pos].trim();
            let else_str = after_when[e_pos + 4..].trim();
            let val_expr = parse_expr(val_str, HdlLanguage::Vhdl)?;
            let cond_expr = parse_expr(cond_str, HdlLanguage::Vhdl)?;
            let else_expr = parse_vhdl_when_else(else_str)?;
            return Ok(Expr::Ternary(
                Box::new(cond_expr),
                Box::new(val_expr),
                Box::new(else_expr),
            ));
        }
    }
    parse_expr(rhs, HdlLanguage::Vhdl)
}

fn parse_vhdl_process(
    proc_src: &str,
    comb_assigns: &mut Vec<CombAssign>,
    seq_blocks: &mut Vec<SeqBlock>,
) -> Result<(), String> {
    let low = proc_src.to_ascii_lowercase();
    let begin_pos = find_keyword(&low, "begin")
        .ok_or_else(|| "VHDL: missing 'begin' in process".to_string())?;
    let body = &proc_src[begin_pos + 5..];

    // Parse top-level if/elsif/else structure in process
    let (stmts, clk_info, async_rst) = parse_vhdl_stmts(body)?;
    if let Some((clk, posedge)) = clk_info {
        seq_blocks.push(SeqBlock {
            clk,
            clk_posedge: posedge,
            async_rst,
            stmts,
        });
    } else {
        lower_comb_stmts_to_assigns(&stmts, comb_assigns);
    }
    Ok(())
}

fn parse_vhdl_stmts(
    src: &str,
) -> Result<(Vec<Stmt>, Option<(String, bool)>, Option<(String, bool)>), String> {
    let mut pos = 0usize;
    let bytes = src.as_bytes();
    let mut stmts = Vec::new();
    let mut clk_info: Option<(String, bool)> = None;
    let mut async_rst: Option<(String, bool)> = None;

    while pos < bytes.len() {
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if pos >= bytes.len() {
            break;
        }
        let rem = &src[pos..];
        let rem_low = rem.to_ascii_lowercase();

        if starts_with_keyword(&rem_low, "elsif")
            || starts_with_keyword(&rem_low, "else")
            || starts_with_keyword(&rem_low, "end")
        {
            break;
        }

        if starts_with_keyword(&rem_low, "if") {
            let (consumed, if_stmt, ci, ar) = parse_vhdl_if(rem)?;
            pos += consumed;
            if ci.is_some() {
                clk_info = ci;
            }
            if ar.is_some() {
                async_rst = ar;
            }
            if let Some(st) = if_stmt {
                stmts.push(st);
            }
            continue;
        }

        let Some(semi) = rem.find(';') else {
            break;
        };
        let line = rem[..semi].trim();
        pos += semi + 1;
        if let Some((lhs, rhs)) = line.split_once("<=").or_else(|| line.split_once(":=")) {
            let target = parse_lhs_target(&lhs.trim().to_ascii_lowercase())?;
            let expr = parse_expr(rhs.trim(), HdlLanguage::Vhdl)?;
            stmts.push(Stmt::Assign { target, expr });
        }
    }

    Ok((stmts, clk_info, async_rst))
}

fn detect_vhdl_clock_edge(cond: &str) -> Option<(String, bool)> {
    let low = cond.trim().to_ascii_lowercase();
    if let Some(rest) = low.strip_prefix("rising_edge") {
        let r = rest.trim();
        if let Some(inner) = r.strip_prefix('(').and_then(|x| x.strip_suffix(')')) {
            return Some((inner.trim().to_string(), true));
        }
    }
    if let Some(rest) = low.strip_prefix("falling_edge") {
        let r = rest.trim();
        if let Some(inner) = r.strip_prefix('(').and_then(|x| x.strip_suffix(')')) {
            return Some((inner.trim().to_string(), false));
        }
    }
    if low.contains("'event") {
        if let Some(tick) = low.find("'event") {
            let sig = low[..tick]
                .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .filter(|s| !s.is_empty())
                .last()?
                .to_string();
            let posedge = !low.contains("'0'");
            return Some((sig, posedge));
        }
    }
    None
}

fn detect_vhdl_reset_cond(cond: &str) -> Option<(String, bool)> {
    let low = cond.trim().to_ascii_lowercase();
    if let Some((l, r)) = low.split_once('=') {
        let sig = l.trim().to_string();
        let val = r.trim();
        if sig.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            if val == "'1'" || val == "1" {
                return Some((sig, true));
            } else if val == "'0'" || val == "0" {
                return Some((sig, false));
            }
        }
    }
    None
}

fn parse_vhdl_if(
    src: &str,
) -> Result<(usize, Option<Stmt>, Option<(String, bool)>, Option<(String, bool)>), String> {
    // Parse `if <c0> then <b0> [elsif <c1> then <b1>]* [else <be>] end if;`
    let mut branches: Vec<(Option<String>, Vec<Stmt>)> = Vec::new();
    let mut pos = 0usize;
    let mut clk_info: Option<(String, bool)> = None;
    let mut async_rst: Option<(String, bool)> = None;

    loop {
        let rem = &src[pos..];
        let rem_low = rem.to_ascii_lowercase();
        let (is_else, kw_len) = if starts_with_keyword(&rem_low, "if") && branches.is_empty() {
            (false, 2)
        } else if starts_with_keyword(&rem_low, "elsif") {
            (false, 5)
        } else if starts_with_keyword(&rem_low, "else") {
            (true, 4)
        } else {
            break;
        };
        pos += kw_len;

        let cond_str = if !is_else {
            let after = &src[pos..];
            let after_low = after.to_ascii_lowercase();
            let then_pos = find_keyword(&after_low, "then")
                .ok_or_else(|| "VHDL: missing 'then' in if statement".to_string())?;
            let c = after[..then_pos].trim().to_string();
            pos += then_pos + 4;
            Some(c)
        } else {
            None
        };

        // Parse statements until next elsif / else / end if
        let mut body_stmts = Vec::new();
        loop {
            while pos < src.len() && src.as_bytes()[pos].is_ascii_whitespace() {
                pos += 1;
            }
            if pos >= src.len() {
                break;
            }
            let r = &src[pos..];
            let r_low = r.to_ascii_lowercase();
            if starts_with_keyword(&r_low, "elsif")
                || starts_with_keyword(&r_low, "else")
                || starts_with_keyword(&r_low, "end")
            {
                break;
            }
            if starts_with_keyword(&r_low, "if") {
                let (c, nested, ci, ar) = parse_vhdl_if(r)?;
                pos += c;
                if ci.is_some() {
                    clk_info = ci;
                }
                if ar.is_some() {
                    async_rst = ar;
                }
                if let Some(st) = nested {
                    body_stmts.push(st);
                }
            } else if starts_with_keyword(&r_low, "case") {
                let (c, nested) = parse_vhdl_case(r)?;
                pos += c;
                if let Some(st) = nested {
                    body_stmts.push(st);
                }
            } else {
                let Some(semi) = r.find(';') else {
                    pos = src.len();
                    break;
                };
                let line = r[..semi].trim();
                pos += semi + 1;
                if let Some((lhs, rhs)) = line.split_once("<=").or_else(|| line.split_once(":=")) {
                    let target = parse_lhs_target(&lhs.trim().to_ascii_lowercase())?;
                    let expr = parse_expr(rhs.trim(), HdlLanguage::Vhdl)?;
                    body_stmts.push(Stmt::Assign { target, expr });
                }
            }
        }

        branches.push((cond_str, body_stmts));
    }

    // Consume `end if;`
    while pos < src.len() && src.as_bytes()[pos].is_ascii_whitespace() {
        pos += 1;
    }
    let rem_low = src[pos..].to_ascii_lowercase();
    if starts_with_keyword(&rem_low, "end") {
        if let Some(semi) = rem_low.find(';') {
            pos += semi + 1;
        }
    }

    // Check for standard VHDL clocked pattern:
    // Case 1: `if rising_edge(clk) then ... end if;`
    if branches.len() == 1 {
        if let Some(c) = &branches[0].0 {
            if let Some(ci) = detect_vhdl_clock_edge(c) {
                clk_info = Some(ci);
                // Return inner statements directly wrapped in a dummy true or unwrap
                if branches[0].1.len() == 1 {
                    return Ok((pos, Some(branches[0].1.remove(0)), clk_info, async_rst));
                }
                return Ok((
                    pos,
                    Some(Stmt::If {
                        cond: Expr::Const { val: 1, width: 1 },
                        then_body: branches[0].1.clone(),
                        else_body: Vec::new(),
                    }),
                    clk_info,
                    async_rst,
                ));
            }
        }
    }
    // Case 2: `if rst = '1' then <reset_body> elsif rising_edge(clk) then <clk_body> end if;`
    if branches.len() == 2 {
        if let (Some(c0), Some(c1)) = (&branches[0].0, &branches[1].0) {
            if let Some(ci) = detect_vhdl_clock_edge(c1) {
                clk_info = Some(ci);
                async_rst = detect_vhdl_reset_cond(c0);
                let cond_expr = parse_expr(c0, HdlLanguage::Vhdl)?;
                return Ok((
                    pos,
                    Some(Stmt::If {
                        cond: cond_expr,
                        then_body: branches[0].1.clone(),
                        else_body: branches[1].1.clone(),
                    }),
                    clk_info,
                    async_rst,
                ));
            }
        }
    }

    // Regular if / elsif / else chain
    let mut acc_else: Vec<Stmt> = Vec::new();
    for (cond_opt, body) in branches.into_iter().rev() {
        if let Some(c) = cond_opt {
            let cond_expr = parse_expr(&c, HdlLanguage::Vhdl)?;
            acc_else = vec![Stmt::If {
                cond: cond_expr,
                then_body: body,
                else_body: acc_else,
            }];
        } else {
            acc_else = body;
        }
    }

    Ok((pos, acc_else.into_iter().next(), clk_info, async_rst))
}

fn parse_vhdl_case(src: &str) -> Result<(usize, Option<Stmt>), String> {
    // Parse `case <expr> is [when <pat> => <stmts>]* end case;`
    let low = src.to_ascii_lowercase();
    let is_pos = find_keyword(&low[4..], "is")
        .ok_or_else(|| "VHDL: missing 'is' in case statement".to_string())?;
    let scrutinee = parse_expr(src[4..4 + is_pos].trim(), HdlLanguage::Vhdl)?;
    let mut pos = 4 + is_pos + 2;

    let mut arms: Vec<(Option<Expr>, Vec<Stmt>)> = Vec::new();
    loop {
        while pos < src.len() && src.as_bytes()[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if pos >= src.len() {
            break;
        }
        let r = &src[pos..];
        let r_low = r.to_ascii_lowercase();
        if starts_with_keyword(&r_low, "end") {
            if let Some(semi) = r_low.find(';') {
                pos += semi + 1;
            }
            break;
        }
        if !starts_with_keyword(&r_low, "when") {
            break;
        }
        pos += 4;
        let Some(arrow_rel) = src[pos..].find("=>") else {
            return Err("VHDL: missing '=>' in when clause".into());
        };
        let pat_str = src[pos..pos + arrow_rel].trim();
        pos += arrow_rel + 2;

        let mut body_stmts = Vec::new();
        loop {
            while pos < src.len() && src.as_bytes()[pos].is_ascii_whitespace() {
                pos += 1;
            }
            if pos >= src.len() {
                break;
            }
            let rr = &src[pos..];
            let rr_low = rr.to_ascii_lowercase();
            if starts_with_keyword(&rr_low, "when") || starts_with_keyword(&rr_low, "end") {
                break;
            }
            if starts_with_keyword(&rr_low, "if") {
                let (c, nested, _, _) = parse_vhdl_if(rr)?;
                pos += c;
                if let Some(st) = nested {
                    body_stmts.push(st);
                }
            } else if starts_with_keyword(&rr_low, "case") {
                let (c, nested) = parse_vhdl_case(rr)?;
                pos += c;
                if let Some(st) = nested {
                    body_stmts.push(st);
                }
            } else {
                let Some(semi) = rr.find(';') else {
                    pos = src.len();
                    break;
                };
                let line = rr[..semi].trim();
                pos += semi + 1;
                if let Some((lhs, rhs)) = line.split_once("<=").or_else(|| line.split_once(":=")) {
                    let target = parse_lhs_target(&lhs.trim().to_ascii_lowercase())?;
                    let expr = parse_expr(rhs.trim(), HdlLanguage::Vhdl)?;
                    body_stmts.push(Stmt::Assign { target, expr });
                }
            }
        }

        if pat_str.eq_ignore_ascii_case("others") {
            arms.push((None, body_stmts));
        } else {
            let pat_expr = parse_expr(pat_str, HdlLanguage::Vhdl)?;
            arms.push((Some(pat_expr), body_stmts));
        }
    }

    let mut acc_else: Vec<Stmt> = Vec::new();
    for (pat_opt, body) in arms.into_iter().rev() {
        if let Some(pat) = pat_opt {
            let cond = Expr::Binary(BinaryOp::Eq, Box::new(scrutinee.clone()), Box::new(pat));
            acc_else = vec![Stmt::If {
                cond,
                then_body: body,
                else_body: acc_else,
            }];
        } else {
            acc_else = body;
        }
    }
    Ok((pos, acc_else.into_iter().next()))
}

// ============================================================================
// Expression Parser (Shared Verilog & VHDL)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    Ident(String),
    Num(u64, usize), // (value, width)
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Colon,
    Comma,
    Question,
    Tilde,
    Bang,
    Amp,
    AmpAmp,
    Pipe,
    PipePipe,
    Caret,
    TildeAmp,
    TildePipe,
    TildeCaret,
    EqEq,
    BangEq,
    Plus,
    Minus,
}

fn tokenize_expr(s: &str, lang: HdlLanguage) -> Result<Vec<Tok>, String> {
    let mut toks = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let ch = bytes[i] as char;
        if ch.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        // VHDL single-bit literal '0' / '1'
        if lang == HdlLanguage::Vhdl && ch == '\'' && i + 2 < bytes.len() && bytes[i + 2] == b'\'' {
            let v = if bytes[i + 1] == b'1' { 1 } else { 0 };
            toks.push(Tok::Num(v, 1));
            i += 3;
            continue;
        }
        // VHDL bit-vector literal "1010"
        if lang == HdlLanguage::Vhdl && ch == '"' {
            i += 1;
            let start = i;
            while i < bytes.len() && bytes[i] != b'"' {
                i += 1;
            }
            let bits = &s[start..i];
            if i < bytes.len() {
                i += 1;
            }
            let val = u64::from_str_radix(&bits.replace('_', ""), 2).unwrap_or(0);
            toks.push(Tok::Num(val, bits.len().max(1)));
            continue;
        }
        // Numbers (including Verilog sized literals like 4'b1010, 8'hFF, 1'b0)
        if ch.is_ascii_digit() {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'\'' || bytes[i] == b'_') {
                i += 1;
            }
            let raw = &s[start..i];
            toks.push(parse_number_token(raw)?);
            continue;
        }
        // Identifiers & VHDL keyword operators
        if ch.is_ascii_alphabetic() || ch == '_' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let word = &s[start..i];
            if lang == HdlLanguage::Vhdl {
                match word.to_ascii_lowercase().as_str() {
                    "and" => toks.push(Tok::Amp),
                    "or" => toks.push(Tok::Pipe),
                    "xor" => toks.push(Tok::Caret),
                    "nand" => toks.push(Tok::TildeAmp),
                    "nor" => toks.push(Tok::TildePipe),
                    "xnor" => toks.push(Tok::TildeCaret),
                    "not" => toks.push(Tok::Tilde),
                    // Strip common numeric_std conversion wrappers
                    "unsigned" | "signed" | "std_logic_vector" => {}
                    other => toks.push(Tok::Ident(other.to_string())),
                }
            } else {
                toks.push(Tok::Ident(word.to_string()));
            }
            continue;
        }
        // Multi-char operators
        let two = if i + 1 < bytes.len() { &s[i..i + 2] } else { "" };
        match two {
            "&&" => { toks.push(Tok::AmpAmp); i += 2; continue; }
            "||" => { toks.push(Tok::PipePipe); i += 2; continue; }
            "==" => { toks.push(Tok::EqEq); i += 2; continue; }
            "!=" | "/=" => { toks.push(Tok::BangEq); i += 2; continue; }
            "~&" => { toks.push(Tok::TildeAmp); i += 2; continue; }
            "~|" => { toks.push(Tok::TildePipe); i += 2; continue; }
            "~^" | "^~" => { toks.push(Tok::TildeCaret); i += 2; continue; }
            _ => {}
        }
        match ch {
            '(' => toks.push(Tok::LParen),
            ')' => toks.push(Tok::RParen),
            '[' => toks.push(Tok::LBracket),
            ']' => toks.push(Tok::RBracket),
            '{' => toks.push(Tok::LBrace),
            '}' => toks.push(Tok::RBrace),
            ':' => toks.push(Tok::Colon),
            ',' => toks.push(Tok::Comma),
            '?' => toks.push(Tok::Question),
            '~' => toks.push(Tok::Tilde),
            '!' => toks.push(Tok::Bang),
            '&' => toks.push(Tok::Amp),
            '|' => toks.push(Tok::Pipe),
            '^' => toks.push(Tok::Caret),
            '=' if lang == HdlLanguage::Vhdl => toks.push(Tok::EqEq),
            '+' => toks.push(Tok::Plus),
            '-' => toks.push(Tok::Minus),
            _ => return Err(format!("Unexpected character '{ch}' in expression '{s}'")),
        }
        i += 1;
    }

    Ok(toks)
}

fn parse_number_token(raw: &str) -> Result<Tok, String> {
    if let Some((w_str, rest)) = raw.split_once('\'') {
        let width = w_str.parse::<usize>().unwrap_or(1).max(1);
        let rest = rest.replace('_', "");
        let (radix, digits) = if let Some(d) = rest.strip_prefix('b').or_else(|| rest.strip_prefix('B')) {
            (2, d)
        } else if let Some(d) = rest.strip_prefix('h').or_else(|| rest.strip_prefix('H')) {
            (16, d)
        } else if let Some(d) = rest.strip_prefix('d').or_else(|| rest.strip_prefix('D')) {
            (10, d)
        } else if let Some(d) = rest.strip_prefix('o').or_else(|| rest.strip_prefix('O')) {
            (8, d)
        } else {
            (10, rest.as_str())
        };
        let val = u64::from_str_radix(digits, radix)
            .map_err(|_| format!("Invalid Verilog literal '{raw}'"))?;
        Ok(Tok::Num(val, width))
    } else {
        let val = raw
            .replace('_', "")
            .parse::<u64>()
            .map_err(|_| format!("Invalid integer literal '{raw}'"))?;
        let width = if val <= 1 {
            1
        } else {
            (64 - val.leading_zeros()) as usize
        };
        Ok(Tok::Num(val, width))
    }
}

struct ExprParser {
    toks: Vec<Tok>,
    pos: usize,
    lang: HdlLanguage,
}

impl ExprParser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn next(&mut self) -> Option<Tok> {
        if self.pos < self.toks.len() {
            let t = self.toks[self.pos].clone();
            self.pos += 1;
            Some(t)
        } else {
            None
        }
    }

    fn parse_ternary(&mut self) -> Result<Expr, String> {
        let cond = self.parse_log_or()?;
        if matches!(self.peek(), Some(Tok::Question)) {
            self.next();
            let t_expr = self.parse_ternary()?;
            if !matches!(self.next(), Some(Tok::Colon)) {
                return Err("Expected ':' in ternary expression".into());
            }
            let f_expr = self.parse_ternary()?;
            Ok(Expr::Ternary(Box::new(cond), Box::new(t_expr), Box::new(f_expr)))
        } else {
            Ok(cond)
        }
    }

    fn parse_log_or(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_log_and()?;
        while matches!(self.peek(), Some(Tok::PipePipe)) {
            self.next();
            let rhs = self.parse_log_and()?;
            lhs = Expr::Binary(BinaryOp::LogOr, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_log_and(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_bit_or()?;
        while matches!(self.peek(), Some(Tok::AmpAmp)) {
            self.next();
            let rhs = self.parse_bit_or()?;
            lhs = Expr::Binary(BinaryOp::LogAnd, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_bit_or(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_bit_xor()?;
        while let Some(tok) = self.peek() {
            let op = match tok {
                Tok::Pipe => BinaryOp::Or,
                Tok::TildePipe => BinaryOp::Nor,
                _ => break,
            };
            self.next();
            let rhs = self.parse_bit_xor()?;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_bit_xor(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_bit_and()?;
        while let Some(tok) = self.peek() {
            let op = match tok {
                Tok::Caret => BinaryOp::Xor,
                Tok::TildeCaret => BinaryOp::Xnor,
                _ => break,
            };
            self.next();
            let rhs = self.parse_bit_and()?;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_bit_and(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_eq()?;
        while let Some(tok) = self.peek() {
            let op = match tok {
                Tok::Amp => BinaryOp::And,
                Tok::TildeAmp => BinaryOp::Nand,
                _ => break,
            };
            self.next();
            let rhs = self.parse_eq()?;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_eq(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_add()?;
        while let Some(tok) = self.peek() {
            let op = match tok {
                Tok::EqEq => BinaryOp::Eq,
                Tok::BangEq => BinaryOp::Neq,
                _ => break,
            };
            self.next();
            let rhs = self.parse_add()?;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_add(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_unary()?;
        while let Some(tok) = self.peek() {
            let op = match tok {
                Tok::Plus => BinaryOp::Add,
                Tok::Minus => BinaryOp::Sub,
                _ => break,
            };
            self.next();
            let rhs = self.parse_unary()?;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        match self.peek() {
            Some(Tok::Tilde) => {
                self.next();
                Ok(Expr::Unary(UnaryOp::BitNot, Box::new(self.parse_unary()?)))
            }
            Some(Tok::Bang) => {
                self.next();
                Ok(Expr::Unary(UnaryOp::LogNot, Box::new(self.parse_unary()?)))
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        let mut expr = match self.next() {
            Some(Tok::Num(v, w)) => Expr::Const { val: v, width: w },
            Some(Tok::Ident(name)) => Expr::Ident(name),
            Some(Tok::LParen) => {
                let inner = self.parse_ternary()?;
                if !matches!(self.next(), Some(Tok::RParen)) {
                    return Err("Unclosed '(' in expression".into());
                }
                inner
            }
            Some(Tok::LBrace) => {
                let mut items = Vec::new();
                loop {
                    items.push(self.parse_ternary()?);
                    if matches!(self.peek(), Some(Tok::Comma)) {
                        self.next();
                    } else {
                        break;
                    }
                }
                if !matches!(self.next(), Some(Tok::RBrace)) {
                    return Err("Unclosed '{' in concatenation".into());
                }
                Expr::Concat(items)
            }
            other => return Err(format!("Unexpected token {other:?} in expression")),
        };

        // Postfix bit-select / slice: `a[2]` or `a[3:1]` or VHDL `a(2)`
        loop {
            if matches!(self.peek(), Some(Tok::LBracket)) {
                self.next();
                let hi = match self.next() {
                    Some(Tok::Num(v, _)) => v as usize,
                    _ => return Err("Expected bit index inside [...]".into()),
                };
                if matches!(self.peek(), Some(Tok::Colon)) {
                    self.next();
                    let lo = match self.next() {
                        Some(Tok::Num(v, _)) => v as usize,
                        _ => return Err("Expected low bit index after ':'".into()),
                    };
                    if !matches!(self.next(), Some(Tok::RBracket)) {
                        return Err("Expected ']'".into());
                    }
                    expr = Expr::Slice(Box::new(expr), hi, lo);
                } else {
                    if !matches!(self.next(), Some(Tok::RBracket)) {
                        return Err("Expected ']'".into());
                    }
                    expr = Expr::BitSelect(Box::new(expr), hi);
                }
            } else if self.lang == HdlLanguage::Vhdl
                && matches!(expr, Expr::Ident(_))
                && matches!(self.peek(), Some(Tok::LParen))
            {
                if let Some(Tok::Num(v, _)) = self.toks.get(self.pos + 1).cloned() {
                    if matches!(self.toks.get(self.pos + 2), Some(Tok::RParen)) {
                        self.pos += 3;
                        expr = Expr::BitSelect(Box::new(expr), v as usize);
                        continue;
                    }
                }
                break;
            } else {
                break;
            }
        }

        Ok(expr)
    }
}

fn parse_expr(s: &str, lang: HdlLanguage) -> Result<Expr, String> {
    let toks = tokenize_expr(s, lang)?;
    if toks.is_empty() {
        return Err("Empty expression".into());
    }
    let mut p = ExprParser { toks, pos: 0, lang };
    p.parse_ternary()
}

fn parse_lhs_target(s: &str) -> Result<LhsTarget, String> {
    let s = s.trim();
    if let Some(open) = s.find('[').or_else(|| s.find('(')) {
        let close = s.find(']').or_else(|| s.find(')')).unwrap_or(s.len());
        let name = s[..open].trim().to_string();
        let idx = s[open + 1..close]
            .trim()
            .parse::<usize>()
            .map_err(|_| format!("Invalid bit index in '{s}'"))?;
        Ok(LhsTarget::Bit(name, idx))
    } else {
        Ok(LhsTarget::Signal(s.to_string()))
    }
}

fn lower_comb_stmts_to_assigns(stmts: &[Stmt], out: &mut Vec<CombAssign>) {
    let mut map: HashMap<String, Expr> = HashMap::new();
    collect_stmt_assigns(stmts, None, &mut map);
    for (sig, expr) in map {
        out.push(CombAssign {
            target: LhsTarget::Signal(sig),
            expr,
        });
    }
}

fn collect_stmt_assigns(
    stmts: &[Stmt],
    cond: Option<&Expr>,
    map: &mut HashMap<String, Expr>,
) {
    for st in stmts {
        match st {
            Stmt::Assign { target, expr } => {
                let sig = match target {
                    LhsTarget::Signal(s) => s.clone(),
                    LhsTarget::Bit(s, _) => s.clone(),
                };
                let prev = map
                    .get(&sig)
                    .cloned()
                    .unwrap_or_else(|| Expr::Ident(sig.clone()));
                let next_expr = if let Some(c) = cond {
                    Expr::Ternary(Box::new(c.clone()), Box::new(expr.clone()), Box::new(prev))
                } else {
                    expr.clone()
                };
                map.insert(sig, next_expr);
            }
            Stmt::If {
                cond: if_c,
                then_body,
                else_body,
            } => {
                let then_c = match cond {
                    Some(pc) => Expr::Binary(BinaryOp::LogAnd, Box::new(pc.clone()), Box::new(if_c.clone())),
                    None => if_c.clone(),
                };
                let not_if = Expr::Unary(UnaryOp::LogNot, Box::new(if_c.clone()));
                let else_c = match cond {
                    Some(pc) => Expr::Binary(BinaryOp::LogAnd, Box::new(pc.clone()), Box::new(not_if)),
                    None => not_if,
                };
                collect_stmt_assigns(then_body, Some(&then_c), map);
                collect_stmt_assigns(else_body, Some(&else_c), map);
            }
        }
    }
}

// ============================================================================
// Built-in XSPICE Synthesizer (Zero External Dependencies)
// ============================================================================

struct XspiceEmitter {
    lines: Vec<String>,
    next_id: usize,
    used_models: HashSet<&'static str>,
    const_zero_node: Option<String>,
    const_one_node: Option<String>,
    signal_bits: HashMap<String, Vec<String>>, // signal_name -> [bit0_dnode, bit1_dnode, ...]
    case_insensitive: bool,
}

impl XspiceEmitter {
    fn new(case_insensitive: bool) -> Self {
        Self {
            lines: Vec::new(),
            next_id: 0,
            used_models: HashSet::new(),
            const_zero_node: None,
            const_one_node: None,
            signal_bits: HashMap::new(),
            case_insensitive,
        }
    }

    fn key(&self, name: &str) -> String {
        if self.case_insensitive {
            name.to_ascii_lowercase()
        } else {
            name.to_string()
        }
    }

    fn fresh(&mut self, prefix: &str) -> String {
        self.next_id += 1;
        format!("_d_{prefix}_{}", self.next_id)
    }

    fn const0(&mut self) -> String {
        if let Some(n) = &self.const_zero_node {
            return n.clone();
        }
        let n = "_d_const0".to_string();
        self.used_models.insert("d_pulldown");
        self.lines.push(format!("a_c0 {n} m_pd"));
        self.const_zero_node = Some(n.clone());
        n
    }

    fn const1(&mut self) -> String {
        if let Some(n) = &self.const_one_node {
            return n.clone();
        }
        let n = "_d_const1".to_string();
        self.used_models.insert("d_pullup");
        self.lines.push(format!("a_c1 {n} m_pu"));
        self.const_one_node = Some(n.clone());
        n
    }

    fn emit_inv(&mut self, a: &str) -> String {
        let out = self.fresh("inv");
        self.used_models.insert("d_inverter");
        self.lines.push(format!("a_{out} {a} {out} m_inv"));
        out
    }

    fn emit_buf(&mut self, a: &str, out: &str) {
        self.used_models.insert("d_buffer");
        let id = self.fresh("buf");
        self.lines.push(format!("a_{id} {a} {out} m_buf"));
    }

    fn emit_gate2(&mut self, kind: &'static str, a: &str, b: &str) -> String {
        let out = self.fresh(kind);
        self.used_models.insert(kind);
        let mname = match kind {
            "d_and" => "m_and",
            "d_or" => "m_or",
            "d_xor" => "m_xor",
            "d_nand" => "m_nand",
            "d_nor" => "m_nor",
            "d_xnor" => "m_xnor",
            _ => "m_and",
        };
        self.lines.push(format!("a_{out} [{a} {b}] {out} {mname}"));
        out
    }

    fn emit_mux(&mut self, sel: &str, a_true: &str, b_false: &str) -> String {
        let nsel = self.emit_inv(sel);
        let t1 = self.emit_gate2("d_and", sel, a_true);
        let t2 = self.emit_gate2("d_and", &nsel, b_false);
        self.emit_gate2("d_or", &t1, &t2)
    }

    fn get_signal_bits(&mut self, name: &str) -> Vec<String> {
        let k = self.key(name);
        if let Some(bits) = self.signal_bits.get(&k) {
            return bits.clone();
        }
        let dnode = format!("d_{}", sanitize_spice_id(&k));
        let v = vec![dnode];
        self.signal_bits.insert(k, v.clone());
        v
    }

    fn lower_expr(&mut self, expr: &Expr, target_width: usize) -> Result<Vec<String>, String> {
        let mut bits = match expr {
            Expr::Const { val, width } => {
                let w = (*width).max(target_width).max(1);
                (0..w)
                    .map(|i| {
                        if (*val >> i) & 1 == 1 {
                            self.const1()
                        } else {
                            self.const0()
                        }
                    })
                    .collect()
            }
            Expr::Ident(name) => self.get_signal_bits(name),
            Expr::BitSelect(inner, idx) => {
                let ib = self.lower_expr(inner, 0)?;
                let b = ib.get(*idx).cloned().unwrap_or_else(|| self.const0());
                vec![b]
            }
            Expr::Slice(inner, hi, lo) => {
                let ib = self.lower_expr(inner, 0)?;
                let (a, b) = if hi >= lo { (*lo, *hi) } else { (*hi, *lo) };
                (a..=b)
                    .map(|i| ib.get(i).cloned().unwrap_or_else(|| self.const0()))
                    .collect()
            }
            Expr::Concat(items) => {
                // Verilog {msb, ..., lsb} -> bits[0] is LSB
                let mut res = Vec::new();
                for it in items.iter().rev() {
                    let b = self.lower_expr(it, 0)?;
                    res.extend(b);
                }
                res
            }
            Expr::Unary(UnaryOp::BitNot, inner) => {
                let ib = self.lower_expr(inner, target_width)?;
                ib.iter().map(|b| self.emit_inv(b)).collect()
            }
            Expr::Unary(UnaryOp::LogNot, inner) => {
                let ib = self.lower_expr(inner, 0)?;
                let any = self.reduce_or(&ib);
                vec![self.emit_inv(&any)]
            }
            Expr::Binary(op, lhs, rhs) => {
                let op_w = match op {
                    BinaryOp::Eq | BinaryOp::Neq | BinaryOp::LogAnd | BinaryOp::LogOr => 0,
                    _ => target_width,
                };
                let lb = self.lower_expr(lhs, op_w)?;
                let rb = self.lower_expr(rhs, op_w)?;
                let w = lb.len().max(rb.len()).max(1);
                let lb = self.pad_bits(lb, w);
                let rb = self.pad_bits(rb, w);
                match op {
                    BinaryOp::And | BinaryOp::Or | BinaryOp::Xor | BinaryOp::Nand | BinaryOp::Nor | BinaryOp::Xnor => {
                        let gk = match op {
                            BinaryOp::And => "d_and",
                            BinaryOp::Or => "d_or",
                            BinaryOp::Xor => "d_xor",
                            BinaryOp::Nand => "d_nand",
                            BinaryOp::Nor => "d_nor",
                            BinaryOp::Xnor => "d_xnor",
                            _ => unreachable!(),
                        };
                        (0..w).map(|i| self.emit_gate2(gk, &lb[i], &rb[i])).collect()
                    }
                    BinaryOp::LogAnd => {
                        let a = self.reduce_or(&lb);
                        let b = self.reduce_or(&rb);
                        vec![self.emit_gate2("d_and", &a, &b)]
                    }
                    BinaryOp::LogOr => {
                        let a = self.reduce_or(&lb);
                        let b = self.reduce_or(&rb);
                        vec![self.emit_gate2("d_or", &a, &b)]
                    }
                    BinaryOp::Eq | BinaryOp::Neq => {
                        let mut xors = Vec::with_capacity(w);
                        for i in 0..w {
                            xors.push(self.emit_gate2("d_xor", &lb[i], &rb[i]));
                        }
                        let diff = self.reduce_or(&xors);
                        if *op == BinaryOp::Neq {
                            vec![diff]
                        } else {
                            vec![self.emit_inv(&diff)]
                        }
                    }
                    BinaryOp::Add => self.emit_adder(&lb, &rb, false),
                    BinaryOp::Sub => self.emit_adder(&lb, &rb, true),
                }
            }
            Expr::Ternary(cond, t_expr, f_expr) => {
                let cb = self.lower_expr(cond, 0)?;
                let sel = self.reduce_or(&cb);
                let tb = self.lower_expr(t_expr, target_width)?;
                let fb = self.lower_expr(f_expr, target_width)?;
                let w = tb.len().max(fb.len()).max(target_width).max(1);
                let tb = self.pad_bits(tb, w);
                let fb = self.pad_bits(fb, w);
                (0..w).map(|i| self.emit_mux(&sel, &tb[i], &fb[i])).collect()
            }
        };

        if target_width > 0 && bits.len() != target_width {
            bits = self.pad_bits(bits, target_width);
            bits.truncate(target_width);
        }
        Ok(bits)
    }

    fn pad_bits(&mut self, mut bits: Vec<String>, width: usize) -> Vec<String> {
        while bits.len() < width {
            bits.push(self.const0());
        }
        bits
    }

    fn reduce_or(&mut self, bits: &[String]) -> String {
        if bits.is_empty() {
            return self.const0();
        }
        let mut acc = bits[0].clone();
        for b in &bits[1..] {
            acc = self.emit_gate2("d_or", &acc, b);
        }
        acc
    }

    fn emit_adder(&mut self, a: &[String], b: &[String], subtract: bool) -> Vec<String> {
        let w = a.len().max(b.len());
        let mut carry = if subtract { self.const1() } else { self.const0() };
        let mut sum = Vec::with_capacity(w);
        for i in 0..w {
            let ai = a.get(i).cloned().unwrap_or_else(|| self.const0());
            let bi_raw = b.get(i).cloned().unwrap_or_else(|| self.const0());
            let bi = if subtract { self.emit_inv(&bi_raw) } else { bi_raw };
            let axb = self.emit_gate2("d_xor", &ai, &bi);
            let si = self.emit_gate2("d_xor", &axb, &carry);
            let c1 = self.emit_gate2("d_and", &ai, &bi);
            let c2 = self.emit_gate2("d_and", &axb, &carry);
            carry = self.emit_gate2("d_or", &c1, &c2);
            sum.push(si);
        }
        sum
    }
}

fn synthesize_to_xspice(block: &HdlBlock, subckt_name: &str) -> Result<Vec<String>, String> {
    let parsed = parse_hdl(block.language, &block.source)?;
    let ci = block.language == HdlLanguage::Vhdl;
    let mut em = XspiceEmitter::new(ci);

    // Build pin list in the exact same order as `CustomSymbol`: inputs first, then outputs/inouts
    // Tuple: (spice_pin_name, digital_node, port_name, expanded_pin_name)
    let mut left_spice_pins: Vec<(String, String, String, String)> = Vec::new();
    let mut right_spice_pins: Vec<(String, String, String, String)> = Vec::new();

    for p in &parsed.ports {
        let sp_names = p.expanded_spice_names();
        let pin_names = p.expanded_pin_names();
        let d_nodes: Vec<String> = sp_names
            .iter()
            .map(|n| format!("d_{}", sanitize_spice_id(&n.to_ascii_lowercase())))
            .collect();
        em.signal_bits.insert(em.key(&p.name), d_nodes.clone());
        for ((ap, dn), pn) in sp_names
            .into_iter()
            .zip(d_nodes.into_iter())
            .zip(pin_names.into_iter())
        {
            match p.dir {
                HdlPortDir::Input => left_spice_pins.push((ap, dn, p.name.clone(), pn)),
                HdlPortDir::Output | HdlPortDir::Inout => {
                    right_spice_pins.push((ap, dn, p.name.clone(), pn))
                }
            }
        }
    }

    // Pre-allocate internal signals
    for (sig, &w) in &parsed.signals {
        let k = em.key(sig);
        if !em.signal_bits.contains_key(&k) {
            let nodes: Vec<String> = (0..w.max(1))
                .map(|i| {
                    if w <= 1 {
                        format!("d_{}", sanitize_spice_id(&k))
                    } else {
                        format!("d_{}_{i}", sanitize_spice_id(&k))
                    }
                })
                .collect();
            em.signal_bits.insert(k, nodes);
        }
    }

    // Emit sequential blocks (flip-flops)
    for sb in &parsed.seq_blocks {
        let clk_bits = em.get_signal_bits(&sb.clk);
        let clk_raw = clk_bits[0].clone();
        let clk_sig = if sb.clk_posedge {
            clk_raw
        } else {
            em.emit_inv(&clk_raw)
        };

        // Check if top-level statement is `if (rst) ... else ...` matching async_rst
        let (rst_sig_node, rst_map, body_stmts) = extract_async_reset(sb, &mut em);

        let mut next_state: HashMap<String, Expr> = HashMap::new();
        collect_stmt_assigns(&body_stmts, None, &mut next_state);

        em.used_models.insert("d_dff");
        for (reg_name, d_expr) in next_state {
            let q_bits = em.get_signal_bits(&reg_name);
            let w = q_bits.len();
            let d_bits = em.lower_expr(&d_expr, w)?;
            let rst_val = rst_map.get(&em.key(&reg_name)).copied().unwrap_or(0);

            for i in 0..w {
                let d_in = &d_bits[i];
                let q_out = &q_bits[i];
                let qb = em.fresh("qb");
                let (set_pin, rst_pin) = if let Some(ref rn) = rst_sig_node {
                    if (rst_val >> i) & 1 == 1 {
                        (rn.as_str(), "NULL")
                    } else {
                        ("NULL", rn.as_str())
                    }
                } else {
                    ("NULL", "NULL")
                };
                let id = em.fresh("dff");
                em.lines.push(format!(
                    "a_{id} {d_in} {clk_sig} {set_pin} {rst_pin} {q_out} {qb} m_dff"
                ));
            }
        }
    }

    // Emit combinational assignments
    for ca in &parsed.comb_assigns {
        match &ca.target {
            LhsTarget::Signal(sig) => {
                let dst_bits = em.get_signal_bits(sig);
                let src_bits = em.lower_expr(&ca.expr, dst_bits.len())?;
                for (s, d) in src_bits.iter().zip(dst_bits.iter()) {
                    em.emit_buf(s, d);
                }
            }
            LhsTarget::Bit(sig, idx) => {
                let dst_bits = em.get_signal_bits(sig);
                let src_bits = em.lower_expr(&ca.expr, 1)?;
                if let (Some(s), Some(d)) = (src_bits.first(), dst_bits.get(*idx)) {
                    em.emit_buf(s, d);
                }
            }
        }
    }

    // Assemble final .subckt:
    // Pin order matches CustomSymbol: [inputs..., outputs..., wired_ref_pins..., extra_external_ref_nets...]
    let extra_refs = block.extra_ref_nets();
    let mut ref_net_to_port: HashMap<String, String> = HashMap::new();
    let mut all_pins: Vec<String> = Vec::new();
    for (ap, _, _, _) in &left_spice_pins {
        all_pins.push(ap.clone());
    }
    for (ap, _, _, _) in &right_spice_pins {
        all_pins.push(ap.clone());
    }
    let mut wired_ref_ports: Vec<String> = Vec::new();
    for (ridx, rpin) in block.ref_pins.iter().enumerate() {
        let rport = format!("_rpin_{ridx}_{}", sanitize_spice_id(&rpin.to_ascii_lowercase()));
        ref_net_to_port.insert(rpin.to_ascii_lowercase(), rport.clone());
        wired_ref_ports.push(rport.clone());
        all_pins.push(rport);
    }
    for (ridx, rnet) in extra_refs.iter().enumerate() {
        let rport = format!("_ref_{ridx}");
        ref_net_to_port.insert(rnet.to_ascii_lowercase(), rport.clone());
        all_pins.push(rport);
    }

    let mut out = Vec::new();
    out.push(format!(
        "* Auto-synthesized {} block '{}'",
        block.language.label(),
        block.module_name
    ));
    out.push(format!(".subckt {subckt_name} {}", all_pins.join(" ")));

    // High-impedance tie-down on wired ref pins so an unconnected ref pin stays at 0V cleanly
    for (ridx, rport) in wired_ref_ports.iter().enumerate() {
        out.push(format!("R_rpin_gnd_{ridx} {rport} 0 1e12"));
    }

    if !left_spice_pins.is_empty() {
        // Sharp per-pin CMOS input buffer referenced to each pin's reference net/pin
        // (0/GND by default) and custom V_IL/V_IH threshold midpoint.
        let mut buf_in: Vec<String> = Vec::with_capacity(left_spice_pins.len());
        for (idx, (a_pin, _, port_name, pin_name)) in left_spice_pins.iter().enumerate() {
            let (ref_opt, v_il, v_ih, _, _) = block.effective_port_settings(port_name, pin_name);
            let v_mid_in = 0.5 * (v_il + v_ih);
            let vin_expr = match ref_opt.and_then(|rn| ref_net_to_port.get(&rn.to_ascii_lowercase())) {
                Some(rport) => format!("(V({a_pin}) - V({rport}))"),
                None => format!("V({a_pin})"),
            };
            let bnode = format!("_cmos_in_{idx}");
            out.push(format!(
                "B_cmos_in_{idx} {bnode} 0 V = 2.5 + 2.5 * tanh(1000 * ({vin_expr} - {v_mid_in}))"
            ));
            buf_in.push(bnode);
        }
        let d_in: Vec<&str> = left_spice_pins.iter().map(|(_, d, _, _)| d.as_str()).collect();
        out.push(format!(
            "a_adc [{}] [{}] m_adc",
            buf_in.join(" "),
            d_in.join(" ")
        ));
    }

    out.extend(em.lines);

    if !right_spice_pins.is_empty() {
        let d_out: Vec<&str> = right_spice_pins.iter().map(|(_, d, _, _)| d.as_str()).collect();
        let norm_nodes: Vec<String> = (0..right_spice_pins.len())
            .map(|i| format!("_dac_norm_{i}"))
            .collect();
        out.push(format!(
            "a_dac [{}] [{}] m_dac",
            d_out.join(" "),
            norm_nodes.join(" ")
        ));
        // Per-pin output voltage driver referenced to each pin's ref_net/pin (0/GND by default)
        for (idx, (a_pin, _, port_name, pin_name)) in right_spice_pins.iter().enumerate() {
            let (ref_opt, _, _, v_low, v_high) = block.effective_port_settings(port_name, pin_name);
            let ref_node = ref_opt
                .and_then(|rn| ref_net_to_port.get(&rn.to_ascii_lowercase()).map(|s| s.as_str()))
                .unwrap_or("0");
            let swing = v_high - v_low;
            let nnode = &norm_nodes[idx];
            out.push(format!(
                "B_cmos_out_{idx} {a_pin} {ref_node} V = {v_low} + {swing} * V({nnode})"
            ));
        }
    }

    // Bridge & digital gate models
    let t_rise = if block.t_rise.trim().is_empty() { "5n" } else { block.t_rise.trim() };
    let t_fall = if block.t_fall.trim().is_empty() { "5n" } else { block.t_fall.trim() };
    if !left_spice_pins.is_empty() {
        out.push(".model m_adc adc_bridge(in_low=1.5 in_high=3.5)".to_string());
    }
    if !right_spice_pins.is_empty() {
        out.push(format!(
            ".model m_dac dac_bridge(out_low=0.0 out_high=1.0 out_undef=0.5 input_load=1e-12 t_rise={t_rise} t_fall={t_fall})"
        ));
    }
    for m in [
        ("d_pulldown", ".model m_pd d_pulldown(load=1e-12)"),
        ("d_pullup", ".model m_pu d_pullup(load=1e-12)"),
        ("d_inverter", ".model m_inv d_inverter(rise_delay=1n fall_delay=1n input_load=1e-12)"),
        ("d_buffer", ".model m_buf d_buffer(rise_delay=1n fall_delay=1n input_load=1e-12)"),
        ("d_and", ".model m_and d_and(rise_delay=1n fall_delay=1n input_load=1e-12)"),
        ("d_or", ".model m_or d_or(rise_delay=1n fall_delay=1n input_load=1e-12)"),
        ("d_xor", ".model m_xor d_xor(rise_delay=1n fall_delay=1n input_load=1e-12)"),
        ("d_nand", ".model m_nand d_nand(rise_delay=1n fall_delay=1n input_load=1e-12)"),
        ("d_nor", ".model m_nor d_nor(rise_delay=1n fall_delay=1n input_load=1e-12)"),
        ("d_xnor", ".model m_xnor d_xnor(rise_delay=1n fall_delay=1n input_load=1e-12)"),
        ("d_dff", ".model m_dff d_dff(clk_delay=1n set_delay=1n reset_delay=1n ic=0 rise_delay=1n fall_delay=1n)"),
    ] {
        if em.used_models.contains(m.0) {
            out.push(m.1.to_string());
        }
    }

    out.push(format!(".ends {subckt_name}"));
    Ok(out)
}

fn extract_async_reset(
    sb: &SeqBlock,
    em: &mut XspiceEmitter,
) -> (Option<String>, HashMap<String, u64>, Vec<Stmt>) {
    if let Some((rst_name, active_high)) = &sb.async_rst {
        if sb.stmts.len() == 1 {
            if let Stmt::If {
                then_body,
                else_body,
                ..
            } = &sb.stmts[0]
            {
                let rst_bits = em.get_signal_bits(rst_name);
                let rst_node = if *active_high {
                    rst_bits[0].clone()
                } else {
                    em.emit_inv(&rst_bits[0])
                };
                let mut rst_vals = HashMap::new();
                for st in then_body {
                    if let Stmt::Assign {
                        target: LhsTarget::Signal(sig),
                        expr: Expr::Const { val, .. },
                    } = st
                    {
                        rst_vals.insert(em.key(sig), *val);
                    }
                }
                return (Some(rst_node), rst_vals, else_body.clone());
            }
        }
    }
    (None, HashMap::new(), sb.stmts.clone())
}

// ============================================================================
// Optional External Verilator d_cosim Backend
// ============================================================================

pub fn has_verilator_cosim_tools() -> bool {
    crate::settings::find_in_path("verilator").is_some()
        && (crate::settings::find_in_path("vlnggen").is_some()
            || PathBuf::from("/usr/share/ngspice/scripts/vlnggen").is_file())
}

fn synthesize_to_verilator_cosim(block: &HdlBlock, subckt_name: &str) -> Result<Vec<String>, String> {
    if block.language != HdlLanguage::Verilog {
        return Err("Verilator d_cosim backend supports Verilog; use Built-in XSPICE for VHDL.".into());
    }
    if crate::settings::find_in_path("verilator").is_none() {
        return Err(
            "Verilator is not installed on PATH. Install verilator or select 'Built-in XSPICE Synthesizer'."
                .into(),
        );
    }
    let vlnggen = crate::settings::find_in_path("vlnggen")
        .or_else(|| {
            let p = PathBuf::from("/usr/share/ngspice/scripts/vlnggen");
            p.is_file().then_some(p)
        })
        .ok_or_else(|| "ngspice 'vlnggen' script not found.".to_string())?;

    let parsed = parse_hdl(block.language, &block.source)?;
    let tmp_dir = std::env::temp_dir().join(format!("mcp_itspice_hdl_{}", sanitize_spice_id(subckt_name)));
    let _ = std::fs::create_dir_all(&tmp_dir);
    let v_file = tmp_dir.join(format!("{}.v", sanitize_spice_id(&parsed.module_name)));
    std::fs::write(&v_file, &block.source)
        .map_err(|e| format!("Failed to write temporary Verilog file: {e}"))?;

    let output = std::process::Command::new(&vlnggen)
        .current_dir(&tmp_dir)
        .arg(&v_file)
        .output()
        .map_err(|e| format!("Failed to invoke vlnggen: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "vlnggen failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let so_path = tmp_dir.join(format!("{}.so", sanitize_spice_id(&parsed.module_name)));
    if !so_path.is_file() {
        return Err(format!("vlnggen did not produce {}", so_path.display()));
    }

    let mut left_pins = Vec::new();
    let mut right_pins = Vec::new();
    for p in &parsed.ports {
        for n in p.expanded_spice_names() {
            let dn = format!("d_{n}");
            match p.dir {
                HdlPortDir::Input => left_pins.push((n, dn)),
                HdlPortDir::Output | HdlPortDir::Inout => right_pins.push((n, dn)),
            }
        }
    }
    let all_pins: Vec<&str> = left_pins
        .iter()
        .chain(right_pins.iter())
        .map(|(a, _)| a.as_str())
        .collect();
    let d_in: Vec<&str> = left_pins.iter().map(|(_, d)| d.as_str()).collect();
    let d_out: Vec<&str> = right_pins.iter().map(|(_, d)| d.as_str()).collect();
    let a_in: Vec<&str> = left_pins.iter().map(|(a, _)| a.as_str()).collect();
    let a_out: Vec<&str> = right_pins.iter().map(|(a, _)| a.as_str()).collect();

    let v_undef = 0.5 * (block.v_low + block.v_high);
    let mut lines = vec![
        format!("* Verilator d_cosim block '{}'", block.module_name),
        format!(".subckt {subckt_name} {}", all_pins.join(" ")),
    ];
    if !a_in.is_empty() {
        lines.push(format!("a_adc [{}] [{}] m_adc", a_in.join(" "), d_in.join(" ")));
    }
    lines.push(format!(
        "a_cosim [{}] [{}] m_cosim",
        d_in.join(" "),
        d_out.join(" ")
    ));
    if !a_out.is_empty() {
        lines.push(format!("a_dac [{}] [{}] m_dac", d_out.join(" "), a_out.join(" ")));
    }
    lines.push(format!(
        ".model m_cosim d_cosim(simulation=\"{}\")",
        so_path.to_string_lossy().replace('\\', "/")
    ));
    lines.push(format!(
        ".model m_adc adc_bridge(in_low={} in_high={})",
        block.v_thresh_low, block.v_thresh_high
    ));
    lines.push(format!(
        ".model m_dac dac_bridge(out_low={} out_high={} out_undef={} input_load=1e-12 t_rise={} t_fall={})",
        block.v_low, block.v_high, v_undef, block.t_rise, block.t_fall
    ));
    lines.push(format!(".ends {subckt_name}"));
    Ok(lines)
}

// ============================================================================
// String helpers
// ============================================================================

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn starts_with_keyword(s: &str, kw: &str) -> bool {
    if !s.starts_with(kw) {
        return false;
    }
    let rest = &s.as_bytes()[kw.len()..];
    rest.first().map(|&b| !is_ident_char(b)).unwrap_or(true)
}

fn find_keyword(s: &str, kw: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut start = 0;
    while let Some(rel) = s[start..].find(kw) {
        let idx = start + rel;
        let before_ok = idx == 0 || !is_ident_char(bytes[idx - 1]);
        let after_idx = idx + kw.len();
        let after_ok = after_idx >= bytes.len() || !is_ident_char(bytes[after_idx]);
        if before_ok && after_ok {
            return Some(idx);
        }
        start = idx + 1;
    }
    None
}

fn find_matching_delim(s: &str, open: char, close: char) -> Option<usize> {
    let mut depth = 0i32;
    for (i, ch) in s.char_indices() {
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
    }
    None
}

fn split_top_level(s: &str, delim: char) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, ch) in s.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            c if c == delim && depth == 0 => {
                out.push(&s[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

fn split_assignment(line: &str, lang: HdlLanguage) -> Option<(&str, &str)> {
    if let Some((l, r)) = line.split_once("<=") {
        return Some((l, r));
    }
    if lang == HdlLanguage::Vhdl {
        if let Some((l, r)) = line.split_once(":=") {
            return Some((l, r));
        }
    }
    // Single '=' (avoiding ==, !=, <=, >=)
    let bytes = line.as_bytes();
    for i in 0..bytes.len() {
        if bytes[i] == b'=' {
            let prev = if i > 0 { bytes[i - 1] } else { 0 };
            let next = if i + 1 < bytes.len() { bytes[i + 1] } else { 0 };
            if prev != b'=' && prev != b'!' && prev != b'<' && prev != b'>' && next != b'=' {
                return Some((&line[..i], &line[i + 1..]));
            }
        }
    }
    None
}

// ============================================================================
// Unit Tests (End-to-End Verilog & VHDL Simulation in ngspice!)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ngspice::NgSpiceEngine;

    #[test]
    fn test_verilog_logic_gate_sim() {
        let mut blk = HdlBlock::default();
        blk.language = HdlLanguage::Verilog;
        blk.source = verilog_templates()[0].1.to_string();
        blk.refresh_metadata();
        assert_eq!(blk.module_name, "logic_gate");
        assert_eq!(blk.ports.len(), 4);

        let subckt = blk.synthesize_subckt("HDL_LOGIC").expect("synth verilog");
        let mut netlist = vec![
            "* Verilog logic_gate test".to_string(),
            "Va a 0 PULSE(0 5 0 1n 1n 10u 20u)".to_string(),
            "Vb b 0 PULSE(0 5 5u 1n 1n 10u 20u)".to_string(),
            "XU1 a b y_and y_xor HDL_LOGIC".to_string(),
            "R1 y_and 0 100k".to_string(),
            "R2 y_xor 0 100k".to_string(),
        ];
        netlist.extend(subckt);
        netlist.push(".tran 100n 25u".to_string());
        netlist.push(".end".to_string());

        let res = {
            let eng = NgSpiceEngine::global().unwrap().lock().unwrap();
            eng.run_netlist(&netlist)
        };
        assert!(res.success, "sim failed: {:?} log: {:?}", res.error, res.log);

        // At t = 2.5u: a=5V, b=0V => y_and=0V, y_xor=5V
        // At t = 7.5u: a=5V, b=5V => y_and=5V, y_xor=0V
        let v_and = res.traces.get("V(y_and)").expect("V(y_and)");
        let v_xor = res.traces.get("V(y_xor)").expect("V(y_xor)");
        let sample_at = |t_target: f64, trace: &[f64]| -> f64 {
            let idx = res
                .x_values
                .iter()
                .position(|&t| t >= t_target)
                .unwrap_or(trace.len() - 1);
            trace[idx]
        };
        assert!(sample_at(2.5e-6, &v_and.real) < 0.5);
        assert!(sample_at(2.5e-6, &v_xor.real) > 4.5);
        assert!(sample_at(7.5e-6, &v_and.real) > 4.5);
        assert!(sample_at(7.5e-6, &v_xor.real) < 0.5);
    }

    #[test]
    fn test_vhdl_counter_sim() {
        let mut blk = HdlBlock::default();
        blk.language = HdlLanguage::Vhdl;
        blk.source = vhdl_templates()[3].1.to_string(); // 4-bit counter
        blk.refresh_metadata();
        assert_eq!(blk.module_name, "counter4");
        assert_eq!(blk.expanded_pins().len(), 6); // clk, rst, q[0..3]

        let subckt = blk.synthesize_subckt("HDL_CNT4").expect("synth vhdl");
        let mut netlist = vec![
            "* VHDL counter4 test".to_string(),
            "Vclk clk 0 PULSE(0 5 5u 1n 1n 5u 10u)".to_string(),
            "Vrst rst 0 PULSE(5 0 2u 1n 1n 100m 200m)".to_string(),
            "XU1 clk rst q0 q1 q2 q3 HDL_CNT4".to_string(),
            "R0 q0 0 100k".to_string(),
            "R1 q1 0 100k".to_string(),
            "R2 q2 0 100k".to_string(),
            "R3 q3 0 100k".to_string(),
        ];
        netlist.extend(subckt);
        netlist.push(".tran 100n 38u".to_string());
        netlist.push(".end".to_string());

        let res = {
            let eng = NgSpiceEngine::global().unwrap().lock().unwrap();
            eng.run_netlist(&netlist)
        };
        assert!(res.success, "sim failed: {:?} log: {:?}", res.error, res.log);

        // Rising edges at 5u (cnt=1), 15u (cnt=2), 25u (cnt=3), 35u (cnt=4)
        let q0 = res.traces.get("V(q0)").expect("V(q0)");
        let q1 = res.traces.get("V(q1)").expect("V(q1)");
        let q2 = res.traces.get("V(q2)").expect("V(q2)");
        let sample_at = |t_target: f64, trace: &[f64]| -> f64 {
            let idx = res
                .x_values
                .iter()
                .position(|&t| t >= t_target)
                .unwrap_or(trace.len() - 1);
            trace[idx]
        };
        // At 8u -> cnt = 1 (001)
        assert!(sample_at(8e-6, &q0.real) > 4.5, "q0 at 8u = {}", sample_at(8e-6, &q0.real));
        assert!(sample_at(8e-6, &q1.real) < 0.5);
        // At 18u -> cnt = 2 (010)
        assert!(sample_at(18e-6, &q0.real) < 0.5);
        assert!(sample_at(18e-6, &q1.real) > 4.5);
        // At 28u -> cnt = 3 (011)
        assert!(sample_at(28e-6, &q0.real) > 4.5);
        assert!(sample_at(28e-6, &q1.real) > 4.5);
        // At 37u -> cnt = 4 (100)
        assert!(sample_at(37e-6, &q0.real) < 0.5);
        assert!(sample_at(37e-6, &q1.real) < 0.5);
        assert!(sample_at(37e-6, &q2.real) > 4.5);
    }

    #[test]
    fn test_verilog_and_vhdl_fsm_sim() {
        for (lang, src) in [
            (HdlLanguage::Verilog, verilog_templates()[5].1),
            (HdlLanguage::Vhdl, vhdl_templates()[4].1),
        ] {
            let mut blk = HdlBlock::default();
            blk.language = lang;
            blk.source = src.to_string();
            blk.refresh_metadata();
            assert_eq!(blk.module_name, "fsm_ctrl");

            let subckt = blk.synthesize_subckt("HDL_FSM").expect("synth fsm");
            let mut netlist = vec![
                format!("* {:?} FSM test", lang),
                "Vclk clk 0 PULSE(0 5 5u 1n 1n 5u 10u)".to_string(),
                "Vrst rst 0 PULSE(5 0 2u 1n 1n 100m 200m)".to_string(),
                "Vstart start 0 DC 5".to_string(),
                "XU1 clk rst start s0 s1 done HDL_FSM".to_string(),
                "R0 s0 0 100k".to_string(),
                "R1 s1 0 100k".to_string(),
                "Rd done 0 100k".to_string(),
            ];
            netlist.extend(subckt);
            netlist.push(".tran 100n 48u".to_string());
            netlist.push(".end".to_string());

            let res = {
                let eng = NgSpiceEngine::global().unwrap().lock().unwrap();
                eng.run_netlist(&netlist)
            };
            assert!(res.success, "{lang:?} FSM sim failed: {:?} log: {:?}", res.error, res.log);

            let s0 = res.traces.get("V(s0)").expect("V(s0)");
            let s1 = res.traces.get("V(s1)").expect("V(s1)");
            let done = res.traces.get("V(done)").expect("V(done)");
            let sample_at = |t_target: f64, trace: &[f64]| -> f64 {
                let idx = res
                    .x_values
                    .iter()
                    .position(|&t| t >= t_target)
                    .unwrap_or(trace.len() - 1);
                trace[idx]
            };
            // clk edges: 5u -> 01, 15u -> 10, 25u -> 11 (done=1), 35u -> 00
            assert!(sample_at(8e-6, &s0.real) > 4.5, "{lang:?} s0@8u");
            assert!(sample_at(8e-6, &s1.real) < 0.5, "{lang:?} s1@8u");
            assert!(sample_at(8e-6, &done.real) < 0.5, "{lang:?} done@8u");

            assert!(sample_at(18e-6, &s0.real) < 0.5, "{lang:?} s0@18u");
            assert!(sample_at(18e-6, &s1.real) > 4.5, "{lang:?} s1@18u");
            assert!(sample_at(18e-6, &done.real) < 0.5, "{lang:?} done@18u");

            assert!(sample_at(28e-6, &s0.real) > 4.5, "{lang:?} s0@28u");
            assert!(sample_at(28e-6, &s1.real) > 4.5, "{lang:?} s1@28u");
            assert!(sample_at(28e-6, &done.real) > 4.5, "{lang:?} done@28u");

            assert!(sample_at(38e-6, &s0.real) < 0.5, "{lang:?} s0@38u");
            assert!(sample_at(38e-6, &s1.real) < 0.5, "{lang:?} s1@38u");
            assert!(sample_at(38e-6, &done.real) < 0.5, "{lang:?} done@38u");
        }
    }

    #[test]
    fn test_hdl_per_port_ref_and_thresholds_sim() {
        // Test custom per-port reference nets (non-GND) and custom thresholds/levels:
        // - Input `a` referenced to `vref_in` (2.0V), with V_IL=0.8V, V_IH=1.2V (midpoint 1.0V above vref_in => 3.0V abs)
        // - Input `b` referenced to GND (0V), default threshold (2.5V)
        // - Output `y_and` referenced to `vref_out` (-2.0V), with V_low=0.0V, V_high=3.3V => swings -2.0V to +1.3V!
        let mut blk = HdlBlock::default();
        blk.language = HdlLanguage::Verilog;
        blk.source = verilog_templates()[0].1.to_string(); // logic_gate(a, b, y_and, y_xor)
        blk.refresh_metadata();
        blk.port_configs.insert(
            "a".into(),
            HdlPortConfig {
                ref_net: "vref_in".into(),
                v_thresh_low: Some(0.8),
                v_thresh_high: Some(1.2),
                v_low: None,
                v_high: None,
            },
        );
        blk.port_configs.insert(
            "y_and".into(),
            HdlPortConfig {
                ref_net: "vref_out".into(),
                v_thresh_low: None,
                v_thresh_high: None,
                v_low: Some(0.0),
                v_high: Some(3.3),
            },
        );

        let extra_refs = blk.extra_ref_nets();
        assert_eq!(extra_refs, vec!["vref_in".to_string(), "vref_out".to_string()]);

        let subckt = blk.synthesize_subckt("HDL_CUSTOM_REF").expect("synth custom ref");
        let mut netlist = vec![
            "* Per-port ref & threshold test".to_string(),
            "Vref_in vref_in 0 DC 2.0".to_string(),
            "Vref_out vref_out 0 DC -2.0".to_string(),
            // `a` ramps 2.2V -> 3.8V over 20u (crosses 3.0V = vref_in + 1.0V at t=10u)
            "Va a 0 PWL(0 2.2 20u 3.8)".to_string(),
            "Vb b 0 DC 5.0".to_string(),
            "XU1 a b y_and y_xor vref_in vref_out HDL_CUSTOM_REF".to_string(),
            "R1 y_and 0 100k".to_string(),
            "R2 y_xor 0 100k".to_string(),
        ];
        netlist.extend(subckt);
        netlist.push(".tran 100n 20u".to_string());
        netlist.push(".end".to_string());

        let res = {
            let eng = NgSpiceEngine::global().unwrap().lock().unwrap();
            eng.run_netlist(&netlist)
        };
        assert!(res.success, "sim failed: {:?} log: {:?}", res.error, res.log);

        let y_and = res.traces.get("V(y_and)").expect("V(y_and)");
        let y_xor = res.traces.get("V(y_xor)").expect("V(y_xor)");
        let sample_at = |t_target: f64, trace: &[f64]| -> f64 {
            let idx = res
                .x_values
                .iter()
                .position(|&t| t >= t_target)
                .unwrap_or(trace.len() - 1);
            trace[idx]
        };
        // At t = 5u: a = 2.6V (< 3.0V => logic 0), b = 5V (logic 1)
        //   y_and = 0 -> vref_out + 0.0 = -2.0V
        //   y_xor = 1 -> 0 + 5.0 = 5.0V
        let y_and_5u = sample_at(5e-6, &y_and.real);
        let y_xor_5u = sample_at(5e-6, &y_xor.real);
        assert!((y_and_5u - (-2.0)).abs() < 0.1, "expected -2.0V, got {y_and_5u}");
        assert!((y_xor_5u - 5.0).abs() < 0.1, "expected 5.0V, got {y_xor_5u}");

        // At t = 15u: a = 3.4V (> 3.0V => logic 1), b = 5V (logic 1)
        //   y_and = 1 -> vref_out + 3.3 = +1.3V
        //   y_xor = 0 -> 0.0V
        let y_and_15u = sample_at(15e-6, &y_and.real);
        let y_xor_15u = sample_at(15e-6, &y_xor.real);
        assert!((y_and_15u - 1.3).abs() < 0.1, "expected +1.3V, got {y_and_15u}");
        assert!(y_xor_15u.abs() < 0.1, "expected 0.0V, got {y_xor_15u}");
    }

    #[test]
    fn test_hdl_wired_ref_pins_sim() {
        use crate::model::{component_pins_world, ComponentKind, Schematic};

        let mut blk = HdlBlock::default();
        blk.language = HdlLanguage::Verilog;
        blk.source = verilog_templates()[0].1.to_string(); // logic_gate(a, b, y_and, y_xor)
        blk.refresh_metadata();
        blk.ref_pins = vec!["REF_IN".into(), "REF_OUT".into()];
        blk.port_configs.insert(
            "a".into(),
            HdlPortConfig {
                ref_net: "REF_IN".into(),
                v_thresh_low: Some(0.8),
                v_thresh_high: Some(1.2),
                v_low: None,
                v_high: None,
            },
        );
        blk.port_configs.insert(
            "y_and".into(),
            HdlPortConfig {
                ref_net: "REF_OUT".into(),
                v_thresh_low: None,
                v_thresh_high: None,
                v_low: Some(0.0),
                v_high: Some(3.3),
            },
        );
        // REF_IN and REF_OUT are physical pins, so extra_ref_nets must be empty!
        assert!(blk.extra_ref_nets().is_empty());

        let sym = blk.build_symbol();
        assert_eq!(sym.pins.len(), 6); // a, b, y_and, y_xor, REF_IN, REF_OUT

        let mut sch = Schematic::new_empty("hdl_ref_pin_test.asc");
        let uid = sch.add_component(ComponentKind::Custom, "U1".into(), "logic_gate".into(), 20, 12);
        {
            let c = sch.components.iter_mut().find(|c| c.id == uid).unwrap();
            c.custom_symbol = Some(sym);
            c.hdl = Some(blk);
        }
        let pins: HashMap<String, (i32, i32)> = {
            let c = sch.components.iter().find(|c| c.id == uid).unwrap();
            component_pins_world(c)
                .into_iter()
                .map(|(n, x, y)| (n, (x, y)))
                .collect()
        };

        // Attach net labels directly to the HDL pins
        for (pname, lbl) in [
            ("a", "sig_a"),
            ("b", "sig_b"),
            ("y_and", "out_and"),
            ("y_xor", "out_xor"),
            ("REF_IN", "n_ref_in"),
            ("REF_OUT", "n_ref_out"),
        ] {
            let (px, py) = pins[pname];
            sch.add_component(ComponentKind::NetLabel, String::new(), lbl.into(), px, py);
        }

        // Add sources driving sig_a, sig_b, n_ref_in (2V), n_ref_out (-2V) and load resistors
        sch.add_directive(
            0,
            25,
            "Vref_in n_ref_in 0 DC 2.0\nVref_out n_ref_out 0 DC -2.0\nVa sig_a 0 PWL(0 2.2 20u 3.8)\nVb sig_b 0 DC 5.0\nR1 out_and 0 100k\nR2 out_xor 0 100k\n.tran 100n 20u".into(),
        );

        let net = sch.to_spice_netlist();
        let res = {
            let eng = NgSpiceEngine::global().unwrap().lock().unwrap();
            eng.run_netlist(&net.lines)
        };
        assert!(res.success, "sim failed: {:?} log: {:?}", res.error, res.log);

        let y_and = res.traces.get("V(out_and)").expect("V(out_and)");
        let sample_at = |t_target: f64, trace: &[f64]| -> f64 {
            let idx = res
                .x_values
                .iter()
                .position(|&t| t >= t_target)
                .unwrap_or(trace.len() - 1);
            trace[idx]
        };
        let v_5u = sample_at(5e-6, &y_and.real);
        let v_15u = sample_at(15e-6, &y_and.real);
        assert!((v_5u - (-2.0)).abs() < 0.1, "expected -2.0V at 5u, got {v_5u}");
        assert!((v_15u - 1.3).abs() < 0.1, "expected +1.3V at 15u, got {v_15u}");
    }
}
