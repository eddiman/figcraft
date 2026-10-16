//! Approved development host: one eframe/winit event loop and the shared editor.
use std::error::Error;
use std::io::Cursor;

use crate::services::IPadServices;
use figcraft_color::{ImageScaleMode, Paint};
use figcraft_doc::{Document, NodeKind};
use figcraft_engine::Session;
use figcraft_ui_egui::FigApp;

/// The T01 small recipe, built with the upstream helper and an original generated image.
fn small_fixture() -> Result<Document, Box<dyn Error>> {
    figcraft_text::scan_system_fonts();
    let (mut doc, page) = figcraft_engine::synthetic::large_document(50)?;
    let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([24, 96, 168, 255]));
    let mut png = Cursor::new(Vec::new());
    image.write_to(&mut png, image::ImageFormat::Png)?;
    let (hash, _, _) = figcraft_format::add_image(&mut doc, png.into_inner())?;
    let id = doc.create(page, NodeKind::Rectangle, "Fixture image", 0.0, -64.0, 32.0, 32.0)?;
    if let Some(node) = doc.node_mut(id) {
        node.fills = vec![Paint::image(hash, ImageScaleMode::Fill)];
    }
    Ok(doc)
}

struct Shell {
    field: String,
    fit_requested: bool,
    smoke: bool,
    recovery: crate::recovery::Shared,
    _lifecycle: figcraft_apple_ipad::LifecycleHost,
}

impl eframe::App for Shell {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.painter().rect_filled(ui.max_rect(), 0.0, ui.visuals().panel_fill);
        ui.add_space(ui.input(|i| i.safe_area_insets().0.top).max(32.0));
        ui.label("Open imports a copy. Export creates a copy in Files.");
        ui.separator();
        let mut move_fixture = false;
        if self.smoke {
            ui.horizontal(|ui| {
                ui.label("iPad startup smoke");
                ui.label("Field:");
                ui.text_edit_singleline(&mut self.field);
                ui.label(format!("{} characters", self.field.chars().count()));
                move_fixture = ui.button("Set fixture X = 10").clicked();
            });
            ui.separator();
        }
        let ctx = ui.ctx().clone();
        figcraft_ui_egui::windows::with_app(|fig| {
            if move_fixture {
                let fixture = fig.session.doc().ok().and_then(|doc| doc.nodes().find(|node| node.name == "Fixture image").map(|node| node.id));
                if let Some(id) = fixture {
                    fig.run("node.set", serde_json::json!({"id": id, "props": {"x": 10.0}}));
                }
            }
            if !self.fit_requested {
                fig.run("view.zoomFit", serde_json::json!({}));
                self.fit_requested = true;
            }
            figcraft_ui_egui::windows::guard(fig, &ctx, |fig| fig.ui(ui));
            crate::recovery::try_with(&self.recovery, |coordinator| {
                if let Some(wait) = coordinator.tick(&fig.session, ctx.input(|i| i.time)) {
                    ctx.request_repaint_after(std::time::Duration::try_from_secs_f64(wait).unwrap_or(std::time::Duration::from_millis(50)));
                }
                if let Some(notice) = coordinator.take_notice() {
                    fig.toast(notice);
                }
            });
        });
    }
}

pub fn run() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let smoke = std::env::args().any(|arg| arg == "--smoke");
    let mut session = Session::empty();
    let coordinator = crate::recovery::Coordinator::open(&crate::recovery::support_dir()?.join("Recovery"), &mut session)?;
    let restored = coordinator.restored();
    if session.docs.is_empty() {
        if smoke {
            session.docs.extend(Session::with_document(small_fixture()?).docs);
            session.active = 0;
        } else {
            session.execute("file.new", &serde_json::json!({}))?;
        }
    }
    let recovery = std::sync::Arc::new(std::sync::Mutex::new(coordinator));
    let mut fig = FigApp::new(session, Box::new(IPadServices::new(recovery.clone())?));
    if restored > 0 {
        fig.toast("Recovered your unsaved work.");
    }
    fig.ui.home = false;
    figcraft_ui_egui::windows::install(fig);
    let mut options = eframe::NativeOptions { renderer: eframe::Renderer::Wgpu, ..Default::default() };
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(create) = &mut options.wgpu_options.wgpu_setup {
        let defaults = create.device_descriptor.clone();
        create.device_descriptor = std::sync::Arc::new(move |adapter| {
            let mut descriptor = defaults(adapter);
            // The simulator's Metal adapter exposes 15, while wgpu's default requests 16.
            descriptor.required_limits.max_inter_stage_shader_variables =
                descriptor.required_limits.max_inter_stage_shader_variables.min(adapter.limits().max_inter_stage_shader_variables);
            descriptor
        });
    }

    eframe::run_native(
        "FigCraft iPad",
        options,
        Box::new(move |cc| {
            let lifecycle = figcraft_apple_ipad::LifecycleHost::new(crate::lifecycle::callback(recovery.clone(), cc.egui_ctx.clone()))
                .map_err(std::io::Error::other)?;
            Ok(Box::new(Shell { field: String::new(), fit_requested: false, smoke, recovery, _lifecycle: lifecycle }))
        }),
    )?;
    Ok(())
}
