/// Self-contained gamma correction adjustment.
///
/// `value = 0.0` is neutral (no change).
/// Positive values brighten, negative values darken.
/// The correction formula: `output = 255 * (input/255)^(1 / (1 + value))`

use super::scalar;

pub struct GammaAdjustment {
    pub value: f32,
}

impl Default for GammaAdjustment {
    fn default() -> Self {
        Self { value: 0.0 }
    }
}

impl GammaAdjustment {
    /// Step size for keyboard increment/decrement.
    pub const STEP: f32 = 0.01;

    /// Safe lower bound so `1.0 + value` never approaches zero.
    pub const MIN: f32 = -0.95;
    /// Conservative upper bound to keep the control practical.
    pub const MAX: f32 = 4.0;

    /// Returns true if gamma is at the neutral (identity) value.
    pub fn is_neutral(&self) -> bool {
        scalar::is_neutral_value(self.value)
    }

    /// Resets gamma to the neutral value.
    pub fn reset(&mut self) {
        self.value = 0.0;
    }

    /// Applies a signed delta to gamma and clamps it to valid bounds.
    pub fn adjust_by(&mut self, delta: f32) -> bool {
        scalar::apply_delta_clamped(&mut self.value, delta, Self::MIN, Self::MAX)
    }

    /// Pre-computes a 256-entry lookup table for the current gamma value.
    /// Using a LUT avoids per-pixel `powf()` calls during application.
    fn build_lut(&self) -> [u8; 256] {
        let mut lut = [0u8; 256];
        // exponent = 1 / (1 + value)
        // value=0  → exp=1.0 (identity)
        // value>0  → exp<1.0 (brighter)
        // value<0  → exp>1.0 (darker)
        let exponent = 1.0 / (1.0 + self.value);

        for i in 0..256 {
            let normalized = i as f32 / 255.0;
            let corrected = normalized.powf(exponent);
            lut[i] = (corrected * 255.0 + 0.5) as u8; // +0.5 for rounding
        }
        lut
    }

    /// Applies gamma correction to an RGBA pixel buffer in-place.
    /// Only R, G, B channels are modified; alpha is left untouched.
    pub fn apply(&self, pixels: &mut [u8]) {
        if self.is_neutral() {
            return;
        }

        let lut = self.build_lut();

        // Process 4 bytes at a time: R, G, B, (skip A)
        for chunk in pixels.chunks_exact_mut(4) {
            chunk[0] = lut[chunk[0] as usize];
            chunk[1] = lut[chunk[1] as usize];
            chunk[2] = lut[chunk[2] as usize];
            // chunk[3] (alpha) is intentionally untouched
        }
    }
}
