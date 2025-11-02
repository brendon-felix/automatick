use anyhow::Result;
use crossterm::event::KeyEvent;
use edtui::{EditorEventHandler, EditorMode, EditorState, EditorTheme, EditorView, Index2, Lines};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::colors::*;
use crate::tui::Frame as TuiFrame;
use crate::ui::{centered_rect, InputField};

/// Trait for modal dialogs that can be displayed as overlays
#[allow(dead_code)]
pub trait Modal {
    /// Get the title of the modal
    fn title(&self) -> &str;

    /// Handle key events for the modal
    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<bool>;

    /// Render the modal content
    fn render(&mut self, frame: &mut TuiFrame, area: Rect);

    /// Get the modal's input values (if any)
    fn get_values(&self) -> Vec<String>;

    /// Clear all inputs
    fn clear_inputs(&mut self);

    /// Set initial values
    fn set_values(&mut self, values: Vec<String>);

    /// Validate inputs and return true if valid (default implementation always returns true)
    fn validate(&mut self) -> bool {
        true
    }

    /// Check if there are any validation errors (default implementation always returns false)
    fn has_validation_errors(&self) -> bool {
        false
    }
}

/// Modal for creating and editing tasks
pub struct TaskModal {
    title: String,
    input_title_editor: EditorState,
    input_description_editor: EditorState,
    input_date_editor: EditorState,
    input_time_editor: EditorState,
    current_input_field: InputField,
    event_handler: EditorEventHandler,
    is_edit_mode: bool,
    desired_column: usize,
    // Validation state
    validation_attempted: bool,
    date_error: Option<String>,
    time_error: Option<String>,
}

impl TaskModal {
    pub fn new(title: String) -> Self {
        Self {
            title,
            input_title_editor: EditorState::default(),
            input_description_editor: EditorState::default(),
            input_date_editor: EditorState::default(),
            input_time_editor: EditorState::default(),
            current_input_field: InputField::Title,
            event_handler: EditorEventHandler::default(),
            is_edit_mode: false,
            desired_column: 0,
            validation_attempted: false,
            date_error: None,
            time_error: None,
        }
    }

    pub fn new_with_defaults(
        title: &str,
        default_title: Option<String>,
        default_description: Option<String>,
        default_date: Option<String>,
        default_time: Option<String>,
        is_edit_mode: bool,
    ) -> Self {
        let mut modal = Self::new(title.to_string());
        modal.is_edit_mode = is_edit_mode;

        if let Some(task_title) = default_title {
            modal.input_title_editor = EditorState::new(Lines::from(task_title));
        }

        if let Some(description) = default_description {
            modal.input_description_editor = EditorState::new(Lines::from(description));
        }

        if let Some(date) = default_date {
            modal.input_date_editor = EditorState::new(Lines::from(date));
        }

        if let Some(time) = default_time {
            modal.input_time_editor = EditorState::new(Lines::from(time));
        }

        // Set editor modes and cursor position
        modal.position_cursor_at_end();

        // Set modes AFTER positioning cursor to ensure they stick
        if is_edit_mode {
            // For Edit Task: set all editors to Normal mode
            modal.set_all_editors_to_normal_mode();
        } else {
            // For New Task: set current editor to Insert mode
            modal.set_current_editor_to_insert_mode();
        }

        modal
    }

    fn get_current_editor_mut(&mut self) -> &mut EditorState {
        match self.current_input_field {
            InputField::Title => &mut self.input_title_editor,
            InputField::Description => &mut self.input_description_editor,
            InputField::Date => &mut self.input_date_editor,
            InputField::Time => &mut self.input_time_editor,
        }
    }

    fn get_current_editor(&self) -> &EditorState {
        match self.current_input_field {
            InputField::Title => &self.input_title_editor,
            InputField::Description => &self.input_description_editor,
            InputField::Date => &self.input_date_editor,
            InputField::Time => &self.input_time_editor,
        }
    }

    pub fn next_input_field(&mut self) {
        self.current_input_field = match self.current_input_field {
            InputField::Title => InputField::Description,
            InputField::Description => InputField::Date,
            InputField::Date => InputField::Time,
            InputField::Time => InputField::Title,
        };
        // Ensure mode is preserved when switching fields
        if self.is_edit_mode {
            self.set_current_editor_to_normal_mode();
        }
    }

