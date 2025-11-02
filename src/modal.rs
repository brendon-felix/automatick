use anyhow::Result;
use crossterm::event::KeyEvent;
use edtui::{EditorEventHandler, EditorMode, EditorState, EditorTheme, EditorView, Index2, Lines};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

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
        // Colors
        const BORDER_NORMAL: Color = Color::Rgb(100, 100, 100);
        const BORDER_INSERT: Color = Color::Green;
        const BORDER_EDIT: Color = Color::Blue;
        const TEXT_FG: Color = Color::White;

        let popup_area = centered_rect(60, 40, area);
        frame.render_widget(Clear, popup_area);

        let chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Title field
                Constraint::Length(8), // Description field
                Constraint::Length(3), // Date field
                Constraint::Length(3), // Time field
            ])
            .split(popup_area);

        // Title field
        let title_border_color = if self.current_input_field == InputField::Title
            && self.is_current_editor_in_insert_mode()
        {
            BORDER_INSERT
        } else {
            BORDER_NORMAL
        };

        let title_block = Block::default()
            .title("Title")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(title_border_color));

        let title_inner = title_block.inner(chunks[0]);
        frame.render_widget(title_block, chunks[0]);

        let title_theme = if self.current_input_field == InputField::Title {
            EditorTheme::default().hide_status_line()
        } else {
            EditorTheme::default().hide_status_line().hide_cursor()
        };
        let title_editor_view = EditorView::new(&mut self.input_title_editor).theme(title_theme);
        frame.render_widget(title_editor_view, title_inner);

        // Description field
        let description_border_color = if self.current_input_field == InputField::Description
            && self.is_current_editor_in_insert_mode()
        {
            BORDER_INSERT
        } else {
            BORDER_NORMAL
        };

        let description_block = Block::default()
            .title("Description")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(description_border_color));

        let description_inner = description_block.inner(chunks[1]);
        frame.render_widget(description_block, chunks[1]);

        let description_theme = if self.current_input_field == InputField::Description {
            EditorTheme::default().hide_status_line()
        } else {
            EditorTheme::default().hide_status_line().hide_cursor()
        };
        let description_editor_view =
            EditorView::new(&mut self.input_description_editor).theme(description_theme);
        frame.render_widget(description_editor_view, description_inner);

        // Date field
        let date_border_color = if self.current_input_field == InputField::Date
            && self.is_current_editor_in_insert_mode()
        {
            BORDER_INSERT
        } else {
            BORDER_NORMAL
        };

        let date_block = Block::default()
            .title("Date (MM/DD/YYYY)")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(date_border_color));

        let date_inner = date_block.inner(chunks[2]);
        frame.render_widget(date_block, chunks[2]);

        let date_theme = if self.current_input_field == InputField::Date {
            EditorTheme::default().hide_status_line()
        } else {
            EditorTheme::default().hide_status_line().hide_cursor()
        };
        let date_editor_view = EditorView::new(&mut self.input_date_editor).theme(date_theme);
        frame.render_widget(date_editor_view, date_inner);

        // Time field
        let time_border_color = if self.current_input_field == InputField::Time
            && self.is_current_editor_in_insert_mode()
        {
            BORDER_INSERT
        } else {
            BORDER_NORMAL
        };

        let time_block = Block::default()
            .title("Time (HH:MM AM/PM)")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(time_border_color));

        let time_inner = time_block.inner(chunks[3]);
        frame.render_widget(time_block, chunks[3]);

        let time_theme = if self.current_input_field == InputField::Time {
            EditorTheme::default().hide_status_line()
        } else {
            EditorTheme::default().hide_status_line().hide_cursor()
        };
        let time_editor_view = EditorView::new(&mut self.input_time_editor).theme(time_theme);
        frame.render_widget(time_editor_view, time_inner);

        // Help text at bottom center of entire window
        let help_text = "Tab: switch fields  •  j/k: navigate  •  h/l: move cursor  •  Enter: confirm  •  Esc: cancel";
        let help_paragraph = Paragraph::new(help_text)
            .style(Style::default().fg(TEXT_FG))
            .alignment(Alignment::Center);

        // Position help text at bottom of the screen
        let help_area = Rect {
            x: area.x,
            y: area.y + area.height - 1,
            width: area.width,
            height: 1,
        };

        frame.render_widget(help_paragraph, help_area);

        // Render the modal border
        let modal_border_color = if self.is_edit_mode {
            BORDER_EDIT
        } else {
            BORDER_INSERT
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

    fn render(&mut self, frame: &mut TuiFrame, area: Rect) {
        const BORDER_NORMAL: Color = Color::Red;
        const TEXT_FG: Color = Color::White;

        // Message content
        let message_lines: Vec<Line> = self
            .message
            .split('\n')
            .map(|line| Line::from(Span::styled(line, Style::default().fg(TEXT_FG))))
            .collect();

        let instructions = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "y",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" to confirm, "),
                Span::styled(
                    "n/Esc",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
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
            .style(Style::default().fg(TEXT_FG))
            .alignment(Alignment::Center);

        // Render the modal border
        let modal_block = Block::default()
            .title(self.title.as_str())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_NORMAL));

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
}

