# MCP_ITSpice

A lightweight schematic capture and waveform viewer written in Rust (`egui`/`eframe`) inspired by a famous SPICE tool, interfaced directly with **`ngspice`** / **`libngspice`** and featuring a built-in **Model Context Protocol (MCP)** server with live GUI synchronization.

## Features

- **Schematic Editor**:
  - Classic light-gray (`#C0C0C0`) canvas with grid snapping, orthogonal wire routing, and junction dots.
  - Vector symbols for Resistors, Capacitors, Inductors, Diodes, Voltage/Current Sources, Op-Amps, NMOS/PMOS MOSFETs (with bulk arrow and body diode), NPN/PNP BJTs (with emitter arrow), Ground, Net Labels, and custom user symbols.
  - Automatic wire cutting when placing or moving 2-pin components onto an existing wire.
  - Independent dragging for component labels (`refdes`), values, and net labels; clicking a component's name or value selects and highlights the component.
  - Multi-selection (marquee box drag & `Shift+Click`), group Move/Delete/Rotate/Mirror, Undo/Redo, and `Ctrl+C` / `Ctrl+X` / `Ctrl+V` clipboard support.
  - Standard `.asc` import/export and JSON project save/load.
- **Symbol Workbench (`File -> New Symbol`)**:
  - Dedicated resizable workbench window for creating custom symbols with pins (preserving SPICE pin order), lines, rectangles, circles, and text.
  - Full `.asy` and `.sym.json` import/export, plus one-click placement onto the schematic.
- **Waveform Viewer & Split Views**:
  - Black (`#000000`) plot area with SI-formatted axes, dashed grid, multi-trace color legend, drag-box zoom, scroll-wheel zoom, and instantaneous hover readout.
  - **Separated Voltage, Current & Power Probing**:
    - **Select / Probe Voltage (`V`)**: Click any wire or net node to plot `V(node)`.
    - **Probe Current (`I`)**: Click any component body or terminal pin to plot `I(r1)`, `I(c1)`, `I(l1)`, `I(d1)`, `I(v1)`, MOSFET `Id`/`Ig`/`Is`, BJT `Ic`/`Ib`/`Ie`, Op-Amp `I(u:pin)`, or custom subcircuit pin currents (using automatic `.save` directives and `0 V` series current-sense sources for subcircuits).
    - **Power Probe (`Alt+Click`)**: Hold `Alt` and click any simulated component to compute and plot instantaneous power `P(comp)` in Watts (`W`).
  - **Same-Graph Dual Y-Axis & Multi-Graph**: Traces share the same graph by default (with left `V` and right `A`/`W` Y-axes when mixed). Click **`+ Graph`** (or right-click → **New Graph**) to create additional stacked plot panes.
  - **Interactive Cursors & Temporal Distance**: Click trace names in the legend or right-click on the plot (**Add Fixed Cursor**) to pin fixed cursors (`C1`, `C2`, `C3+`), drag them horizontally, and measure `Δt` / `1/Δt` (Hz) and `ΔV`/`ΔI` between `C1` and `C2` (`Esc` clears cursors).
  - **Horizontal (`Split H`)** and **Vertical (`Split V`)** split views with draggable splitters and `-` / `+` pane size controls.
- **Python Analysis Terminal (`🐍 Python` Tab)**:
  - Dedicated interactive `python3` REPL tab connected to your system Python (or a configurable Python/virtualenv path in **File → Settings...**).
  - Automatically syncs simulation data (`x`, `x_name`, `traces`, `netlist`) and provides helpers: `tr(name)`, `rms(s)`, `avg(s)`, `pp(s)`, `info()`, `reload()`.
  - Write computed waveforms directly back to the viewer: `plot(name, y)` updates the integrated graph, while `new_plot(name, y)` / `new_graph()` creates a new plot pane.
- **SPICE Configuration Dialogs**:
  - **Independent Voltage / Current Source**: `(none)`, `PULSE`, `SINE`, `EXP`, `SFFM`, `PWL`, `PWL FILE`, DC value, Small-signal AC, and Parasitic properties.
  - **Capacitor Properties**: Manufacturer/Part/Type metadata, Capacitance, Voltage/RMS Current ratings, ESR, ESL, Rpar, Cpar, and built-in capacitor preset library.
  - **MOSFET Properties**: Model name, Manufacturer, Polarity (`N-chan`/`P-chan`), Vds, Rds(on), Qgate, and built-in NMOS/PMOS preset library.
  - **Configure Analysis**: `Transient`, `AC Analysis`, `DC sweep`, `Noise`, `DC Transfer`, and `DC op pnt` with live SPICE command preview.
- **`ngspice` & Python Integration (`File -> Settings...`)**:
  - Configurable `ngspice` executable or `libngspice.so.0` shared library path and configurable `python3` / virtualenv executable path (with auto-detection and file browser).
  - Configurable `ngspice` default model folders and user model folders (searched first), automatically indexing `.model` and `.subckt` definitions across `.lib`, `.mod`, `.mdl`, `.sub`, `.cir`, `.sp`, `.inc`, and `.txt` files.
  - Non-blocking background simulation execution, netlist viewer, and `ngspice` output log viewer.
- **Built-in MCP Server (`--mcp`)**:
  - JSON-RPC 2.0 over `stdin`/`stdout` with live GUI synchronization (or `--mcp --headless` for headless automation).

## Requirements & Dependencies

