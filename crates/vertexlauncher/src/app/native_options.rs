use config::Config;
use eframe::{self, egui};
use std::sync::Arc;

use super::{app_icon, app_metadata};

pub fn build(startup_config: &Config) -> eframe::NativeOptions {
    let startup_power_preference = if startup_config.low_power_gpu_preferred() {
        eframe::egui_wgpu::wgpu::PowerPreference::LowPower
    } else {
        eframe::egui_wgpu::wgpu::PowerPreference::HighPerformance
    };

    eframe::NativeOptions {
        viewport: egui::ViewportBuilder {
            title: Some("Vertex Launcher".into()),
            app_id: Some("vertexlauncher".into()),
            inner_size: Some(egui::vec2(1280.0, 800.0)),
            min_inner_size: Some(egui::vec2(900.0, 460.0)),
            resizable: Some(true),
            decorations: Some(false),
            transparent: Some(startup_config.window_blur_enabled()),
            icon: app_icon::egui_icon(),
            ..Default::default()
        },
        renderer: eframe::Renderer::Wgpu,
        hardware_acceleration: eframe::HardwareAcceleration::Required,
        vsync: false,
        multisampling: 4,
        depth_buffer: 32,
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
                    native_adapter_selector: None,
                    device_descriptor: Arc::new(|adapter| {
                        let info = adapter.get_info();
                        app_metadata::record_graphics_adapter(
                            &info.name,
                            &info.driver,
                            &info.driver_info,
                        );
                        tracing::info!(
                            target: "vertexlauncher/app/graphics",
                            "Selected graphics adapter: {} backend={:?} type={:?} vendor=0x{:04x} device=0x{:04x}",
                            info.name,
                            info.backend,
                            info.device_type,
                            info.vendor,
                            info.device
                        );

                        let base_limits = if info.backend == eframe::egui_wgpu::wgpu::Backend::Gl {
                            eframe::egui_wgpu::wgpu::Limits::downlevel_webgl2_defaults()
                        } else {
                            eframe::egui_wgpu::wgpu::Limits::default()
                        };

                        eframe::egui_wgpu::wgpu::DeviceDescriptor {
                            label: Some("egui wgpu device"),
                            required_limits: eframe::egui_wgpu::wgpu::Limits {
                                max_texture_dimension_2d: 8192,
                                ..base_limits
                            },
                            ..Default::default()
                        }
                    }),
                },
            ),
            on_surface_error: std::sync::Arc::new(|_| {
                eframe::egui_wgpu::SurfaceErrorAction::RecreateSurface
            }),
        },
        ..Default::default()
    }
}
