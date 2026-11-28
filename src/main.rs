mod analysis_dialog;
mod app;
mod capacitor_dialog;
mod hdl;
mod hdl_dialog;
mod icons;
mod mcp;
mod model;
mod mosfet_dialog;
mod ngspice;
mod python_terminal;
mod resistor_dialog;
mod schematic_ui;
mod settings;
mod source_dialog;
mod symbol_editor;
mod waveform_ui;

use std::sync::{Arc, Mutex};

use app::ItSpiceApp;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let is_mcp = args.iter().any(|a| a == "--mcp");
    let is_headless = args.iter().any(|a| a == "--headless");

    if is_mcp && is_headless {
        let mut srv = mcp::McpServer::new();
        if let Err(e) = srv.run_stdio() {
            eprintln!("MCP server error: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }

    let mcp_shared = if is_mcp {
        let shared = Arc::new(Mutex::new(mcp::McpSharedState::new()));
        let bg_shared = shared.clone();
        std::thread::spawn(move || {
            let mut srv = mcp::McpServer::with_shared(bg_shared);
            if let Err(e) = srv.run_stdio() {
                eprintln!("MCP server error: {e}");
            }
        });
        Some(shared)
    } else {
        None
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MCP_ITSpice - Draft1.asc")
            .with_icon(icons::app_window_icon())
            .with_inner_size([1280.0, 820.0]),
        ..Default::default()
    };

    eframe::run_native(
        "MCP_ITSpice",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::light());
            Ok(Box::new(ItSpiceApp::new(cc, mcp_shared)))
        }),
    )
}
