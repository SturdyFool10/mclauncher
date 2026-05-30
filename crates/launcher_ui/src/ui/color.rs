use colorlab::{ColorSpace, Oklab, Srgb};
use egui::Color32;

/// Convert an egui `Color32` (sRGB, un-premultiplied) to an `Oklab` value
/// using ColorLab's sRGB → linear-RGB → OKLab pipeline.
pub(crate) fn color32_to_oklab(c: Color32) -> Oklab {
    let srgb = Srgb {
        r: c.r() as f64 / 255.0,
        g: c.g() as f64 / 255.0,
        b: c.b() as f64 / 255.0,
        a: c.a() as f64 / 255.0,
    };
    Oklab::from_color(&srgb.to_color())
}

/// Convert an `Oklab` value back to an egui `Color32` using ColorLab's
/// OKLab → linear-RGB → sRGB pipeline.
pub(crate) fn oklab_to_color32(ok: Oklab) -> Color32 {
    let srgb = Srgb::from_color(&ok.to_color());
    Color32::from_rgba_unmultiplied(
        (srgb.r.clamp(0.0, 1.0) * 255.0) as u8,
        (srgb.g.clamp(0.0, 1.0) * 255.0) as u8,
        (srgb.b.clamp(0.0, 1.0) * 255.0) as u8,
        (srgb.a.clamp(0.0, 1.0) * 255.0) as u8,
    )
}

/// Linearly interpolate between two `Color32` values in OKLab space.
///
/// `t = 0.0` returns `a`, `t = 1.0` returns `b`.  Intermediate values
/// follow a perceptually uniform straight-line path, avoiding the muddy
/// grey midpoints that appear when mixing saturated colours in sRGB or
/// linear-light RGB.
pub(crate) fn lerp_color32_oklab(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0) as f64;
    let ok_a = color32_to_oklab(a);
    let ok_b = color32_to_oklab(b);
    oklab_to_color32(Oklab {
        l: ok_a.l + (ok_b.l - ok_a.l) * t,
        a: ok_a.a + (ok_b.a - ok_a.a) * t,
        b: ok_a.b + (ok_b.b - ok_a.b) * t,
        alpha: ok_a.alpha + (ok_b.alpha - ok_a.alpha) * t,
    })
}
