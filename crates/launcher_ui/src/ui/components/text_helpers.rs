use egui::Ui;
use textui::{LabelOptions, TextUi};

pub fn normalize_inline_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn truncate_single_line_text_with_ellipsis(
    text_ui: &mut TextUi,
    ui: &Ui,
    text: &str,
    max_width: f32,
    label_options: &LabelOptions,
) -> String {
    let normalized = normalize_inline_whitespace(text);
    if normalized.is_empty() {
        return String::new();
    }

    if max_width <= 0.0 {
        return "...".to_owned();
    }

    let mut measure_width =
        |candidate: &str| -> f32 { text_ui.measure_text_size(ui, candidate, label_options).x };

    if measure_width(normalized.as_str()) <= max_width {
        return normalized;
    }

    let ellipsis = "...";
    if measure_width(ellipsis) > max_width {
        return String::new();
    }

    let mut cutoff = 0usize;
    for (index, _) in normalized
        .char_indices()
        .skip(1)
        .chain(std::iter::once((normalized.len(), '\0')))
    {
        let candidate = format!("{}{}", &normalized[..index], ellipsis);
        if measure_width(candidate.as_str()) <= max_width {
            cutoff = index;
        } else {
            break;
        }
    }

    if cutoff == 0 {
        ellipsis.to_owned()
    } else {
        format!("{}{}", &normalized[..cutoff], ellipsis)
    }
}