    pub fn previous_input_field(&mut self) {
        self.current_input_field = match self.current_input_field {
            InputField::Title => InputField::Time,
            InputField::Description => InputField::Title,
            InputField::Date => InputField::Description,
            InputField::Time => InputField::Date,
        };
        // Ensure mode is preserved when switching fields
        if self.is_edit_mode {
            self.set_current_editor_to_normal_mode();
        }
    }

    pub fn set_current_editor_to_insert_mode(&mut self) {
        let editor = self.get_current_editor_mut();
        editor.mode = EditorMode::Insert;
    }

    pub fn set_current_editor_to_normal_mode(&mut self) {
        let editor = self.get_current_editor_mut();
        editor.mode = EditorMode::Normal;
    }

    pub fn set_all_editors_to_normal_mode(&mut self) {
        self.input_title_editor.mode = EditorMode::Normal;
        self.input_description_editor.mode = EditorMode::Normal;
        self.input_date_editor.mode = EditorMode::Normal;
        self.input_time_editor.mode = EditorMode::Normal;
    }

    pub fn position_cursor_at_end(&mut self) {
        let editor = self.get_current_editor_mut();

        // Position cursor at the end of the text
        if editor.lines.is_empty() {
            editor.cursor = Index2::new(0, 0);
            self.desired_column = 0;
        } else if let Some(last_row_idx) = editor.lines.len().checked_sub(1) {
            if let Some(last_col) = editor.lines.len_col(last_row_idx) {
                editor.cursor = Index2::new(last_row_idx, last_col);
                self.desired_column = last_col;
            }
        }
    }

    pub fn position_cursor_at_start(&mut self) {
        let editor = self.get_current_editor_mut();
        editor.cursor = Index2::new(0, 0);
        self.desired_column = 0;
    }

    pub fn position_cursor_at_desired_column(&mut self, row: usize) {
        let desired_col = self.desired_column;
        let editor = self.get_current_editor_mut();

        // Get the length of the target row
        let target_col = if let Some(row_len) = editor.lines.len_col(row) {
            // Position at desired column, but don't go beyond the end of the line
            desired_col.min(row_len)
        } else {
            0
        };

        editor.cursor = Index2::new(row, target_col);
    }

    pub fn update_desired_column(&mut self) {
        let editor = self.get_current_editor();
        self.desired_column = editor.cursor.col;
    }

    fn is_at_first_line_in_multiline_field(&self) -> bool {
        if self.current_input_field != InputField::Description {
            return false;
        }
        let editor = self.get_current_editor();
        editor.cursor.row == 0
    }

    fn is_at_last_line_in_multiline_field(&self) -> bool {
        if self.current_input_field != InputField::Description {
            return false;
        }
        let editor = self.get_current_editor();
        if editor.lines.is_empty() {
            return true;
        }
        let last_row_idx = editor.lines.len().saturating_sub(1);
        editor.cursor.row >= last_row_idx
    }

    fn navigate_to_first_field_or_line(&mut self) {
        if self.current_input_field == InputField::Description {
            // If in description field, go to first line
            let editor = self.get_current_editor_mut();
            editor.cursor = Index2::new(0, 0);
            self.desired_column = 0;
        } else {
            // Go to first field (Title)
            self.current_input_field = InputField::Title;
            self.position_cursor_at_start();
            if self.is_edit_mode {
                self.set_current_editor_to_normal_mode();
            }
        }
    }

    fn navigate_to_last_field_or_line(&mut self) {
        if self.current_input_field == InputField::Description {
            // If in description field, go to last line
            let editor = self.get_current_editor_mut();
            if editor.lines.is_empty() {
                editor.cursor = Index2::new(0, 0);
                self.desired_column = 0;
            } else if let Some(last_row_idx) = editor.lines.len().checked_sub(1) {
                if let Some(last_col) = editor.lines.len_col(last_row_idx) {
                    editor.cursor = Index2::new(last_row_idx, last_col);
                    self.desired_column = last_col;
                }
            }
        } else {
            // Go to last field (Time)
            self.current_input_field = InputField::Time;
            self.position_cursor_at_end();
            if self.is_edit_mode {
                self.set_current_editor_to_normal_mode();
            }
        }
    }

