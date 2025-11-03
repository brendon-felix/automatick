use anyhow::Result;
use crossterm::event::KeyEvent;
use edtui::{EditorEventHandler, EditorMode, EditorState, EditorTheme, EditorView, Index2, Lines};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use super::super::centered_rect;
use super::super::colors::*;
use super::super::tui::Frame as TuiFrame;
use super::Modal;

pub struct PostponeModal {
    title: String,
    input_duration_editor: EditorState,
    event_handler: EditorEventHandler,
    is_edit_mode: bool,
    // Validation state
    validation_attempted: bool,
    duration_error: Option<String>,
}

impl PostponeModal {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            input_duration_editor: EditorState::default(),
            event_handler: EditorEventHandler::default(),
            is_edit_mode: false,
            validation_attempted: false,
            duration_error: None,
        }
    }

    pub fn new_with_default(title: &str, default_duration: Option<String>) -> Self {
        let mut modal = Self::new(title);

        if let Some(duration) = default_duration {
            modal.input_duration_editor = EditorState::new(Lines::from(duration));
        }

        // Set initial mode and cursor position
        modal.position_cursor_at_end();
        modal.set_editor_to_insert_mode();

        modal
    }

    #[allow(dead_code)]
    pub fn new_for_edit(title: &str, default_duration: Option<String>) -> Self {
        let mut modal = Self::new_with_default(title, default_duration);
        modal.is_edit_mode = true;
        // For edit mode, start in Normal mode like TaskModal
        modal.set_editor_to_normal_mode();
        modal
    }

    pub fn set_editor_to_insert_mode(&mut self) {
        self.input_duration_editor.mode = EditorMode::Insert;
    }

    pub fn set_editor_to_normal_mode(&mut self) {
        self.input_duration_editor.mode = EditorMode::Normal;
    }

    pub fn position_cursor_at_start(&mut self) {
        self.input_duration_editor.cursor = Index2::new(0, 0);
    }

    pub fn position_cursor_at_end(&mut self) {
        // Position cursor at the end of the text
        if self.input_duration_editor.lines.is_empty() {
            self.input_duration_editor.cursor = Index2::new(0, 0);
        } else if let Some(last_row_idx) = self.input_duration_editor.lines.len().checked_sub(1) {
            if let Some(last_col) = self.input_duration_editor.lines.len_col(last_row_idx) {
                self.input_duration_editor.cursor = Index2::new(last_row_idx, last_col);
            }
        }
    }

    pub fn is_editor_in_insert_mode(&self) -> bool {
        self.input_duration_editor.mode == EditorMode::Insert
    }

    pub fn get_input_duration(&self) -> String {
        String::from(self.input_duration_editor.lines.clone())
    }

    pub fn handle_input_key_event(&mut self, key_event: KeyEvent) -> Result<()> {
        self.event_handler
            .on_key_event(key_event, &mut self.input_duration_editor);

        // Clear validation errors when user edits the field
        if self.validation_attempted {
            self.duration_error = None;
        }

        Ok(())
    }
}