impl PostponeModal {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            input_duration_editor: EditorState::default(),
            event_handler: EditorEventHandler::default(),
        }
    }

    pub fn new_with_default(title: &str, default_duration: Option<String>) -> Self {
        let mut modal = Self::new(title);

        if let Some(duration) = default_duration {
            modal.input_duration_editor = EditorState::new(Lines::from(duration));
        }

        // Set initial mode and cursor position
        modal.set_editor_to_insert_mode();
        modal.position_cursor_at_end();

        modal
    }

    pub fn set_editor_to_insert_mode(&mut self) {
        self.input_duration_editor.mode = EditorMode::Insert;
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
        Ok(())
    }
}

impl Modal for PostponeModal {
    fn title(&self) -> &str {
        &self.title
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<bool> {
        self.handle_input_key_event(key_event)?;
        Ok(true)
    }

    fn render(&mut self, frame: &mut TuiFrame, area: Rect) {
        // Colors
        const BORDER_INSERT: Color = Color::Green;
        const TEXT_FG: Color = Color::White;

        let popup_area = centered_rect(60, 25, area);
        frame.render_widget(Clear, popup_area);

        let chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Duration field
                Constraint::Min(1),    // Instructions
            ])
            .split(popup_area);

        // Duration field
        let duration_border_color = if self.is_editor_in_insert_mode() {
            BORDER_INSERT
        } else {
            Color::Rgb(100, 100, 100)
        };

        let duration_block = Block::default()
            .title("Duration (e.g., \"5min\", \"2 hours\", \"1day\", \"now\", \"now + 30min\")")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(duration_border_color));

        let duration_inner = duration_block.inner(chunks[0]);
        frame.render_widget(duration_block, chunks[0]);

        let duration_theme = EditorTheme::default().hide_status_line();
        let duration_editor_view =
            EditorView::new(&mut self.input_duration_editor).theme(duration_theme);
        frame.render_widget(duration_editor_view, duration_inner);

        // Instructions
        let instructions = Paragraph::new(vec![
            Line::from(vec![Span::styled(
                "Examples:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![
                Span::raw("  • "),
                Span::styled("5min", Style::default().fg(Color::Cyan)),
                Span::raw(" or "),
                Span::styled("5 minutes", Style::default().fg(Color::Cyan)),
                Span::raw(" - 5 minutes from due date"),
            ]),
            Line::from(vec![
                Span::raw("  • "),
                Span::styled("2hr", Style::default().fg(Color::Cyan)),
                Span::raw(" or "),
                Span::styled("2 hours", Style::default().fg(Color::Cyan)),
                Span::raw(" - 2 hours from due date"),
            ]),
            Line::from(vec![
                Span::raw("  • "),
                Span::styled("1day", Style::default().fg(Color::Cyan)),
                Span::raw(" or "),
                Span::styled("1 day", Style::default().fg(Color::Cyan)),
                Span::raw(" - 1 day from due date"),
            ]),
            Line::from(vec![
                Span::raw("  • "),
                Span::styled("now", Style::default().fg(Color::Cyan)),
                Span::raw(" - Set to current time"),
            ]),
            Line::from(vec![
                Span::raw("  • "),
                Span::styled("now + 30min", Style::default().fg(Color::Cyan)),
                Span::raw(" - 30 minutes from current time"),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "Enter",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" to postpone, "),
                Span::styled(
                    "Esc",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" to cancel"),
            ]),
        ])
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(TEXT_FG));

        frame.render_widget(instructions, chunks[1]);

        // Render the modal border
        let modal_block = Block::default()
            .title(self.title.as_str())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_INSERT));

        frame.render_widget(modal_block, popup_area);
    }

    fn get_values(&self) -> Vec<String> {
        vec![self.get_input_duration()]
    }

    fn clear_inputs(&mut self) {
        self.input_duration_editor = EditorState::default();
    }

    fn set_values(&mut self, values: Vec<String>) {
        if values.len() >= 1 && !values[0].is_empty() {
            self.input_duration_editor = EditorState::new(Lines::from(values[0].clone()));
        }
    }
}