    fn handle_j_navigation(&mut self) -> bool {
        if self.current_input_field == InputField::Description {
            // In description field, check if we're at the last line
            if self.is_at_last_line_in_multiline_field() {
                // Move to next field
                self.next_input_field();
                // Position cursor at desired column on first row of next field
                self.position_cursor_at_desired_column(0);
                if self.is_edit_mode {
                    self.set_current_editor_to_normal_mode();
                }
                return true;
            } else {
                // Let editor handle moving down within the field
                return false;
            }
        } else {
            // In single-line field, move to next field
            self.next_input_field();
            // Position cursor at desired column on first row of next field
            if self.current_input_field == InputField::Description {
                self.position_cursor_at_desired_column(0);
            } else {
                self.position_cursor_at_desired_column(0);
            }
            if self.is_edit_mode {
                self.set_current_editor_to_normal_mode();
            }
            return true;
        }
    }

    fn handle_k_navigation(&mut self) -> bool {
        if self.current_input_field == InputField::Description {
            // In description field, check if we're at the first line
            if self.is_at_first_line_in_multiline_field() {
                // Move to previous field
                self.previous_input_field();
                // Position cursor at desired column on last row of previous field
                let last_row = {
                    let editor = self.get_current_editor();
                    if editor.lines.is_empty() {
                        0
                    } else {
                        editor.lines.len().saturating_sub(1)
                    }
                };
                self.position_cursor_at_desired_column(last_row);
                if self.is_edit_mode {
                    self.set_current_editor_to_normal_mode();
                }
                return true;
            } else {
                // Let editor handle moving up within the field
                return false;
            }
        } else {
            // In single-line field, move to previous field
            self.previous_input_field();
            // Position cursor at desired column on last row of previous field
            let last_row = {
                let editor = self.get_current_editor();
                if editor.lines.is_empty() {
                    0
                } else {
                    editor.lines.len().saturating_sub(1)
                }
            };
            self.position_cursor_at_desired_column(last_row);
            if self.is_edit_mode {
                self.set_current_editor_to_normal_mode();
            }
            return true;
        }
    }

    pub fn is_current_editor_in_insert_mode(&self) -> bool {
        let editor = self.get_current_editor();
        editor.mode == EditorMode::Insert
    }

    pub fn get_input_value(&self) -> String {
        String::from(self.input_title_editor.lines.clone())
    }

    pub fn get_input_description(&self) -> String {
        String::from(self.input_description_editor.lines.clone())
    }

    pub fn get_input_date(&self) -> String {
        String::from(self.input_date_editor.lines.clone())
    }

    pub fn get_input_time(&self) -> String {
        String::from(self.input_time_editor.lines.clone())
    }

    pub fn handle_input_key_event(&mut self, key_event: KeyEvent) -> Result<()> {
        match self.current_input_field {
            InputField::Title => {
                self.event_handler
                    .on_key_event(key_event, &mut self.input_title_editor);
            }
            InputField::Description => {
                self.event_handler
                    .on_key_event(key_event, &mut self.input_description_editor);
            }
            InputField::Date => {
                self.event_handler
                    .on_key_event(key_event, &mut self.input_date_editor);
            }
            InputField::Time => {
                self.event_handler
                    .on_key_event(key_event, &mut self.input_time_editor);
            }
        }
        Ok(())
    }

    pub fn handle_input_key_event_and_update_column(&mut self, key_event: KeyEvent) -> Result<()> {
        self.handle_input_key_event(key_event)?;
        // Update desired column after handling input that may change cursor position
        self.update_desired_column();

        // Clear validation errors when user edits a field
        if self.validation_attempted {
            match self.current_input_field {
                InputField::Date => self.date_error = None,
                InputField::Time => self.time_error = None,
                _ => {}
            }
        }

        Ok(())
    }
}