impl Modal for PostponeModal {
    fn title(&self) -> &str {
        &self.title
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<bool> {
        use crossterm::event::KeyCode;

        match key_event.code {
            KeyCode::Esc => {
                // If editor is in insert mode, switch to normal mode
                if self.is_editor_in_insert_mode() {
                    self.set_editor_to_normal_mode();
                    Ok(true)
                } else {
                    // Already in normal mode, let app handle it (close modal)
                    Ok(false)
                }
            }
            KeyCode::Enter => {
                // For single-line duration field, always confirm input (both normal and insert mode)
                Ok(false)
            }
            KeyCode::Char('h') | KeyCode::Left => {
                // Handle horizontal movement
                self.handle_input_key_event(key_event)?;
                Ok(true)
            }
            KeyCode::Char('l') | KeyCode::Right => {
                // Handle horizontal movement
                self.handle_input_key_event(key_event)?;
                Ok(true)
            }
            KeyCode::Char('0') => {
                // Handle home key (beginning of line) - only in normal mode
                if !self.is_editor_in_insert_mode() {
                    self.position_cursor_at_start();
                    Ok(true)
                } else {
                    // In insert mode, let editor handle normally
                    self.handle_input_key_event(key_event)?;
                    Ok(true)
                }
            }
            KeyCode::Char('$') => {
                // Handle end key (end of line) - only in normal mode
                if !self.is_editor_in_insert_mode() {
                    self.position_cursor_at_end();
                    Ok(true)
                } else {
                    // In insert mode, let editor handle normally
                    self.handle_input_key_event(key_event)?;
                    Ok(true)
                }
            }
            KeyCode::Char('i') => {
                // Enter insert mode - only in normal mode
                if !self.is_editor_in_insert_mode() {
                    self.set_editor_to_insert_mode();
                    Ok(true)
                } else {
                    // Already in insert mode, let editor handle normally
                    self.handle_input_key_event(key_event)?;
                    Ok(true)
                }
            }
            KeyCode::Char('a') => {
                // Enter insert mode after cursor - only in normal mode
                if !self.is_editor_in_insert_mode() {
                    // Move cursor one position right, then enter insert mode
                    self.handle_input_key_event(KeyEvent::new(
                        KeyCode::Right,
                        crossterm::event::KeyModifiers::NONE,
                    ))?;
                    self.set_editor_to_insert_mode();
                    Ok(true)
                } else {
                    // In insert mode, let editor handle normally
                    self.handle_input_key_event(key_event)?;
                    Ok(true)
                }
            }
            KeyCode::Char('A') => {
                // Enter insert mode at end of line - only in normal mode
                if !self.is_editor_in_insert_mode() {
                    self.position_cursor_at_end();
                    self.set_editor_to_insert_mode();
                    Ok(true)
                } else {
                    // In insert mode, let editor handle normally
                    self.handle_input_key_event(key_event)?;
                    Ok(true)
                }
            }
            KeyCode::Char('I') => {
                // Enter insert mode at beginning of line - only in normal mode
                if !self.is_editor_in_insert_mode() {
                    self.position_cursor_at_start();
                    self.set_editor_to_insert_mode();
                    Ok(true)
                } else {
                    // In insert mode, let editor handle normally
                    self.handle_input_key_event(key_event)?;
                    Ok(true)
                }
            }
            KeyCode::Home | KeyCode::End => {
                // Handle home/end keys
                self.handle_input_key_event(key_event)?;
                Ok(true)
            }
            KeyCode::Char('x') => {
                // Delete character under cursor - only in normal mode
                if !self.is_editor_in_insert_mode() {
                    self.handle_input_key_event(KeyEvent::new(
                        KeyCode::Delete,
                        crossterm::event::KeyModifiers::NONE,
                    ))?;
                    Ok(true)
                } else {
                    // In insert mode, let editor handle normally
                    self.handle_input_key_event(key_event)?;
                    Ok(true)
                }
            }
            KeyCode::Char('X') => {
                // Delete character before cursor - only in normal mode
                if !self.is_editor_in_insert_mode() {
                    self.handle_input_key_event(KeyEvent::new(
                        KeyCode::Backspace,
                        crossterm::event::KeyModifiers::NONE,
                    ))?;
                    Ok(true)
                } else {
                    // In insert mode, let editor handle normally
                    self.handle_input_key_event(key_event)?;
                    Ok(true)
                }
            }
            KeyCode::Char('d') => {
                // Delete line (dd) - only in normal mode
                if !self.is_editor_in_insert_mode() {
                    // Clear the entire content
                    self.input_duration_editor = EditorState::default();
                    Ok(true)
                } else {
                    // In insert mode, let editor handle normally
                    self.handle_input_key_event(key_event)?;
                    Ok(true)
                }
            }
            KeyCode::Char('c') => {
                // Change line (cc) - clear content and enter insert mode - only in normal mode
                if !self.is_editor_in_insert_mode() {
                    // Clear the entire content and enter insert mode
                    self.input_duration_editor = EditorState::default();
                    self.set_editor_to_insert_mode();
                    Ok(true)
                } else {
                    // In insert mode, let editor handle normally
                    self.handle_input_key_event(key_event)?;
                    Ok(true)
                }
            }
            KeyCode::Backspace | KeyCode::Delete => {
                // Handle deletion
                self.handle_input_key_event(key_event)?;
                Ok(true)
            }
            _ => {
                // For all other keys (including text input), handle normally
                self.handle_input_key_event(key_event)?;
                Ok(true)
            }
        }
    }

    fn render(&mut self, frame: &mut TuiFrame, area: Rect) {
        let popup_area = centered_rect(45, 15, area);
        frame.render_widget(Clear, popup_area);

        // Render background
        let bg_block = Block::default().style(Style::default().bg(NORMAL_BG));
        frame.render_widget(bg_block, popup_area);

        let chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(4), // Duration field + error message
                Constraint::Length(1), // Help text
            ])
            .split(popup_area);

