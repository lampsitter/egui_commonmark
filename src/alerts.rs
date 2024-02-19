use crate::elements::{blockquote, newline};
use egui::Ui;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Alert {
    /// The color that will be used to put emphasis to the alert
    pub accent_color: egui::Color32,
    /// The icon that will be rendered
    pub icon: char,
    /// The identifier that will be look for in the blockquote
    pub identifier: String,
    /// The identifier that will be shown when rendering
    pub identifier_rendered: String,
}

impl Alert {
    pub(crate) fn ui(&self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
        blockquote(ui, self.accent_color, |ui| {
            newline(ui);
            ui.colored_label(self.accent_color, self.icon.to_string());
            ui.add_space(3.0);
            ui.colored_label(self.accent_color, &self.identifier_rendered);
            // end line
            newline(ui);
            add_contents(ui);
        })
    }
}

#[derive(Debug, Clone)]
pub struct AlertBundle {
    /// the key is `[!identifier]`
    alerts: HashMap<String, Alert>,
}

impl AlertBundle {
    fn from_alerts(alerts: Vec<Alert>) -> Self {
        let mut map = HashMap::with_capacity(alerts.len());
        for alert in alerts {
            // Store it the way it will be in text to make lookup easier
            map.insert(format!("[!{}]", alert.identifier), alert);
        }

        Self { alerts: map }
    }

    pub(crate) fn try_get_alert(&self, text: &str) -> Option<&Alert> {
        self.alerts.get(text)
    }

    pub fn empty() -> Self {
        AlertBundle {
            alerts: Default::default(),
        }
    }

    /// github flavoured markdown alerts
    /// `[!NOTE]`, `[!TIP]`, `[!IMPORTANT]`, `[!WARNING]` and `[!CAUTION]`.
    ///
    /// This is used by default
    pub fn gfm() -> Self {
        Self::from_alerts(vec![
            Alert {
                accent_color: egui::Color32::BLUE,
                icon: '❕',
                identifier: "NOTE".to_owned(),
                identifier_rendered: "Note".to_owned(),
            },
            Alert {
                accent_color: egui::Color32::LIGHT_GREEN,
                icon: '💡',
                identifier: "TIP".to_owned(),
                identifier_rendered: "Tip".to_owned(),
            },
            Alert {
                accent_color: egui::Color32::DARK_GREEN, // FIXME: Purple
                icon: '💬',
                identifier: "IMPORTANT".to_owned(),
                identifier_rendered: "Important".to_owned(),
            },
            Alert {
                accent_color: egui::Color32::YELLOW,
                icon: '⚠',
                identifier: "WARNING".to_owned(),
                identifier_rendered: "Warning".to_owned(),
            },
            Alert {
                accent_color: egui::Color32::RED,
                icon: '🔴',
                identifier: "CAUTION".to_owned(),
                identifier_rendered: "Caution".to_owned(),
            },
        ])
    }

    /// See if the bundle contains no alerts
    pub fn is_empty(&self) -> bool {
        self.alerts.is_empty()
    }
}