impl Modal for TaskModal {
    fn title(&self) -> &str {
        &self.title
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<bool> {
        use crossterm::event::KeyCode;

        match key_event.code {
            KeyCode::Esc => {
                // If current editor is in insert mode, switch to normal mode
                if self.is_current_editor_in_insert_mode() {
                    let editor = self.get_current_editor_mut();
                    editor.mode = EditorMode::Normal;
                    Ok(true)
                } else {
                    // Already in normal mode, let app handle it (close modal)
                    Ok(false)
                }
            }
            KeyCode::Enter => {
                match self.current_input_field {
                    InputField::Description => {
                        if self.is_current_editor_in_insert_mode() {
                            // Allow newline insertion for description field in insert mode
                            self.handle_input_key_event(key_event)?;
                            Ok(true)
                        } else {
                            // In normal mode, confirm input
                            Ok(false)
                        }
                    }
                    InputField::Title | InputField::Date | InputField::Time => {
                        // For single-line fields, always confirm input (both normal and insert mode)
                        Ok(false)
                    }
                }
            }
            KeyCode::Tab => {
                self.next_input_field();
                self.position_cursor_at_end();
                if !self.is_edit_mode {
                    self.set_current_editor_to_insert_mode();
                } else {
                    // For edit mode, ensure we stay in Normal mode
                    self.set_current_editor_to_normal_mode();
                }
                Ok(true)
            }
            KeyCode::BackTab => {
                self.previous_input_field();
                self.position_cursor_at_end();
                if !self.is_edit_mode {
                    self.set_current_editor_to_insert_mode();
                } else {
                    // For edit mode, ensure we stay in Normal mode
                    self.set_current_editor_to_normal_mode();
                }
                Ok(true)
            }
            KeyCode::Char('j') => {
                // Only handle j navigation in Normal mode
                if !self.is_current_editor_in_insert_mode() {
                    if self.handle_j_navigation() {
                        Ok(true)
                    } else {
                        // Let editor handle the key (move down within multiline field)
                        self.handle_input_key_event(key_event)?;
                        Ok(true)
                    }
                } else {
                    // In insert mode, let editor handle normally
                    self.handle_input_key_event(key_event)?;
                    Ok(true)
                }
            }
            KeyCode::Char('k') => {
                // Only handle k navigation in Normal mode
                if !self.is_current_editor_in_insert_mode() {
                    if self.handle_k_navigation() {
                        Ok(true)
                    } else {
                        // Let editor handle the key (move up within multiline field)
                        self.handle_input_key_event(key_event)?;
                        Ok(true)
                    }
                } else {
                    // In insert mode, let editor handle normally
                    self.handle_input_key_event(key_event)?;
                    Ok(true)
                }
            }
            KeyCode::Char('g') => {
                // Only handle g navigation in Normal mode
                if !self.is_current_editor_in_insert_mode() {
                    self.navigate_to_first_field_or_line();
                    Ok(true)
                } else {
                    // In insert mode, let editor handle normally
                    self.handle_input_key_event(key_event)?;
                    Ok(true)
                }
            }
            KeyCode::Char('G') => {
                // Only handle G navigation in Normal mode
                if !self.is_current_editor_in_insert_mode() {
                    self.navigate_to_last_field_or_line();
                    Ok(true)
                } else {
                    // In insert mode, let editor handle normally
                    self.handle_input_key_event(key_event)?;
                    Ok(true)
                }
            }
            KeyCode::Down => {
                // Only handle Down navigation in Normal mode (same as j)
                if !self.is_current_editor_in_insert_mode() {
                    if self.handle_j_navigation() {
                        Ok(true)
                    } else {
                        // Let editor handle the key (move down within multiline field)
                        self.handle_input_key_event(key_event)?;
                        Ok(true)
                    }
                } else {
                    // In insert mode, let editor handle normally
                    self.handle_input_key_event(key_event)?;
                    Ok(true)
                }
            }
            KeyCode::Up => {
                // Only handle Up navigation in Normal mode (same as k)
                if !self.is_current_editor_in_insert_mode() {
                    if self.handle_k_navigation() {
                        Ok(true)
                    } else {
                        // Let editor handle the key (move up within multiline field)
                        self.handle_input_key_event(key_event)?;
                        Ok(true)
                    }
                } else {
                    // In insert mode, let editor handle normally
                    self.handle_input_key_event(key_event)?;
                    Ok(true)
                }
            }
            KeyCode::Char('h') | KeyCode::Left => {
                // Handle horizontal movement - update desired column
                self.handle_input_key_event_and_update_column(key_event)?;
                Ok(true)
            }
            KeyCode::Char('l') | KeyCode::Right => {
                // Handle horizontal movement - update desired column
                self.handle_input_key_event_and_update_column(key_event)?;
                Ok(true)
            }
            KeyCode::Backspace | KeyCode::Delete => {
                // Handle deletion - update desired column
                self.handle_input_key_event_and_update_column(key_event)?;
                Ok(true)
            }
            KeyCode::Home | KeyCode::End => {
                // Handle home/end keys - update desired column
                self.handle_input_key_event_and_update_column(key_event)?;
                Ok(true)
            }
            _ => {
                // For all other keys (including text input), handle normally and update desired column
                // This catches regular text input while avoiding conflicts with vim keys handled above
                self.handle_input_key_event_and_update_column(key_event)?;
                Ok(true)
            }
        }
    }

    fn render(&mut self, frame: &mut TuiFrame, area: Rect) {
        let popup_area = centered_rect(60, 50, area);
        frame.render_widget(Clear, popup_area);

        // Render background
        let bg_block = Block::default().style(Style::default().bg(NORMAL_BG));
        frame.render_widget(bg_block, popup_area);

        let chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Title field
                Constraint::Length(8), // Description field
                Constraint::Length(4), // Date field + error message
                Constraint::Length(4), // Time field + error message
                Constraint::Min(1),    // Help text
            ])
            .split(popup_area);
        // Title field
        let title_border_color = if self.current_input_field == InputField::Title
            && self.is_current_editor_in_insert_mode()
        {
            if self.is_edit_mode {
                BORDER_EDIT
            } else {
                BORDER_NEW
            }
        } else {
            BORDER_PROCESSING
        };

