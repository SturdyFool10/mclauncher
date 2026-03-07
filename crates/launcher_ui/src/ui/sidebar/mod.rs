use egui::{Context, ScrollArea, SidePanel, Ui};

use crate::assets;
use crate::screens::AppScreen;
use crate::ui::components::icon_button;
use crate::ui::style;

mod app_nav;
mod profiles;

#[derive(Debug, Clone, Copy)]
struct SidebarLayout {
    nav_icon_width: f32,
}

#[derive(Debug, Clone)]
pub struct ProfileShortcut {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Default)]
pub struct SidebarOutput {
    pub selected_screen: Option<AppScreen>,
    pub selected_profile_id: Option<String>,
    pub create_instance_clicked: bool,
}

pub fn render(
    ctx: &Context,
    active_screen: AppScreen,
    profile_shortcuts: &[ProfileShortcut],
) -> SidebarOutput {
    let mut output = SidebarOutput::default();
    let viewport_width = ctx.input(|i| i.content_rect().width());
    let nav_icon_width = (viewport_width * 0.025).clamp(16.0, 40.0);
    let horizontal_padding = (viewport_width * 0.005).clamp(4.0, 12.0);
    let sidebar_width = nav_icon_width + (horizontal_padding * 2.0);
    let content_width = (sidebar_width - (horizontal_padding * 2.0)).max(1.0);
    let layout = SidebarLayout { nav_icon_width };

    SidePanel::left("task_bar_left")
        .resizable(false)
        .exact_width(sidebar_width)
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
        .show_separator_line(false)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let panel_rect = ui.max_rect();
            let content_rect = egui::Rect::from_min_max(
                egui::pos2(panel_rect.left() + horizontal_padding, panel_rect.top()),
                egui::pos2(panel_rect.right() - horizontal_padding, panel_rect.bottom()),
            );
            let _ = ui.allocate_rect(panel_rect, egui::Sense::hover());
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(content_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
                |ui| {
                    ui.set_width(content_width);
                    render_segments(ui, active_screen, profile_shortcuts, &mut output, layout);
                },
            );
        });

    output
}

fn render_segments(
    ui: &mut Ui,
    active_screen: AppScreen,
    profile_shortcuts: &[ProfileShortcut],
    output: &mut SidebarOutput,
    layout: SidebarLayout,
) {
    let row_height = layout.nav_icon_width.max(1.0);
    let nav_count = AppScreen::FIXED_NAV.len() as f32;
    let nav_stack_height =
        (nav_count * row_height) + ((nav_count - 1.0).max(0.0) * style::SPACE_SM);
    let divider_height = 1.0;
    let desired_top_height = style::SPACE_XS
        + nav_stack_height
        + style::SPACE_MD
        + row_height
        + style::SPACE_LG
        + divider_height
        + style::SPACE_MD;
    let full_rect = ui.available_rect_before_wrap();
    if full_rect.width() <= 0.0 || full_rect.height() <= 0.0 {
        return;
    }

    let min_bottom_height = row_height.max(8.0);
    let max_top_height = (full_rect.height() - min_bottom_height).max(0.0);
    let top_height = desired_top_height.min(max_top_height);
    let top_rect = egui::Rect::from_min_max(
        full_rect.min,
        egui::pos2(full_rect.max.x, full_rect.min.y + top_height),
    );
    let bottom_rect =
        egui::Rect::from_min_max(egui::pos2(full_rect.min.x, top_rect.max.y), full_rect.max);

    let _ = ui.allocate_rect(full_rect, egui::Sense::hover());

    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(top_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            ui.set_min_height(top_height);
            ui.add_space(style::SPACE_XS);
            app_nav::render(ui, active_screen, output, layout.nav_icon_width);

            ui.add_space(style::SPACE_MD);
            let create_response = ui
                .allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), row_height),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        icon_button::svg(
                            ui,
                            "create_instance",
                            assets::PLUS_SVG,
                            "Create instance",
                            false,
                            layout.nav_icon_width,
                        )
                    },
                )
                .inner;
            if create_response.clicked() {
                output.create_instance_clicked = true;
            }

            ui.add_space(style::SPACE_LG);
            let (divider_rect, _) = ui.allocate_exact_size(
                egui::vec2(ui.available_width().max(1.0), divider_height),
                egui::Sense::hover(),
            );
            ui.painter().hline(
                divider_rect.x_range(),
                divider_rect.center().y,
                egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
            );
            ui.add_space(style::SPACE_MD);
        },
    );

    if bottom_rect.height() > 0.0 {
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(bottom_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                ui.add_space(style::SPACE_LG);
                let scroll_height = ui.available_height().max(1.0);
                ScrollArea::vertical()
                    .id_salt("profiles_scroll_v4")
                    .auto_shrink([false, false])
                    .max_height(scroll_height)
                    .show(ui, |ui| {
                        profiles::render(ui, profile_shortcuts, output, layout.nav_icon_width)
                    });
            },
        );
    }
}
