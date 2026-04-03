/// Self-contained shadows adjustment.
///
/// `value = 0.0` is neutral.
/// Positive values lift dark regions, negative values deepen dark regions.

use super::scalar;

pub struct ShadowsAdjustment {
    pub value: f32,
}

impl Default for ShadowsAdjustment {
    fn default() -> Self {
        Self { value: 0.0 }
    }
}

impl ShadowsAdjustment {
    pub const STEP: f32 = 0.01;
    pub const MIN: f32 = -1.0;
    pub const MAX: f32 = 1.0;

    pub fn is_neutral(&self) -> bool {
        scalar::is_neutral_value(self.value)
    }

    pub fn reset(&mut self) {
        self.value = 0.0;
    }

    pub fn adjust_by(&mut self, delta: f32) -> bool {
        scalar::apply_delta_clamped(&mut self.value, delta, Self::MIN, Self::MAX)
    }

    fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
        let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    fn apply_channel(c: f32, delta: f32) -> f32 {
        if delta >= 0.0 {
            c + (1.0 - c) * delta
        } else {
            c * (1.0 + delta)
        }
    }

    pub fn apply(&self, pixels: &mut [u8]) {
        if self.is_neutral() {
            return;
        }

        for chunk in pixels.chunks_exact_mut(4) {
            let r = chunk[0] as f32 / 255.0;
            let g = chunk[1] as f32 / 255.0;
            let b = chunk[2] as f32 / 255.0;

            let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            let weight = 1.0 - Self::smoothstep(0.0, 0.5, luma);
            let delta = self.value * weight;

            let nr = Self::apply_channel(r, delta).clamp(0.0, 1.0);
            let ng = Self::apply_channel(g, delta).clamp(0.0, 1.0);
            let nb = Self::apply_channel(b, delta).clamp(0.0, 1.0);

            chunk[0] = (nr * 255.0 + 0.5) as u8;
            chunk[1] = (ng * 255.0 + 0.5) as u8;
            chunk[2] = (nb * 255.0 + 0.5) as u8;
        }
    }
}
