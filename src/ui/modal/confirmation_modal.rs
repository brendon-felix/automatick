use anyhow::Result;
use crossterm::event::KeyEvent;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use super::super::colors::*;
use super::super::tui::Frame as TuiFrame;
use super::Modal;

#[derive(Debug, Clone)]
pub enum ConfirmationType {
    Delete,
    Complete,
}

pub struct ConfirmationModal {
    title: String,
    message: String,
    confirmation_type: ConfirmationType,
}

impl ConfirmationModal {
    pub fn new(title: &str, message: &str) -> Self {
        Self {
            title: title.to_string(),
            message: message.to_string(),
            confirmation_type: ConfirmationType::Delete, // Default for backward compatibility
        }
    }

    pub fn new_with_type(title: &str, message: &str, confirmation_type: ConfirmationType) -> Self {
        Self {
            title: title.to_string(),
            message: message.to_string(),
            confirmation_type,
        }
    }

    pub fn confirmation_type(&self) -> &ConfirmationType {
        &self.confirmation_type
    }
}

impl Modal for ConfirmationModal {
    fn title(&self) -> &str {
        &self.title
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn handle_key_event(&mut self, _key_event: KeyEvent) -> Result<bool> {
        // This modal doesn't handle any special key events internally
        // All key handling is done at the app level
        Ok(false)
    }

    // Uses default implementations for validate() and has_validation_errors()

    fn render(&mut self, frame: &mut TuiFrame, area: Rect) {
        // Message content
        let message_lines: Vec<Line> = self
            .message
            .split('\n')
            .map(|line| Line::from(Span::styled(line, Style::default().fg(TEXT_WHITE))))
            .collect();

        let instructions = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "y",
                    Style::default()
                        .fg(ACCENT_GREEN)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" to confirm, "),
                Span::styled(
                    "n/Esc",
                    Style::default().fg(ACCENT_RED).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" to cancel"),
            ]),
        ];

        let mut all_lines = message_lines;
        all_lines.extend(instructions);

        // Calculate content dimensions
        let max_line_width = all_lines
            .iter()
            .map(|line| line.width())
            .max()
            .unwrap_or(20);
        let content_height = all_lines.len();

        // Add padding for borders and title, but keep it minimal
        let modal_width = (max_line_width + 4).min(area.width as usize) as u16; // +4 for left/right borders and padding
        let modal_height = (content_height + 3) as u16; // +3 for top/bottom borders and title

        // Center the tight modal
        let popup_x = (area.width.saturating_sub(modal_width)) / 2;
        let popup_y = (area.height.saturating_sub(modal_height)) / 2;

        let popup_area = Rect {
            x: area.x + popup_x,
            y: area.y + popup_y,
            width: modal_width,
            height: modal_height,
        };

        frame.render_widget(Clear, popup_area);

        let message_paragraph = Paragraph::new(all_lines)
            .style(Style::default().fg(TEXT_WHITE).bg(NORMAL_BG))
            .alignment(Alignment::Center);

        // Choose border color based on confirmation type
        let border_color = match self.confirmation_type {
            ConfirmationType::Delete => BORDER_DANGER,
            ConfirmationType::Complete => ACCENT_GREEN,
        };

        // Render the modal border
        let modal_block = Block::default()
            .title(self.title.as_str())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .style(Style::default().bg(NORMAL_BG));

        let inner_area = modal_block.inner(popup_area);
        frame.render_widget(modal_block, popup_area);
        frame.render_widget(message_paragraph, inner_area);
    }

    fn get_values(&self) -> Vec<String> {
        vec![]
    }

    fn clear_inputs(&mut self) {
        // No inputs to clear
    }

    fn set_values(&mut self, _values: Vec<String>) {
        // No values to set
    }
}
