pub const DEFAULT_NOTIFICATION_DURATION: f64 = 3.0;

pub struct NotificationToast {
    message: Option<String>,
    shown_at: Option<f64>,
}

impl NotificationToast {
    pub fn new() -> Self {
        Self {
            message: None,
            shown_at: None,
        }
    }

    pub fn show(&mut self, time: f64, message: String) {
        self.message = Some(message);
        self.shown_at = Some(time);
    }

    pub fn message(&mut self, time: f64) -> Option<&str> {
        let Some(start) = self.shown_at else {
            return None;
        };
        if time - start >= DEFAULT_NOTIFICATION_DURATION {
            self.message = None;
            self.shown_at = None;
            return None;
        }

        self.message.as_deref()
    }
}
