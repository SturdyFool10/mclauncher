use egui::Ui;
use textui::{LabelOptions, TextUi};

pub fn render(ui: &mut Ui, text_ui: &mut TextUi, selected_profile_id: Option<&str>) {
    let text_color = ui.visuals().text_color();
    let heading = LabelOptions {
        font_size: 30.0,
        line_height: 34.0,
        weight: 700,
        color: text_color,
        wrap: false,
        ..LabelOptions::default()
    };
    let body = LabelOptions {
        color: text_color,
        ..LabelOptions::default()
    };

    let _ = text_ui.label(ui, "library_heading", "Library", &heading);
    ui.add_space(8.0);
    let _ = text_ui.label(
        ui,
        "library_desc",
        "Manage installed content and versions here.",
        &body,
    );

    if let Some(profile_id) = selected_profile_id {
        ui.add_space(8.0);
        let _ = text_ui.label(
            ui,
            "library_profile_scope",
            &format!("Scoped to profile: {profile_id}"),
            &body,
        );
    }
}
