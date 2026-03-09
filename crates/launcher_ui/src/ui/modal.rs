use egui::{Color32, Context, CornerRadius, Frame, Id, Margin, Order, Rect, Stroke};

const MODAL_CORNER_RADIUS: u8 = 14;
const MODAL_INNER_MARGIN: i8 = 14;
const MODAL_SCRIM_ALPHA: u8 = 160;

pub fn show_scrim(ctx: &Context, id: impl std::hash::Hash, viewport_rect: Rect) {
    egui::Area::new(Id::new(id))
        .order(Order::Foreground)
        .fixed_pos(viewport_rect.min)
        .show(ctx, |ui| {
            ui.painter().rect_filled(
                viewport_rect,
                CornerRadius::ZERO,
                Color32::from_rgba_premultiplied(0, 0, 0, MODAL_SCRIM_ALPHA),
            );
        });
}

pub fn window_frame(ctx: &Context) -> Frame {
    let base = ctx.style().visuals.window_fill;
    Frame::new()
        .fill(Color32::from_rgba_premultiplied(
            base.r(),
            base.g(),
            base.b(),
            255,
        ))
        .stroke(Stroke::new(
            1.0,
            ctx.style().visuals.widgets.hovered.bg_stroke.color,
        ))
        .corner_radius(CornerRadius::same(MODAL_CORNER_RADIUS))
        .inner_margin(Margin::same(MODAL_INNER_MARGIN))
}
