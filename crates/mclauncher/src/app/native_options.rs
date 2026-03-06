use config::Config;
use eframe::{self, egui};

pub fn build(startup_config: &Config) -> eframe::NativeOptions {
    let startup_power_preference = if startup_config.low_power_gpu_preferred() {
        eframe::egui_wgpu::wgpu::PowerPreference::LowPower
    } else {
        eframe::egui_wgpu::wgpu::PowerPreference::HighPerformance
    };

    eframe::NativeOptions {
        viewport: egui::ViewportBuilder {
            title: Some("Vertex Launcher".into()),
            inner_size: Some(egui::vec2(1280.0, 800.0)),
            min_inner_size: Some(egui::vec2(320.0, 240.0)),
            resizable: Some(true),
            decorations: Some(false),
            transparent: Some(startup_config.window_blur_enabled()),
            ..Default::default()
        },
        renderer: eframe::Renderer::Wgpu,
        hardware_acceleration: eframe::HardwareAcceleration::Required,
        vsync: false,
        multisampling: 4,
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
                    power_preference: startup_power_preference,
                    ..Default::default()
                },
            ),
            on_surface_error: std::sync::Arc::new(|_| {
                eframe::egui_wgpu::SurfaceErrorAction::RecreateSurface
            }),
        },
        ..Default::default()
    }
}