### 1. Rust Toolchain
Install `rustc` and `cargo` via [rustup](https://rustup.rs/):
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
```

### 2. System Libraries & `ngspice`
`MCP_ITSpice` uses `eframe`/`egui` (OpenGL + X11/Wayland), `rfd` (GTK3 file dialogs), and `libngspice` (or `ngspice` CLI) at runtime:

- **Ubuntu / Debian**:
  ```bash
  sudo apt update
  sudo apt install -y build-essential pkg-config libngspice0 ngspice \
    libgtk-3-dev libx11-dev libxkbcommon-dev libwayland-dev libgl-dev
  ```
- **Fedora**:
  ```bash
  sudo dnf install -y gcc pkg-config ngspice ngspice-libs \
    gtk3-devel libX11-devel libxkbcommon-devel wayland-devel mesa-libGL-devel
  ```
- **Arch Linux**:
  ```bash
  sudo pacman -S --needed base-devel pkgconf ngspice \
    gtk3 libx11 libxkbcommon wayland libglvnd
  ```

## Build & Run

### Build from Source
```bash
git clone <your-repo-url> MCP_ITSpice
cd MCP_ITSpice
cargo build --release
```
The compiled binary will be at `target/release/MCP_ITSpice` (or `target/debug/MCP_ITSpice` for debug builds via `cargo build`).

### Run the GUI
```bash
./run.sh
```
Or with Cargo directly:
```bash
cargo run --release
```

### Run Tests
```bash
cargo test
```

## Keyboard Shortcuts

| Key | Action |
| --- | --- |
| `W` | Draw Wire |
| `V` | Select / Probe Voltage (`V(node)`) |
| `I` | Probe Current (`I(comp)` / `Id` / `Ic` / pin currents) |
| `Alt+Click` | Compute & probe component instantaneous power (`P(comp)`) |
| `G` | Place Ground (`0`) |
| `R` | Place Resistor |
| `C` | Place Capacitor |
| `L` | Place Inductor |
| `D` | Place Diode |
| `P` | Open Component Picker |
| `N` | Place Net Label |
| `Ctrl+R` | Rotate placement ghost or selected components |
| `E` | Mirror placement ghost or selected components |
| `Ctrl+Z` | Undo |
| `Ctrl+Y` / `Ctrl+Shift+Z` | Redo |
| `Ctrl+C` / `Ctrl+X` / `Ctrl+V` | Copy / Cut / Paste selection |
| `Delete` / `Backspace` | Delete selected items |
| `Esc` | Clear selection / cancel current tool |
| `Alt+Drag` / `Middle-Drag` | Pan schematic |
| `Scroll` | Zoom schematic |

## MCP Server

MCP_ITSpice includes a built-in MCP server (`MCP_ITSpice`) that allows AI assistants to build schematics, edit components, run `ngspice` simulations, and inspect waveform traces.

- **Live GUI + MCP**: `./run_mcp.sh` (opens the GUI window and updates it in real time as MCP tool calls arrive)
- **Headless MCP**: `./run_mcp.sh --headless`

### Client Configuration Example
```json
{
  "mcpServers": {
    "MCP_ITSpice": {
      "command": "/path/to/MCP_ITSpice/run_mcp.sh",
      "args": []
    }
  }
}
```
*(Add `"--headless"` to `"args"` if you want headless mode without a GUI window.)*

### Exposed MCP Tools
- `get_schematic`, `new_schematic`, `load_schematic`, `save_schematic`
- `add_component`, `edit_component`, `remove_component`, `add_wire`, `set_directive`
- `run_simulation`, `run_spice_netlist`, `get_trace`

## Project Structure

- `src/main.rs` — Entry point, GUI / `--mcp` / `--headless` mode dispatch
- `src/app.rs` — Main application shell, menus, toolbar, split-pane (`Split H` / `Split V`) layout, settings UI, and MCP live sync
- `src/model.rs` — Schematic data model, union-find SPICE netlist generator, `.asc` parser/exporter, and wire splitting
- `src/ngspice.rs` — `libloading` FFI wrapper for `libngspice.so.0` and `ngspice` CLI runner
- `src/settings.rs` — Persistent `ngspice` executable and default/user model folder indexing
- `src/schematic_ui.rs` — Interactive schematic canvas, component symbols, marquee selection, label dragging, and clipboard
- `src/waveform_ui.rs` — Black-background waveform plotter with dual Y-axes, multi-graph panes, fixed cursors, `Δt` readout, and SI formatting
- `src/python_terminal.rs` — Interactive system Python (`python3`) analysis REPL tab with simulation sync and `plot()` / `new_plot()` waveform export
- `src/symbol_editor.rs` — Resizable Symbol Workbench with `.asy` / `.sym.json` support
- `src/source_dialog.rs` — Independent Voltage/Current Source configuration dialog
- `src/capacitor_dialog.rs` — Capacitor Properties dialog and ngspice library/parametric configuration
- `src/resistor_dialog.rs` — Resistor Properties dialog and ngspice library/parametric configuration
- `src/mosfet_dialog.rs` — MOSFET Properties dialog and preset library
- `src/analysis_dialog.rs` — Configure Analysis (`.tran`, `.ac`, `.dc`, `.noise`, `.tf`, `.op`) dialog
- `src/icons.rs` — Vector toolbar icons and window icon
- `src/mcp.rs` — JSON-RPC 2.0 MCP server and shared GUI state
