//! `Notification` tool: sends a Windows toast notification via WinRT.

use rmcp::schemars;
use serde::Deserialize;
use windows::Data::Xml::Dom::XmlDocument;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
use windows::core::HSTRING;

/// Parameters for the `Notification` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct NotificationParams {
    /// The title/heading of the toast notification.
    #[schemars(description = "The title/heading of the toast notification.")]
    pub title: String,
    /// The body text of the toast notification displayed below the title.
    #[schemars(description = "The body text of the toast notification displayed below the title.")]
    pub message: String,
    /// The Application User Model ID used as the toast's app identity.
    #[schemars(
        description = "The valid Application User Model ID of the toast notification. Required to display the notification in a specific app."
    )]
    pub app_id: String,
}

/// Sends a toast notification. Always returns a caller-facing text response,
/// even on failure.
pub fn send_notification(title: &str, message: &str, app_id: &str) -> String {
    match try_send(title, message, app_id) {
        Ok(()) => format!("Notification sent: \"{title}\" - {message}"),
        Err(e) => format!("Notification may have failed to send: {e}"),
    }
}

fn try_send(title: &str, message: &str, app_id: &str) -> windows::core::Result<()> {
    // Safe to call on every send: WinRT treats re-initialization on the same
    // thread as a no-op success (S_FALSE).
    unsafe { RoInitialize(RO_INIT_MULTITHREADED)? };

    let template = format!(
        "<toast><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>",
        xml_escape(title),
        xml_escape(message),
    );

    let xml = XmlDocument::new()?;
    xml.LoadXml(&HSTRING::from(template))?;

    let toast = ToastNotification::CreateToastNotification(&xml)?;
    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(app_id))?;
    notifier.Show(&toast)
}

/// Escapes text for embedding inside XML element content.
fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_special_characters() {
        assert_eq!(xml_escape("<a> & \"b\" 'c'"), "&lt;a&gt; &amp; &quot;b&quot; &apos;c&apos;");
    }
}
