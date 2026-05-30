/// Duration of a toggle transition in seconds.
pub(crate) const TOGGLE_ANIM_SECS: f32 = 0.18;

/// Cubic ease-out: rapid onset, smooth deceleration into the end state.
/// `t` must be in [0, 1]; values outside that range are clamped.
pub(crate) fn cubic_ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Per-toggle animation state persisted in egui's transient data map.
///
/// Stores a `from` position, a `to` position, and the time elapsed into the
/// current transition.  Visual position is derived on demand by applying
/// cubic ease-out, so the stored data is always compact and cheap to copy.
#[derive(Clone)]
pub(crate) struct ToggleAnimState {
    /// Visual position when the current animation started (0 = off, 1 = on).
    pub(crate) from: f32,
    /// Where the animation is headed.
    pub(crate) to: f32,
    /// Seconds elapsed into the current animation.
    pub(crate) elapsed: f32,
}

impl ToggleAnimState {
    /// Create state already settled at `pos` with no pending animation.
    pub(crate) fn settled(pos: f32) -> Self {
        Self { from: pos, to: pos, elapsed: TOGGLE_ANIM_SECS }
    }

    /// Current visual position in [0, 1] after applying cubic ease-out.
    pub(crate) fn visual_pos(&self) -> f32 {
        let t = (self.elapsed / TOGGLE_ANIM_SECS).clamp(0.0, 1.0);
        self.from + (self.to - self.from) * cubic_ease_out(t)
    }

    /// `true` once the animation has run to completion.
    pub(crate) fn is_done(&self) -> bool {
        self.elapsed >= TOGGLE_ANIM_SECS
    }

    /// Advance the animation clock by `dt` seconds.
    pub(crate) fn advance(&mut self, dt: f32) {
        self.elapsed += dt;
    }

    /// Redirect the animation toward `new_target`.  If the target changed,
    /// the animation restarts from the current visual position so a mid-flight
    /// reversal is seamless rather than jumping.
    pub(crate) fn redirect(&mut self, new_target: f32) {
        if (self.to - new_target).abs() > f32::EPSILON {
            self.from = self.visual_pos();
            self.to = new_target;
            self.elapsed = 0.0;
        }
    }
}
