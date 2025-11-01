use anyhow::Result;
use crossterm::event::KeyEvent;
use edtui::{EditorEventHandler, EditorMode, EditorState, EditorTheme, EditorView, Index2, Lines};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::tui::Frame as TuiFrame;
use crate::ui::{centered_rect, InputField};

/// Trait for modal dialogs that can be displayed as overlays
pub trait Modal {
    /// Get the title of the modal
    #[allow(dead_code)]
    fn title(&self) -> &str;

    /// Handle key events for the modal
    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<bool>;

    /// Render the modal content
    fn render(&mut self, frame: &mut TuiFrame, area: Rect);

    /// Get the modal's input values (if any)
    fn get_values(&self) -> Vec<String>;

    /// Clear all inputs
    #[allow(dead_code)]
    fn clear_inputs(&mut self);

    /// Set initial values
    #[allow(dead_code)]
    fn set_values(&mut self, values: Vec<String>);
}

/// Modal for creating and editing tasks
pub struct TaskModal {
    title: String,
    input_title_editor: EditorState,
    input_date_editor: EditorState,
    input_time_editor: EditorState,
    current_input_field: InputField,
    event_handler: EditorEventHandler,
}

impl TaskModal {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            input_title_editor: EditorState::default(),
            input_date_editor: EditorState::default(),
            input_time_editor: EditorState::default(),
            current_input_field: InputField::Title,
            event_handler: EditorEventHandler::default(),
        }
    }

    pub fn new_with_defaults(
        title: &str,
        default_title: Option<String>,
        default_date: Option<String>,
        default_time: Option<String>,
    ) -> Self {
        let mut modal = Self::new(title);

        if let Some(task_title) = default_title {
            modal.input_title_editor = EditorState::new(Lines::from(task_title));
        }

        if let Some(date) = default_date {
            modal.input_date_editor = EditorState::new(Lines::from(date));
        }

        if let Some(time) = default_time {
            modal.input_time_editor = EditorState::new(Lines::from(time));
        }

        // Set initial mode and cursor position
        modal.set_current_editor_to_insert_mode();
        modal.position_cursor_at_end();

        modal
    }

    fn get_current_editor_mut(&mut self) -> &mut EditorState {
        match self.current_input_field {
            InputField::Title => &mut self.input_title_editor,
            InputField::Date => &mut self.input_date_editor,
            InputField::Time => &mut self.input_time_editor,
        }
    }

    fn get_current_editor(&self) -> &EditorState {
        match self.current_input_field {
            InputField::Title => &self.input_title_editor,
            InputField::Date => &self.input_date_editor,
            InputField::Time => &self.input_time_editor,
        }
    }

    pub fn next_input_field(&mut self) {
        self.current_input_field = match self.current_input_field {
            InputField::Title => InputField::Date,
            InputField::Date => InputField::Time,
            InputField::Time => InputField::Title,
        };
    }

    pub fn previous_input_field(&mut self) {
        self.current_input_field = match self.current_input_field {
            InputField::Title => InputField::Time,
            InputField::Date => InputField::Title,
            InputField::Time => InputField::Date,
        };
    }

    pub fn set_current_editor_to_insert_mode(&mut self) {
        let editor = self.get_current_editor_mut();
        editor.mode = EditorMode::Insert;
    }

    pub fn position_cursor_at_end(&mut self) {
        let editor = self.get_current_editor_mut();

        // Position cursor at the end of the text
        if editor.lines.is_empty() {
            editor.cursor = Index2::new(0, 0);
        } else if let Some(last_row_idx) = editor.lines.len().checked_sub(1) {
            if let Some(last_col) = editor.lines.len_col(last_row_idx) {
                editor.cursor = Index2::new(last_row_idx, last_col);
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
}

impl Modal for TaskModal {
    fn title(&self) -> &str {
        &self.title
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<bool> {
        use crossterm::event::KeyCode;

        match key_event.code {
            KeyCode::Tab => {
                self.next_input_field();
                self.set_current_editor_to_insert_mode();
                self.position_cursor_at_end();
                Ok(true)
            }
            KeyCode::BackTab => {
                self.previous_input_field();
                self.set_current_editor_to_insert_mode();
                self.position_cursor_at_end();
                Ok(true)
            }
            _ => {
                self.handle_input_key_event(key_event)?;
                Ok(true)
            }
        }
    }

    fn render(&mut self, frame: &mut TuiFrame, area: Rect) {
        // Colors
        const BORDER_NORMAL: Color = Color::Rgb(100, 100, 100);
        const BORDER_INSERT: Color = Color::Green;
        const TEXT_FG: Color = Color::White;

        let popup_area = centered_rect(60, 30, area);
        frame.render_widget(Clear, popup_area);

        let chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Title field
                Constraint::Length(3), // Date field
                Constraint::Length(3), // Time field
                Constraint::Min(1),    // Instructions
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

        let date_inner = date_block.inner(chunks[1]);
        frame.render_widget(date_block, chunks[1]);

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

        let time_inner = time_block.inner(chunks[2]);
        frame.render_widget(time_block, chunks[2]);

        let time_theme = if self.current_input_field == InputField::Time {
            EditorTheme::default().hide_status_line()
        } else {
            EditorTheme::default().hide_status_line().hide_cursor()
        };
        let time_editor_view = EditorView::new(&mut self.input_time_editor).theme(time_theme);
        frame.render_widget(time_editor_view, time_inner);

        // Instructions
        let instructions = Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    "Tab",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" / "),
                Span::styled(
                    "Shift+Tab",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" to switch fields"),
            ]),
            Line::from(vec![
                Span::styled(
                    "Enter",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" to confirm, "),
                Span::styled(
                    "Esc",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" to cancel"),
            ]),
        ])
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(TEXT_FG));

        frame.render_widget(instructions, chunks[3]);

        // Render the modal border
        let modal_block = Block::default()
            .title(self.title.as_str())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_INSERT));

        frame.render_widget(modal_block, popup_area);
    }

    fn get_values(&self) -> Vec<String> {
        vec![
            self.get_input_value(),
            self.get_input_date(),
            self.get_input_time(),
        ]
    }

    fn clear_inputs(&mut self) {
        self.input_title_editor = EditorState::default();
        self.input_date_editor = EditorState::default();
        self.input_time_editor = EditorState::default();
        self.current_input_field = InputField::Title;
    }

    fn set_values(&mut self, values: Vec<String>) {
        if values.len() >= 1 && !values[0].is_empty() {
            self.input_title_editor = EditorState::new(Lines::from(values[0].clone()));
        }
        if values.len() >= 2 && !values[1].is_empty() {
            self.input_date_editor = EditorState::new(Lines::from(values[1].clone()));
        }
        if values.len() >= 3 && !values[2].is_empty() {
            self.input_time_editor = EditorState::new(Lines::from(values[2].clone()));
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

        let popup_area = centered_rect(50, 20, area);
        frame.render_widget(Clear, popup_area);

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

        let message_paragraph = Paragraph::new(all_lines)
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(TEXT_FG))
            .alignment(ratatui::layout::Alignment::Center);

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