        let title_block = Block::default()
            .title("Title")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(title_border_color))
            .style(Style::default().bg(NORMAL_BG));

        let title_inner = title_block.inner(chunks[0]);
        frame.render_widget(title_block, chunks[0]);

        let title_theme = if self.current_input_field == InputField::Title {
            EditorTheme::default()
                .base(Style::default().bg(NORMAL_BG).fg(TEXT_FG))
                .cursor_style(Style::default().bg(TEXT_FG).fg(NORMAL_BG))
                .selection_style(Style::default().bg(SELECTED_BG).fg(TEXT_FG))
                .hide_status_line()
        } else {
            EditorTheme::default()
                .base(Style::default().bg(NORMAL_BG).fg(TEXT_FG))
                .hide_status_line()
                .hide_cursor()
        };
        let title_editor_view = EditorView::new(&mut self.input_title_editor).theme(title_theme);
        frame.render_widget(title_editor_view, title_inner);

        // Description field
        let description_border_color = if self.current_input_field == InputField::Description
            && self.is_current_editor_in_insert_mode()
        {
            if self.is_edit_mode {
                BORDER_EDIT
            } else {
                BORDER_NEW
            }
        } else {
            BORDER_PROCESSING
        };

        let description_block = Block::default()
            .title("Description")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(description_border_color))
            .style(Style::default().bg(NORMAL_BG));

        let description_inner = description_block.inner(chunks[1]);
        frame.render_widget(description_block, chunks[1]);

        let description_theme = if self.current_input_field == InputField::Description {
            EditorTheme::default()
                .base(Style::default().bg(NORMAL_BG).fg(TEXT_FG))
                .cursor_style(Style::default().bg(TEXT_FG).fg(NORMAL_BG))
                .selection_style(Style::default().bg(SELECTED_BG).fg(TEXT_FG))
                .hide_status_line()
        } else {
            EditorTheme::default()
                .base(Style::default().bg(NORMAL_BG).fg(TEXT_FG))
                .hide_status_line()
                .hide_cursor()
        };
        let description_editor_view =
            EditorView::new(&mut self.input_description_editor).theme(description_theme);
        frame.render_widget(description_editor_view, description_inner);

        // Date field with error message layout
        let date_field_layout = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Date input
                Constraint::Length(1), // Error message
            ])
            .split(chunks[2]);

        let date_has_error = self.validation_attempted && self.date_error.is_some();
        let date_border_color = if date_has_error {
            ACCENT_RED
        } else if self.current_input_field == InputField::Date
            && self.is_current_editor_in_insert_mode()
        {
            if self.is_edit_mode {
                BORDER_EDIT
            } else {
                BORDER_NEW
            }
        } else {
            BORDER_PROCESSING
        };

        let date_block = Block::default()
            .title("Date (MM/DD/YYYY)")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(date_border_color))
            .style(Style::default().bg(NORMAL_BG));

        let date_inner = date_block.inner(date_field_layout[0]);
        frame.render_widget(date_block, date_field_layout[0]);

        let date_theme = if self.current_input_field == InputField::Date {
            EditorTheme::default()
                .base(Style::default().bg(NORMAL_BG).fg(TEXT_FG))
                .cursor_style(Style::default().bg(TEXT_FG).fg(NORMAL_BG))
                .selection_style(Style::default().bg(SELECTED_BG).fg(TEXT_FG))
                .hide_status_line()
        } else {
            EditorTheme::default()
                .base(Style::default().bg(NORMAL_BG).fg(TEXT_FG))
                .hide_status_line()
                .hide_cursor()
        };
        let date_editor_view = EditorView::new(&mut self.input_date_editor).theme(date_theme);
        frame.render_widget(date_editor_view, date_inner);

        // Render date error message if present
        if let Some(error) = &self.date_error {
            let error_paragraph =
                Paragraph::new(error.as_str()).style(Style::default().fg(ACCENT_RED).bg(NORMAL_BG));
            frame.render_widget(error_paragraph, date_field_layout[1]);
        }

        // Time field with error message layout
        let time_field_layout = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Time input
                Constraint::Length(1), // Error message
            ])
            .split(chunks[3]);

        let time_has_error = self.validation_attempted && self.time_error.is_some();
        let time_border_color = if time_has_error {
            ACCENT_RED
        } else if self.current_input_field == InputField::Time
            && self.is_current_editor_in_insert_mode()
        {
            if self.is_edit_mode {
                BORDER_EDIT
            } else {
                BORDER_NEW
            }
        } else {
            BORDER_PROCESSING
        };

        let time_block = Block::default()
            .title("Time (HH:MM AM/PM)")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(time_border_color))
            .style(Style::default().bg(NORMAL_BG));

        let time_inner = time_block.inner(time_field_layout[0]);
        frame.render_widget(time_block, time_field_layout[0]);

        let time_theme = if self.current_input_field == InputField::Time {
            EditorTheme::default()
                .base(Style::default().bg(NORMAL_BG).fg(TEXT_FG))
                .cursor_style(Style::default().bg(TEXT_FG).fg(NORMAL_BG))
                .selection_style(Style::default().bg(SELECTED_BG).fg(TEXT_FG))
                .hide_status_line()
        } else {
            EditorTheme::default()
                .base(Style::default().bg(NORMAL_BG).fg(TEXT_FG))
                .hide_status_line()
                .hide_cursor()
        };
        let time_editor_view = EditorView::new(&mut self.input_time_editor).theme(time_theme);
        frame.render_widget(time_editor_view, time_inner);

        // Render time error message if present
        if let Some(error) = &self.time_error {
            let error_paragraph =
                Paragraph::new(error.as_str()).style(Style::default().fg(ACCENT_RED).bg(NORMAL_BG));
            frame.render_widget(error_paragraph, time_field_layout[1]);
        }

        // Help text at bottom of modal
        let help_text = vec![Line::from(vec![
            Span::styled("Tab", Style::default().fg(ACCENT_YELLOW)),
            Span::raw(" switch fields  •  "),
            Span::styled("Enter", Style::default().fg(ACCENT_GREEN)),
            Span::raw(" confirm  •  "),
            Span::styled("Esc", Style::default().fg(ACCENT_RED)),
            Span::raw(" cancel"),
        ])];
        let help_paragraph = Paragraph::new(help_text)
            .style(Style::default().bg(NORMAL_BG))
            .alignment(Alignment::Center);

        frame.render_widget(help_paragraph, chunks[4]);

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
        vec![
            self.get_input_value(),
            self.get_input_description(),
            self.get_input_date(),
            self.get_input_time(),
        ]
    }

    fn clear_inputs(&mut self) {
        self.input_title_editor = EditorState::default();
        self.input_description_editor = EditorState::default();
        self.input_date_editor = EditorState::default();
        self.input_time_editor = EditorState::default();
        self.current_input_field = InputField::Title;
        self.validation_attempted = false;
        self.date_error = None;
        self.time_error = None;
    }

    fn set_values(&mut self, values: Vec<String>) {
        if values.len() >= 1 && !values[0].is_empty() {
            self.input_title_editor = EditorState::new(Lines::from(values[0].clone()));
        }
        if values.len() >= 2 && !values[1].is_empty() {
            self.input_description_editor = EditorState::new(Lines::from(values[1].clone()));
        }
        if values.len() >= 3 && !values[2].is_empty() {
            self.input_date_editor = EditorState::new(Lines::from(values[2].clone()));
        }
        if values.len() >= 4 && !values[3].is_empty() {
            self.input_time_editor = EditorState::new(Lines::from(values[3].clone()));
        }
    }

    fn validate(&mut self) -> bool {
        use crate::utils::{parse_date_us_format, parse_time_us_format};

        self.validation_attempted = true;
        let mut is_valid = true;

        // Validate date field (only if not empty)
        let date_str = self.get_input_date();
        if !date_str.trim().is_empty() {
            if let Err(err) = parse_date_us_format(&date_str) {
                self.date_error = Some(err);
                is_valid = false;
            } else {
                self.date_error = None;
            }
        } else {
            self.date_error = None;
        }

        // Validate time field (only if not empty)
        let time_str = self.get_input_time();
        if !time_str.trim().is_empty() {
            if let Err(err) = parse_time_us_format(&time_str) {
                self.time_error = Some(err);
                is_valid = false;
            } else {
                self.time_error = None;
            }
        } else {
            self.time_error = None;
        }

        is_valid
    }

    fn has_validation_errors(&self) -> bool {
        self.date_error.is_some() || self.time_error.is_some()
    }
}

