//! WireStudio — native WireASM IDE.
//!
//! Builds a graph, compiles it to real AArch64/x86-64 assembler, shows the
//! result, and can assemble + run it on the current host.

mod app;
mod canvas;
mod circuit;
mod editor;
mod i18n;
mod inspector;
mod settings;

use std::path::PathBuf;
use std::sync::Arc;

use app::WireStudio;

fn main() -> eframe::Result<()> {
    // Optional positional argument: a graph to open on startup.
    let initial = std::env::args()
        .skip(1)
        .find(|a| !a.starts_with('-'))
        .map(PathBuf::from);

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([1024.0, 640.0]),
        renderer: eframe::Renderer::Glow,
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "WireStudio",
        options,
        Box::new(move |cc| {
            install_fonts(&cc.egui_ctx);
            cc.egui_ctx.set_visuals(eframe::egui::Visuals::dark());
            Ok(Box::new(WireStudio::new(initial.clone())))
        }),
    )
}

/// Keep the built-in fonts, but append a system font that covers Cyrillic (and
/// other scripts) in case the default face misses a glyph.
fn install_fonts(ctx: &eframe::egui::Context) {
    let mut fonts = eframe::egui::FontDefinitions::default();
    let candidates = [
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/Library/Fonts/Arial Unicode.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/noto/NotoSans-Regular.ttf",
        "C:\\Windows\\Fonts\\segoeui.ttf",
        "C:\\Windows\\Fonts\\arial.ttf",
    ];
    for path in candidates {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        fonts.font_data.insert(
            "wire_fallback".to_owned(),
            Arc::new(eframe::egui::FontData::from_owned(bytes)),
        );
        for family in [
            eframe::egui::FontFamily::Proportional,
            eframe::egui::FontFamily::Monospace,
        ] {
            if let Some(list) = fonts.families.get_mut(&family) {
                list.push("wire_fallback".to_owned());
            }
        }
        break;
    }
    ctx.set_fonts(fonts);
}