        // Duration field with error message layout
        let duration_field_layout = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Duration input
                Constraint::Length(1), // Error message
            ])
            .split(chunks[0]);

        let duration_has_error = self.validation_attempted && self.duration_error.is_some();
        let duration_border_color = if duration_has_error {
            ACCENT_RED
        } else if self.is_editor_in_insert_mode() {
            if self.is_edit_mode {
                BORDER_EDIT
            } else {
                BORDER_NEW
            }
        } else {
            BORDER_PROCESSING
        };

        let duration_block = Block::default()
            .title("Duration (e.g., \"5min\", \"2 hours\", \"1day\", \"now\", \"now + 30min\")")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(duration_border_color))
            .style(Style::default().bg(NORMAL_BG));

        let duration_inner = duration_block.inner(duration_field_layout[0]);
        frame.render_widget(duration_block, duration_field_layout[0]);

        let duration_theme = EditorTheme::default()
            .base(Style::default().bg(NORMAL_BG).fg(TEXT_FG))
            .cursor_style(Style::default().bg(TEXT_FG).fg(NORMAL_BG))
            .selection_style(Style::default().bg(SELECTED_BG).fg(TEXT_FG))
            .hide_status_line();
        let duration_editor_view =
            EditorView::new(&mut self.input_duration_editor).theme(duration_theme);
        frame.render_widget(duration_editor_view, duration_inner);

        // Render duration error message if present
        if let Some(error) = &self.duration_error {
            let error_paragraph =
                Paragraph::new(error.as_str()).style(Style::default().fg(ACCENT_RED).bg(NORMAL_BG));
            frame.render_widget(error_paragraph, duration_field_layout[1]);
        }

        // Help text at bottom of modal (matching TaskModal style)
        let help_text = vec![Line::from(vec![
            Span::styled("Enter", Style::default().fg(ACCENT_GREEN)),
            Span::raw(" confirm  •  "),
            Span::styled("Esc", Style::default().fg(ACCENT_RED)),
            Span::raw(" cancel"),
        ])];
        let help_paragraph = Paragraph::new(help_text)
            .style(Style::default().bg(NORMAL_BG))
            .alignment(Alignment::Center);

        frame.render_widget(help_paragraph, chunks[1]);

        // Render the modal border
        let modal_border_color = if self.is_edit_mode {
            BORDER_EDIT
        } else {
            BORDER_NEW
        };
        let modal_block = Block::default()
            .title(self.title.as_str())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(modal_border_color));

        frame.render_widget(modal_block, popup_area);
    }

    fn get_values(&self) -> Vec<String> {
        vec![self.get_input_duration()]
    }

    fn clear_inputs(&mut self) {
        self.input_duration_editor = EditorState::default();
        self.validation_attempted = false;
        self.duration_error = None;
    }

    fn set_values(&mut self, values: Vec<String>) {
        if values.len() >= 1 && !values[0].is_empty() {
            self.input_duration_editor = EditorState::new(Lines::from(values[0].clone()));
        }
    }

    fn validate(&mut self) -> bool {
        use crate::utils::parse_duration;

        self.validation_attempted = true;
        let duration_str = self.get_input_duration();

        if duration_str.trim().is_empty() {
            self.duration_error = Some("Duration cannot be empty".to_string());
            return false;
        }

        match parse_duration(&duration_str) {
            Ok(_) => {
                self.duration_error = None;
                true
            }
            Err(err) => {
                self.duration_error = Some(err);
                false
            }
        }
    }

    fn has_validation_errors(&self) -> bool {
        self.duration_error.is_some()
    }
}
