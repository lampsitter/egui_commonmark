use crate::elements::{blockquote, newline};
use egui::Ui;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Alert {
    /// The color that will be used to put emphasis to the alert
    pub accent_color: egui::Color32,
    /// The icon that will be displayed
    pub icon: char,
    /// The identifier that will be used to look for the blockquote such as NOTE and TIP
    pub identifier: String,
    /// The identifier that will be shown when rendering. E.g: Note and Tip
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
                accent_color: egui::Color32::from_rgb(10, 80, 210),
                icon: '❕',
                identifier: "NOTE".to_owned(),
                identifier_rendered: "Note".to_owned(),
            },
            Alert {
                accent_color: egui::Color32::from_rgb(0, 130, 20),
                icon: '💡',
                identifier: "TIP".to_owned(),
                identifier_rendered: "Tip".to_owned(),
            },
            Alert {
                accent_color: egui::Color32::from_rgb(150, 30, 140),
                icon: '💬',
                identifier: "IMPORTANT".to_owned(),
                identifier_rendered: "Important".to_owned(),
            },
            Alert {
                accent_color: egui::Color32::from_rgb(200, 120, 0),
                icon: '⚠',
                identifier: "WARNING".to_owned(),
                identifier_rendered: "Warning".to_owned(),
            },
            Alert {
                accent_color: egui::Color32::from_rgb(220, 0, 0),
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
