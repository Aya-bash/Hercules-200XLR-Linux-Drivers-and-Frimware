mod app;
mod audio;
mod config;
mod bridge;
mod theme;
mod views;

use app::StreamControlApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Hercules Stream 200 XLR — Studio Control")
            .with_inner_size([1080.0, 690.0])
            .with_min_inner_size([940.0, 580.0])
            .with_active(true),
        ..Default::default()
    };

    eframe::run_native(
        "hsm200-gui",
        native_options,
        Box::new(|cc| Ok(Box::new(StreamControlApp::new(cc)))),
    )
}
