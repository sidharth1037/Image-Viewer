/// The adjustment pipeline orchestrator.
///
/// Owns all individual adjustment modules and provides a single entry point
/// (`apply_all`) for applying every active adjustment to a pixel buffer.
///
/// To add a new adjustment type:
/// 1. Create its file in `src/adjustments/` (e.g. `contrast.rs`)
/// 2. Add a `pub` field here (e.g. `pub contrast: ContrastAdjustment`)
/// 3. Wire it into `has_adjustments()`, `reset_all()`, and `apply_all()`

use super::gamma::GammaAdjustment;

pub struct AdjustmentPipeline {
    pub gamma: GammaAdjustment,
    // Future: pub contrast: ContrastAdjustment,
    // Future: pub exposure: ExposureAdjustment,
    // Future: pub saturation: SaturationAdjustment,
}

impl Default for AdjustmentPipeline {
    fn default() -> Self {
        Self {
            gamma: GammaAdjustment::default(),
        }
    }
}

impl AdjustmentPipeline {
    /// Returns true if any adjustment is non-neutral (i.e. something has been changed).
    pub fn has_adjustments(&self) -> bool {
        !self.gamma.is_neutral()
        // Future: || !self.contrast.is_neutral()
    }

    /// Resets every adjustment back to its default/neutral value.
    pub fn reset_all(&mut self) {
        self.gamma.reset();
        // Future: self.contrast.reset();
    }

    /// Takes the original pixel buffer, clones it, and applies all active
    /// adjustments in sequence. Returns the adjusted pixel data.
    ///
    /// Processing order matters for correctness:
    ///   1. Exposure (future)
    ///   2. Contrast (future)
    ///   3. Gamma
    ///   4. Saturation (future)
    pub fn apply_all(&self, original_pixels: &[u8]) -> Vec<u8> {
        let mut pixels = original_pixels.to_vec();

        // Apply each adjustment in the correct order
        self.gamma.apply(&mut pixels);
        // Future: self.contrast.apply(&mut pixels);

        pixels
    }
}
