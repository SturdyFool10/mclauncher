use eframe::{self, egui};
use egui::CentralPanel;
use fontloader::{FontCatalog, FontSpec, Slant, Stretch, Weight};

fn topbar_buttons() -> Vec<&'static str> {
    vec!["File", "Edit", "View", "Help"]
}

struct VertexApp {}

impl VertexApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut cat = FontCatalog::new();
        cat.load_system();

        let spec = FontSpec::new(&["Maple Mono NF"])
            .weight(Weight::REGULAR)
            .slant(Slant::Upright)
            .stretch(Stretch::Normal);

        if let Ok((bytes, _face_index)) = cat.query_bytes(&spec) {
            fontloader::egui_integration::install_font_as_primary(
                &cc.egui_ctx,
                "maple_mono_nf_regular",
                bytes,
                18.0,
            );
        } else {
            eprintln!("Maple Mono NF Regular not found; using egui default fonts.");
        }
        Self {}
    }
}

impl eframe::App for VertexApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        CentralPanel::default().show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                for button in topbar_buttons() {
                    let val = ui.button(button);
                    if val.clicked() {
                        println!("button {} clicked", button)
                    }
                }
            });
            ui.label("Hello Vertex Launcher")
        });
    }
}

fn main() -> eframe::Result<()> {
    let options: eframe::NativeOptions = eframe::NativeOptions {
        viewport: egui::ViewportBuilder {
            title: Some("Vertex Launcher".into()),
            inner_size: Some(egui::vec2(1280.0, 800.0)),
            min_inner_size: Some(egui::vec2(320.0, 240.0)),
            resizable: Some(true),
            ..Default::default()
        },
        renderer: eframe::Renderer::Wgpu,
        hardware_acceleration: eframe::HardwareAcceleration::Required,
        vsync: false,
        multisampling: 1,
        depth_buffer: 0,
        stencil_buffer: 0,
        dithering: false,
        centered: false,
        persist_window: false,
        event_loop_builder: None,
        window_builder: None,
        shader_version: None,
        run_and_return: false,
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            present_mode: eframe::egui_wgpu::wgpu::PresentMode::AutoNoVsync,
            desired_maximum_frame_latency: Some(1),
            wgpu_setup: eframe::egui_wgpu::WgpuSetup::CreateNew(
                eframe::egui_wgpu::WgpuSetupCreateNew {
                    instance_descriptor: eframe::egui_wgpu::wgpu::InstanceDescriptor {
                        backends: eframe::egui_wgpu::wgpu::Backends::VULKAN,
                        ..Default::default()
                    },
                    power_preference: eframe::egui_wgpu::wgpu::PowerPreference::LowPower,
                    ..Default::default()
                },
            ),
            on_surface_error: std::sync::Arc::new(|_| {
                eframe::egui_wgpu::SurfaceErrorAction::RecreateSurface
            }),
        },
        ..Default::default()
    };

    eframe::run_native(
        "Vertex Launcher",
        options,
        Box::new(|cc| Ok(Box::new(VertexApp::new(cc)))),
    )
}
