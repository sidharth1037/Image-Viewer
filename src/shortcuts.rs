use eframe::egui;

#[derive(Clone, Copy)]
pub struct Shortcut {
    pub key: egui::Key,
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub command: bool,
}

impl Shortcut {
    pub const fn new(
        key: egui::Key,
        alt: bool,
        ctrl: bool,
        shift: bool,
        command: bool,
    ) -> Self {
        Self {
            key,
            alt,
            ctrl,
            shift,
            command,
        }
    }

    fn modifiers_match(&self, modifiers: egui::Modifiers) -> bool {
        // `modifiers.command` may mirror Ctrl on non-macOS platforms.
        // Keep strict matching for Alt/Ctrl/Shift, but avoid rejecting
        // Ctrl shortcuts just because `command` is also true.
        let command_matches = if self.command {
            modifiers.command
        } else if self.ctrl {
            true
        } else {
            !modifiers.command
        };

        self.alt == modifiers.alt
            && self.ctrl == modifiers.ctrl
            && self.shift == modifiers.shift
            && command_matches
    }

    fn modifiers_match_with_shift_override(&self, modifiers: egui::Modifiers, shift: bool) -> bool {
        let command_matches = if self.command {
            modifiers.command
        } else if self.ctrl {
            true
        } else {
            !modifiers.command
        };

        self.alt == modifiers.alt
            && self.ctrl == modifiers.ctrl
            && shift == modifiers.shift
            && command_matches
    }

    pub fn is_pressed(&self, input: &egui::InputState) -> bool {
        input.key_pressed(self.key) && self.modifiers_match(input.modifiers)
    }

    pub fn is_held(&self, input: &egui::InputState) -> bool {
        input.key_down(self.key) && self.modifiers_match(input.modifiers)
    }

    /// Returns speed multiplier for adjustment-style shortcuts.
    /// 10.0 for exact match, 1.0 for Shift-augmented match when base shortcut doesn't use Shift.
    pub fn pressed_step_multiplier(&self, input: &egui::InputState) -> Option<f32> {
        if input.key_pressed(self.key) && self.modifiers_match(input.modifiers) {
            return Some(10.0);
        }

        let shifted_symbol_pressed = shifted_symbol_for_key(self.key).is_some_and(|symbol| {
            input.events.iter().any(|event| {
                if let egui::Event::Text(text) = event {
                    text.chars().count() == 1 && text.starts_with(symbol)
                } else {
                    false
                }
            })
        });

        if !self.shift
            && (input.key_pressed(self.key) || shifted_symbol_pressed)
            && self.modifiers_match_with_shift_override(input.modifiers, true)
        {
            return Some(1.0);
        }

        None
    }
}

fn shifted_symbol_for_key(key: egui::Key) -> Option<char> {
    match key {
        egui::Key::Num0 => Some(')'),
        egui::Key::Num1 => Some('!'),
        egui::Key::Num2 => Some('@'),
        egui::Key::Num3 => Some('#'),
        egui::Key::Num4 => Some('$'),
        egui::Key::Num5 => Some('%'),
        egui::Key::Num6 => Some('^'),
        egui::Key::Num7 => Some('&'),
        egui::Key::Num8 => Some('*'),
        egui::Key::Num9 => Some('('),
        _ => None,
    }
}