/// Modal for confirming destructive actions like deleting tasks
pub struct ConfirmationModal {
    title: String,
    message: String,
}

impl ConfirmationModal {
    pub fn new(title: &str, message: &str) -> Self {
        Self {
            title: title.to_string(),
            message: message.to_string(),
        }
    }
}

impl Modal for ConfirmationModal {
    fn title(&self) -> &str {
        &self.title
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

        // Render the modal border
        let modal_block = Block::default()
            .title(self.title.as_str())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_DANGER))
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

/// Modal for postponing tasks with duration expression
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
        let popup_area = centered_rect(60, 27, area);
        frame.render_widget(Clear, popup_area);

        // Render background
        let bg_block = Block::default().style(Style::default().bg(NORMAL_BG));
        frame.render_widget(bg_block, popup_area);

        let chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(4), // Duration field + error message
                Constraint::Min(1),    // Instructions
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

        // Instructions
        let instructions = Paragraph::new(vec![
            Line::from(vec![Span::styled(
                "Examples:",
                Style::default()
                    .fg(ACCENT_YELLOW)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![
                Span::raw("  • "),
                Span::styled("5min", Style::default().fg(ACCENT_CYAN)),
                Span::raw(" or "),
                Span::styled("5 minutes", Style::default().fg(ACCENT_CYAN)),
                Span::raw(" - 5 minutes from due date"),
            ]),
            Line::from(vec![
                Span::raw("  • "),
                Span::styled("2hr", Style::default().fg(ACCENT_CYAN)),
                Span::raw(" or "),
                Span::styled("2 hours", Style::default().fg(ACCENT_CYAN)),
                Span::raw(" - 2 hours from due date"),
            ]),
            Line::from(vec![
                Span::raw("  • "),
                Span::styled("1day", Style::default().fg(ACCENT_CYAN)),
                Span::raw(" or "),
                Span::styled("1 day", Style::default().fg(ACCENT_CYAN)),
                Span::raw(" - 1 day from due date"),
            ]),
            Line::from(vec![
                Span::raw("  • "),
                Span::styled("now", Style::default().fg(ACCENT_CYAN)),
                Span::raw(" - Set to current time"),
            ]),
            Line::from(vec![
                Span::raw("  • "),
                Span::styled("now + 30min", Style::default().fg(ACCENT_CYAN)),
                Span::raw(" - 30 minutes from current time"),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "Enter",
                    Style::default()
                        .fg(ACCENT_GREEN)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" to postpone, "),
                Span::styled(
                    "Esc",
                    Style::default().fg(ACCENT_RED).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" to cancel"),
            ]),
        ])
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(TEXT_FG).bg(NORMAL_BG));

        frame.render_widget(instructions, chunks[1]);

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
