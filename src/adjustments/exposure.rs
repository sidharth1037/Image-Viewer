/// Self-contained exposure adjustment.
///
/// `value = 0.0` is neutral (no change).
/// Positive values brighten, negative values darken.
/// Unit is in stops: each +1.0 doubles brightness.

use super::scalar;

pub struct ExposureAdjustment {
    pub value: f32,
}

impl Default for ExposureAdjustment {
    fn default() -> Self {
        Self { value: 0.0 }
    }
}

impl ExposureAdjustment {
    pub const STEP: f32 = 0.01;
    pub const MIN: f32 = -2.0;
    pub const MAX: f32 = 2.0;

    pub fn is_neutral(&self) -> bool {
        scalar::is_neutral_value(self.value)
    }

    pub fn reset(&mut self) {
        self.value = 0.0;
    }

    pub fn adjust_by(&mut self, delta: f32) -> bool {
        scalar::apply_delta_clamped(&mut self.value, delta, Self::MIN, Self::MAX)
    }

    fn build_lut(&self) -> [u8; 256] {
        let mut lut = [0u8; 256];
        let factor = 2.0_f32.powf(self.value);

        for i in 0..256 {
            let adjusted = (i as f32 * factor).clamp(0.0, 255.0);
            lut[i] = (adjusted + 0.5) as u8;
        }

        lut
    }

    pub fn apply(&self, pixels: &mut [u8]) {
        if self.is_neutral() {
            return;
        }

        let lut = self.build_lut();
        for chunk in pixels.chunks_exact_mut(4) {
            chunk[0] = lut[chunk[0] as usize];
            chunk[1] = lut[chunk[1] as usize];
            chunk[2] = lut[chunk[2] as usize];
        }
    }
}
