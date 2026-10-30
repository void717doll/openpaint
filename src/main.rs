#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod clipboard;
mod dialogs;
mod ui;

use macroquad::prelude::*;

fn window_conf() -> Conf {
    Conf {
        window_title: "openpaint".into(),
        window_width: 1280,
        window_height: 800,
        high_dpi: true,
        sample_count: 1,
        icon: window_icon(),
        ..Default::default()
    }
}

fn window_icon() -> Option<macroquad::miniquad::conf::Icon> {
    let image = image::load_from_memory(include_bytes!("../assets/openpaint.png"))
        .ok()?
        .to_rgba8();
    let scaled = |size| {
        image::imageops::resize(&image, size, size, image::imageops::FilterType::Lanczos3)
            .into_raw()
    };
    Some(macroquad::miniquad::conf::Icon {
        small: scaled(16).try_into().ok()?,
        medium: scaled(32).try_into().ok()?,
        big: scaled(64).try_into().ok()?,
    })
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("openpaint {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("openpaint {}\n\nusage: openpaint [image ...]\n\n--help     show help\n--version  show version", env!("CARGO_PKG_VERSION"));
        return;
    }
    macroquad::Window::from_config(window_conf(), run());
}

async fn run() {
    if let Ok(font) = load_ttf_font_from_bytes(include_bytes!("../assets/DejaVuSans.ttf")) {
        set_default_font(font);
    }
    let mut editor = app::Editor::new();
    prevent_quit();
    loop {
        editor.frame();
        if editor.quit {
            break;
        }
        next_frame().await;
    }
}
