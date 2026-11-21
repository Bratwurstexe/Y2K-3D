#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod claude;
mod i18n;
mod icons;
mod labs;
mod library;
mod mesh;
mod noise;
mod picture;
mod render;
mod scene;
mod settings;
mod terrain;
mod wire;

use std::path::PathBuf;

fn app_icon() -> egui::IconData {
    let n = 64u32;
    let mut rgba = Vec::with_capacity((n * n * 4) as usize);
    for y in 0..n {
        for x in 0..n {
            let fx = x as f32 / n as f32;
            let fy = y as f32 / n as f32;
            let peak1 = 0.25 + (fx - 0.38).abs() * 1.3;
            let peak2 = 0.45 + (fx - 0.72).abs() * 1.1;
            let ground = peak1.min(peak2).min(0.8);
            let c: [u8; 4] = if fy > ground {
                if fy < ground + 0.08 && fy < 0.45 {
                    [240, 240, 245, 255]
                } else {
                    [
                        (150.0 - fy * 60.0) as u8,
                        (110.0 - fy * 40.0) as u8,
                        (70.0 - fy * 20.0) as u8,
                        255,
                    ]
                }
            } else {
                let d = ((fx - 0.78).powi(2) + (fy - 0.2).powi(2)).sqrt();
                if d < 0.09 {
                    [255, 205, 90, 255]
                } else {
                    [
                        (60.0 + fy * 150.0) as u8,
                        (100.0 + fy * 110.0) as u8,
                        (170.0 + fy * 60.0) as u8,
                        255,
                    ]
                }
            };
            rgba.extend_from_slice(&c);
        }
    }
    egui::IconData {
        rgba,
        width: n,
        height: n,
    }
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    // command-line rendering: --render <scene.l3d | demo> <picture.png>
    if let Some(i) = args.iter().position(|a| a == "--render") {
        if let (Some(src), Some(out)) = (args.get(i + 1), args.get(i + 2)) {
            let scene = if src == "demo" {
                scene::Scene::demo()
            } else if src == "demo-landscape" {
                scene::Scene::demo_landscape()
            } else {
                match std::fs::read_to_string(src)
                    .ok()
                    .and_then(|s| serde_json::from_str(&s).ok())
                {
                    Some(s) => s,
                    None => {
                        eprintln!("Szene nicht lesbar: {src}");
                        std::process::exit(1);
                    }
                }
            };
            let mut scene = scene;
            if let Some(sz) = args
                .iter()
                .position(|a| a == "--size")
                .and_then(|k| args.get(k + 1))
            {
                if let Some((w, h)) = sz.split_once('x') {
                    scene.render.width = w.parse().unwrap_or(scene.render.width);
                    scene.render.height = h.parse().unwrap_or(scene.render.height);
                }
            }
            if let Some(name) = args
                .iter()
                .position(|a| a == "--sky")
                .and_then(|k| args.get(k + 1))
            {
                if let Some(sky) = scene::sky_presets()
                    .into_iter()
                    .find(|s| s.name.eq_ignore_ascii_case(name))
                {
                    scene.sky = sky;
                }
            }
            let t = std::time::Instant::now();
            let img = render::render_frame(&scene);
            let ok = img.save(out).is_ok();
            eprintln!(
                "{} in {:.1} s",
                if ok {
                    "gerendert"
                } else {
                    "Fehler beim Speichern"
                },
                t.elapsed().as_secs_f32()
            );
            std::process::exit(if ok { 0 } else { 1 });
        }
    }
    let screenshot = args
        .iter()
        .position(|a| a == "--screenshot")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Y2K-3D")
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([1000.0, 640.0])
        .with_icon(std::sync::Arc::new(app_icon()));
    if screenshot.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }
    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    #[cfg(target_os = "macos")]
    if screenshot.is_some() {
        options.event_loop_builder = Some(Box::new(|b| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            b.with_activate_ignoring_other_apps(false);
        }));
    }
    eframe::run_native(
        "Y2K-3D",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, screenshot)))),
    )
}
