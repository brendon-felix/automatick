use anyhow::Result;
use crossterm::event::KeyEvent;
use edtui::{EditorEventHandler, EditorMode, EditorState, EditorTheme, EditorView, Index2, Lines};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use super::super::colors::*;
use super::super::tui::Frame as TuiFrame;
use super::super::{centered_rect, InputField};
use super::Modal;

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
            InputField::Title => InputField::Date,
            InputField::Date => InputField::Time,
            InputField::Time => InputField::Description,
            InputField::Description => InputField::Title,
        };
        // Ensure mode is preserved when switching fields
        if self.is_edit_mode {
            self.set_current_editor_to_normal_mode();
        }
    }

    pub fn previous_input_field(&mut self) {
        self.current_input_field = match self.current_input_field {
            InputField::Title => InputField::Description,
            InputField::Date => InputField::Title,
            InputField::Time => InputField::Date,
            InputField::Description => InputField::Time,
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
            // Go to last field (Description)
            self.current_input_field = InputField::Description;
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

    fn handle_j_navigation_no_wrap(&mut self) -> bool {
        if self.current_input_field == InputField::Description
            && !self.is_at_last_line_in_multiline_field()
        {
            // Stay in description field, let editor handle j
            return false;
        }

        // Move to next field (no wraparound)
        match self.current_input_field {
            InputField::Title => {
                self.current_input_field = InputField::Date;
                self.position_cursor_at_desired_column(0);
                if self.is_edit_mode {
                    self.set_current_editor_to_normal_mode();
                }
                true
            }
            InputField::Date => {
                self.current_input_field = InputField::Time;
                self.position_cursor_at_desired_column(0);
                if self.is_edit_mode {
                    self.set_current_editor_to_normal_mode();
                }
                true
            }
            InputField::Time => {
                self.current_input_field = InputField::Description;
                self.position_cursor_at_desired_column(0);
                if self.is_edit_mode {
                    self.set_current_editor_to_normal_mode();
                }
                true
            }
            InputField::Description => {
                // Don't wrap around, stay at description field
                true
            }
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

    fn handle_k_navigation_no_wrap(&mut self) -> bool {
        if self.current_input_field == InputField::Description
            && !self.is_at_first_line_in_multiline_field()
        {
            // Stay in description field, let editor handle k
            return false;
        }

        // Move to previous field (no wraparound)
        match self.current_input_field {
            InputField::Title => {
                // Don't wrap around, stay at title field
                true
            }
            InputField::Date => {
                self.current_input_field = InputField::Title;
                self.position_cursor_at_desired_column(0);
                if self.is_edit_mode {
                    self.set_current_editor_to_normal_mode();
                }
                true
            }
            InputField::Time => {
                self.current_input_field = InputField::Date;
                self.position_cursor_at_desired_column(0);
                if self.is_edit_mode {
                    self.set_current_editor_to_normal_mode();
                }
                true
            }
            InputField::Description => {
                self.current_input_field = InputField::Time;
                self.position_cursor_at_desired_column(0);
                if self.is_edit_mode {
                    self.set_current_editor_to_normal_mode();
                }
                true
            }
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

    fn as_any(&self) -> &dyn std::any::Any {
        self
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
                // Only handle j navigation in Normal mode (no wraparound)
                if !self.is_current_editor_in_insert_mode() {
                    if self.handle_j_navigation_no_wrap() {
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
                // Only handle k navigation in Normal mode (no wraparound)
                if !self.is_current_editor_in_insert_mode() {
                    if self.handle_k_navigation_no_wrap() {
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
                // Only handle Down navigation in Normal mode (with wraparound)
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
                // Only handle Up navigation in Normal mode (with wraparound)
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
                Constraint::Length(4), // Date field + error message
                Constraint::Length(4), // Time field + error message
                Constraint::Min(5),    // Description field (remaining space, min 5 lines)
                Constraint::Length(1), // Help text
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

        // Date field with error message layout
        let date_field_layout = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Date input
                Constraint::Length(1), // Error message
            ])
            .split(chunks[1]);

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
            .split(chunks[2]);

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

        let description_inner = description_block.inner(chunks[3]);
        frame.render_widget(description_block, chunks[3]);

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
