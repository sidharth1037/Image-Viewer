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
use super::contrast::ContrastAdjustment;
use super::saturation::SaturationAdjustment;
use super::exposure::ExposureAdjustment;
use super::highlights::HighlightsAdjustment;
use super::shadows::ShadowsAdjustment;

#[derive(Clone, Copy)]
pub enum AdjustmentTarget {
    Saturation,
    Exposure,
    Highlights,
    Shadows,
    Contrast,
    Gamma,
}

pub struct AdjustmentPipeline {
    pub saturation: SaturationAdjustment,
    pub exposure: ExposureAdjustment,
    pub highlights: HighlightsAdjustment,
    pub shadows: ShadowsAdjustment,
    pub contrast: ContrastAdjustment,
    pub gamma: GammaAdjustment,
}

impl Default for AdjustmentPipeline {
    fn default() -> Self {
        Self {
            saturation: SaturationAdjustment::default(),
            exposure: ExposureAdjustment::default(),
            highlights: HighlightsAdjustment::default(),
            shadows: ShadowsAdjustment::default(),
            contrast: ContrastAdjustment::default(),
            gamma: GammaAdjustment::default(),
        }
    }
}

impl AdjustmentPipeline {
    /// Returns overlay text for a specific adjustment target.
    pub fn overlay_text_for(&self, target: AdjustmentTarget) -> String {
        match target {
            AdjustmentTarget::Saturation => format!("Saturation: {:+.2}", self.saturation.value),
            AdjustmentTarget::Exposure => format!("Exposure: {:+.2}", self.exposure.value),
            AdjustmentTarget::Highlights => format!("Highlights: {:+.2}", self.highlights.value),
            AdjustmentTarget::Shadows => format!("Shadows: {:+.2}", self.shadows.value),
            AdjustmentTarget::Contrast => format!("Contrast: {:+.2}", self.contrast.value),
            AdjustmentTarget::Gamma => format!("Gamma: {:+.2}", self.gamma.value),
        }
    }

    /// Returns true if any adjustment is non-neutral (i.e. something has been changed).
    pub fn has_adjustments(&self) -> bool {
        !self.saturation.is_neutral()
            || !self.exposure.is_neutral()
            || !self.highlights.is_neutral()
            || !self.shadows.is_neutral()
            || !self.contrast.is_neutral()
            || !self.gamma.is_neutral()
    }

    /// Resets every adjustment back to its default/neutral value.
    pub fn reset_all(&mut self) {
        self.saturation.reset();
        self.exposure.reset();
        self.highlights.reset();
        self.shadows.reset();
        self.contrast.reset();
        self.gamma.reset();
    }

    /// Takes the original pixel buffer, clones it, and applies all active
    /// adjustments in sequence. Returns the adjusted pixel data.
    ///
    /// Processing order matters for correctness:
    ///   1. Exposure
    ///   2. Contrast
    ///   3. Highlights
    ///   4. Shadows
    ///   5. Gamma
    ///   6. Saturation
    pub fn apply_all(&self, original_pixels: &[u8]) -> Vec<u8> {
        let mut pixels = original_pixels.to_vec();

        // Apply each adjustment in the correct order
        self.exposure.apply(&mut pixels);
        self.contrast.apply(&mut pixels);
        self.highlights.apply(&mut pixels);
        self.shadows.apply(&mut pixels);
        self.gamma.apply(&mut pixels);
        self.saturation.apply(&mut pixels);

        pixels
    }
}
