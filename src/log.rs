use colored::*;
use chrono::Local;
use serde_json::Value;

pub struct Log;

impl Log {
    pub fn info<T: AsRef<str>>(message: T) {
        Self::log_auto(message, &["cyan"], 2, 0);
    }

    pub fn success<T: AsRef<str>>(message: T) {
        Self::log_auto(message, &["green"], 2, 0);
    }

    pub fn error<T: AsRef<str>>(message: T) {
        Self::log_auto(message, &["red"], 2, 0);
    }

    pub fn warning<T: AsRef<str>>(message: T) {
        Self::log_auto(message, &["yellow"], 2, 0);
    }

    pub fn cluster<T: AsRef<str>>(message: T) {
        Self::log_auto(message, &["bold", "magenta"], 2, 0);
    }

    pub fn http<T: AsRef<str>>(message: T) {
        Self::info(message);
    }

    pub fn discover<T: AsRef<str>>(message: T) {
        Self::log_auto(message, &["bold", "bright_cyan"], 2, 0);
    }

    pub fn websocket<T: AsRef<str>>(message: T) {
        Self::success(message);
    }

    pub fn webhook_sender<T: AsRef<str>>(message: T) {
        Self::log_auto(message, &["bold", "white"], 2, 0);
    }

    pub fn info_title<T: AsRef<str>>(message: T) {
        Self::log_auto(message, &["bold", "on_cyan"], 2, 1);
    }

    pub fn success_title<T: AsRef<str>>(message: T) {
        Self::log_auto(message, &["bold", "on_green"], 2, 1);
    }

    pub fn error_title<T: AsRef<str>>(message: T) {
        Self::log_auto(Self::prefix_with_time(message), &["bold", "on_red"], 2, 1);
    }

    pub fn warning_title<T: AsRef<str>>(message: T) {
        Self::log_auto(Self::prefix_with_time(message), &["bold", "on_yellow"], 2, 1);
    }

    pub fn cluster_title<T: AsRef<str>>(message: T) {
        Self::log_auto(Self::prefix_with_time(message), &["bold", "yellow", "on_magenta"], 2, 1);
    }

    pub fn http_title<T: AsRef<str>>(message: T) {
        Self::info_title(Self::prefix_with_time(message));
    }

    pub fn discover_title<T: AsRef<str>>(message: T) {
        Self::log_auto(Self::prefix_with_time(message), &["bold", "bright_cyan", "on_white"], 2, 1);
    }

    pub fn websocket_title<T: AsRef<str>>(message: T) {
        Self::success_title(Self::prefix_with_time(message));
    }

    pub fn webhook_sender_title<T: AsRef<str>>(message: T) {
        Self::log_auto(Self::prefix_with_time(message), &["bold", "blue", "on_white"], 2, 1);
    }

    pub fn br() {
        println!();
    }

    fn prefix_with_time<T: AsRef<str>>(message: T) -> String {
        format!("[{}] {}", Local::now().to_rfc2822(), message.as_ref())
    }

    fn log_auto<T: AsRef<str>>(message: T, styles: &[&str], margin: usize, padding: usize) {
        let message = message.as_ref();
        if let Ok(json_value) = serde_json::from_str::<Value>(message) {
            // If it's valid JSON, prettify it
            let pretty_json = serde_json::to_string_pretty(&json_value).unwrap_or_else(|_| message.to_string());
            Self::log_formatted(&pretty_json, styles, margin, padding);
        } else {
            // If it's not JSON, log it as is
            Self::log_formatted(message, styles, margin, padding);
        }
    }

    fn log_formatted(message: &str, styles: &[&str], margin: usize, padding: usize) {
        let margin_spaces = " ".repeat(margin);
        let padding_spaces = " ".repeat(padding);
        for line in message.lines() {
            println!("{}{}{}{}", margin_spaces, padding_spaces, Self::apply_styles(line, styles), padding_spaces);
        }
    }

    fn apply_styles(message: &str, styles: &[&str]) -> ColoredString {
        let mut colored_message = message.normal();
        for style in styles {
            colored_message = match *style {
                "bold" => colored_message.bold(),
                "on_cyan" => colored_message.on_cyan(),
                "on_green" => colored_message.on_green(),
                "on_red" => colored_message.on_red(),
                "on_yellow" => colored_message.on_yellow(),
                "on_magenta" => colored_message.on_magenta(),
                "on_white" => colored_message.on_white(),
                "cyan" => colored_message.cyan(),
                "green" => colored_message.green(),
                "red" => colored_message.red(),
                "yellow" => colored_message.yellow(),
                "magenta" => colored_message.magenta(),
                "blue" => colored_message.blue(),
                "white" => colored_message.white(),
                "bright_cyan" => colored_message.bright_cyan(),
                _ => colored_message,
            };
        }
        colored_message
    }
}