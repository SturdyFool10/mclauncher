use egui::{
    self, Align, Context, CursorIcon, Layout, ResizeDirection, Sense, TopBottomPanel,
    ViewportCommand,
};
use textui::{LabelOptions, TextUi};

use crate::{assets, screens::AppScreen, ui::components::icon_button};

const TOP_BAR_HEIGHT: f32 = 38.0;
const CONTROL_SLOT_WIDTH: f32 = 20.0;
const CONTROL_ICON_MAX_WIDTH: f32 = 20.0;
const CONTROL_GAP: f32 = 7.0;
const CONTROL_GROUP_PADDING: f32 = 12.0;
const RESIZE_GRAB_THICKNESS: f32 = 6.0;

pub fn render(ctx: &Context, active_screen: AppScreen, text_ui: &mut TextUi) {
    TopBottomPanel::top("window_top_bar")
        .exact_height(TOP_BAR_HEIGHT)
        .resizable(false)
        .frame(
            egui::Frame::new()
                .fill(ctx.style().visuals.panel_fill)
                .inner_margin(egui::Margin::ZERO)
                .outer_margin(egui::Margin::ZERO)
                .stroke(egui::Stroke::new(
                    1.0,
                    ctx.style().visuals.widgets.noninteractive.bg_stroke.color,
                )),
        )
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let full_rect = ui.max_rect();
            let controls_width =
                (CONTROL_SLOT_WIDTH * 3.0) + (CONTROL_GAP * 2.0) + (CONTROL_GROUP_PADDING * 2.0);
            let controls_min_x = (full_rect.max.x - controls_width).max(full_rect.min.x);
            let drag_rect = egui::Rect::from_min_max(
                full_rect.min,
                egui::pos2(controls_min_x, full_rect.max.y),
            );
            let controls_rect = egui::Rect::from_min_max(
                egui::pos2(controls_min_x, full_rect.min.y),
                full_rect.max,
            );

            let drag_response = ui.interact(
                drag_rect,
                ui.id().with("top_bar_drag_region"),
                Sense::click_and_drag(),
            );
            if drag_response.drag_started() {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            }

            ui.scope_builder(egui::UiBuilder::new().max_rect(drag_rect), |ui| {
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.add_space(10.0);
                    let text_color = ui.visuals().text_color();
                    let title_style = LabelOptions {
                        font_size: 30.0,
                        line_height: 34.0,
                        weight: 700,
                        color: text_color,
                        wrap: false,
                        ..LabelOptions::default()
                    };
                    let _ = text_ui.label(ui, "topbar_title", "Minecraft Launcher", &title_style);
                    ui.add_space(12.0);
                    let mut section_style = LabelOptions {
                        font_size: 18.0,
                        line_height: 24.0,
                        wrap: false,
                        ..LabelOptions::default()
                    };
                    section_style.color = ui.visuals().weak_text_color();
                    let _ = text_ui.label(
                        ui,
                        ("topbar_screen", active_screen.label()),
                        active_screen.label(),
                        &section_style,
                    );
                });
            });

            ui.scope_builder(egui::UiBuilder::new().max_rect(controls_rect), |ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(CONTROL_GROUP_PADDING);
                    render_controls(ui, ctx);
                    ui.add_space(CONTROL_GROUP_PADDING);
                });
            });
        });
}

pub fn handle_window_resize(ctx: &Context) {
    let (content_rect, pointer_pos, primary_pressed, is_maximized, is_fullscreen) =
        ctx.input(|i| {
            (
                i.content_rect(),
                i.pointer.interact_pos(),
                i.pointer.primary_pressed(),
                i.viewport().maximized.unwrap_or(false),
                i.viewport().fullscreen.unwrap_or(false),
            )
        });

    if is_maximized || is_fullscreen {
        return;
    }

    let Some(pointer_pos) = pointer_pos else {
        return;
    };

    let left = pointer_pos.x <= content_rect.left() + RESIZE_GRAB_THICKNESS;
    let right = pointer_pos.x >= content_rect.right() - RESIZE_GRAB_THICKNESS;
    let top = pointer_pos.y <= content_rect.top() + RESIZE_GRAB_THICKNESS;
    let bottom = pointer_pos.y >= content_rect.bottom() - RESIZE_GRAB_THICKNESS;

    let direction = if top && left {
        Some(ResizeDirection::NorthWest)
    } else if top && right {
        Some(ResizeDirection::NorthEast)
    } else if bottom && left {
        Some(ResizeDirection::SouthWest)
    } else if bottom && right {
        Some(ResizeDirection::SouthEast)
    } else if top {
        Some(ResizeDirection::North)
    } else if bottom {
        Some(ResizeDirection::South)
    } else if left {
        Some(ResizeDirection::West)
    } else if right {
        Some(ResizeDirection::East)
    } else {
        None
    };

    if let Some(direction) = direction {
        ctx.set_cursor_icon(resize_cursor_icon(direction));
        if primary_pressed {
            ctx.send_viewport_cmd(ViewportCommand::BeginResize(direction));
        }
    }
}

fn render_controls(ui: &mut egui::Ui, ctx: &Context) {
    if render_control_button(ui, "close", assets::X_SVG, "Close").clicked() {
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }
    ui.add_space(CONTROL_GAP);

    let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    if is_maximized {
        if render_control_button(ui, "restore_down", assets::COPY_SVG, "Restore down").clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Maximized(false));
        }
    } else if render_control_button(ui, "maximize", assets::CHEVRON_UP_SVG, "Maximize").clicked() {
        ctx.send_viewport_cmd(ViewportCommand::Maximized(true));
    }
    ui.add_space(CONTROL_GAP);

    if render_control_button(ui, "minimize", assets::CHEVRON_DOWN_SVG, "Minimize").clicked() {
        ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
    }
}

fn resize_cursor_icon(direction: ResizeDirection) -> CursorIcon {
    match direction {
        ResizeDirection::North => CursorIcon::ResizeNorth,
        ResizeDirection::South => CursorIcon::ResizeSouth,
        ResizeDirection::East => CursorIcon::ResizeEast,
        ResizeDirection::West => CursorIcon::ResizeWest,
        ResizeDirection::NorthEast => CursorIcon::ResizeNorthEast,
        ResizeDirection::SouthEast => CursorIcon::ResizeSouthEast,
        ResizeDirection::NorthWest => CursorIcon::ResizeNorthWest,
        ResizeDirection::SouthWest => CursorIcon::ResizeSouthWest,
    }
}

fn render_control_button(
    ui: &mut egui::Ui,
    icon_id: &str,
    icon_bytes: &'static [u8],
    tooltip: &str,
) -> egui::Response {
    ui.allocate_ui_with_layout(
        egui::vec2(CONTROL_SLOT_WIDTH, ui.available_height()),
        Layout::left_to_right(Align::Center),
        |ui| {
            icon_button::svg(
                ui,
                icon_id,
                icon_bytes,
                tooltip,
                false,
                CONTROL_ICON_MAX_WIDTH,
            )
        },
    )
    .inner
}
