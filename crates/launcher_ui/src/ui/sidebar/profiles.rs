use egui::Ui;

use crate::assets;
use crate::ui::components::icon_button;
use crate::ui::style;

use super::{ProfileShortcut, SidebarOutput};

pub fn render(
    ui: &mut Ui,
    profile_shortcuts: &[ProfileShortcut],
    output: &mut SidebarOutput,
    max_icon_width: f32,
) {
    if profile_shortcuts.is_empty() {
        return;
    }

    let row_height = max_icon_width.max(1.0);
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = style::SPACE_SM;
        for profile in profile_shortcuts {
            let icon_id = format!("user_profile_{}", profile.id);
            let response = ui
                .allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), row_height),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        icon_button::svg(
                            ui,
                            icon_id.as_str(),
                            assets::USER_SVG,
                            profile.name.as_str(),
                            false,
                            max_icon_width,
                        )
                    },
                )
                .inner;
            if response.clicked() {
                output.selected_profile_id = Some(profile.id.clone());
            }
        }
    });
}
