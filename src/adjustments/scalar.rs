/// Shared helpers for scalar-style adjustments (gamma, contrast, etc.).

/// Returns true if a scalar adjustment value is effectively neutral.
pub fn is_neutral_value(value: f32) -> bool {
    value.abs() < f32::EPSILON
}

/// Applies a delta with clamping and reports whether the value changed.
pub fn apply_delta_clamped(value: &mut f32, delta: f32, min: f32, max: f32) -> bool {
    let next = (*value + delta).clamp(min, max);
    let changed = (next - *value).abs() > f32::EPSILON;
    *value = next;
    changed
}