#[derive(Clone, Copy)]
pub struct ShortcutConfig {
    pub navigate_next: Shortcut,
    pub navigate_prev: Shortcut,
    pub jump_to_start: Shortcut,
    pub jump_to_end: Shortcut,
    pub cycle_sort_method_prev: Shortcut,
    pub cycle_sort_method_next: Shortcut,
    pub sort_ascending: Shortcut,
    pub sort_descending: Shortcut,
    pub toggle_settings: Shortcut,
    pub toggle_search: Shortcut,
    pub toggle_toolbar: Shortcut,
    pub reveal_in_explorer: Shortcut,
    pub delete_current_file_permanently: Shortcut,
    pub overwrite_with_adjustments: Shortcut,
    pub reload_current_context: Shortcut,
    pub rotate_clockwise: Shortcut,
    pub close_window: Shortcut,
    pub contrast_decrease: Shortcut,
    pub contrast_increase: Shortcut,
    pub gamma_decrease: Shortcut,
    pub gamma_increase: Shortcut,
    pub saturation_decrease: Shortcut,
    pub saturation_increase: Shortcut,
    pub exposure_decrease: Shortcut,
    pub exposure_increase: Shortcut,
    pub highlights_decrease: Shortcut,
    pub highlights_increase: Shortcut,
    pub shadows_decrease: Shortcut,
    pub shadows_increase: Shortcut,
    pub reset_adjustments: Shortcut,
    pub show_original_hold: Shortcut,
    pub clear_active_view: Shortcut,
}

impl Default for ShortcutConfig {
    fn default() -> Self {
        Self {
            navigate_next: Shortcut::new(egui::Key::ArrowRight, false, false, false, false),
            navigate_prev: Shortcut::new(egui::Key::ArrowLeft, false, false, false, false),
            jump_to_start: Shortcut::new(egui::Key::J, false, true, false, false),
            jump_to_end: Shortcut::new(egui::Key::J, false, true, true, false),
            cycle_sort_method_prev: Shortcut::new(egui::Key::ArrowLeft, false, true, false, false),
            cycle_sort_method_next: Shortcut::new(egui::Key::ArrowRight, false, true, false, false),
            sort_ascending: Shortcut::new(egui::Key::ArrowUp, false, true, false, false),
            sort_descending: Shortcut::new(egui::Key::ArrowDown, false, true, false, false),
            toggle_settings: Shortcut::new(egui::Key::Comma, false, true, false, false),
            toggle_search: Shortcut::new(egui::Key::F, false, true, false, false),
            toggle_toolbar: Shortcut::new(egui::Key::T, false, false, false, false),
            reveal_in_explorer: Shortcut::new(egui::Key::E, false, true, true, false),
            delete_current_file_permanently: Shortcut::new(egui::Key::Delete, false, false, true, false),
            overwrite_with_adjustments: Shortcut::new(egui::Key::S, false, true, true, false),
            reload_current_context: Shortcut::new(egui::Key::R, false, false, true, false),
            rotate_clockwise: Shortcut::new(egui::Key::R, false, true, false, false),
            close_window: Shortcut::new(egui::Key::Q, false, true, false, false),
            contrast_decrease: Shortcut::new(egui::Key::Num3, false, false, false, false),
            contrast_increase: Shortcut::new(egui::Key::Num4, false, false, false, false),
            gamma_decrease: Shortcut::new(egui::Key::Num5, false, false, false, false),
            gamma_increase: Shortcut::new(egui::Key::Num6, false, false, false, false),
            saturation_decrease: Shortcut::new(egui::Key::Num1, false, false, false, false),
            saturation_increase: Shortcut::new(egui::Key::Num2, false, false, false, false),
            exposure_decrease: Shortcut::new(egui::Key::Num7, false, false, false, false),
            exposure_increase: Shortcut::new(egui::Key::Num8, false, false, false, false),
            highlights_decrease: Shortcut::new(egui::Key::Num9, false, false, false, false),
            highlights_increase: Shortcut::new(egui::Key::Num0, false, false, false, false),
            shadows_decrease: Shortcut::new(egui::Key::O, false, false, false, false),
            shadows_increase: Shortcut::new(egui::Key::P, false, false, false, false),
            reset_adjustments: Shortcut::new(egui::Key::R, true, false, false, false),
            show_original_hold: Shortcut::new(egui::Key::O, true, false, false, false),
            clear_active_view: Shortcut::new(egui::Key::Escape, false, false, true, false),
        }
    }
}
