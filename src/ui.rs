use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use crossterm::event::KeyEvent;
use edtui::{EditorEventHandler, EditorMode, EditorState, EditorTheme, EditorView, Index2, Lines};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Padding, Paragraph},
};
use ticks::tasks::{Task, TaskPriority};

use crate::app::Mode;
use crate::colors::*;
use crate::modal::Modal;
use crate::tui::Frame as TuiFrame;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewTab {
    Today,
    Week,
    Inbox,
}

// Color constants are now centralized in colors.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputField {
    Title,
    Description,
    Date,
    Time,
}

pub struct TaskEditor {
    pub input_title_editor: EditorState,
    pub input_description_editor: EditorState,
    pub input_date_editor: EditorState,
    pub input_time_editor: EditorState,
    pub current_input_field: InputField,
    pub event_handler: EditorEventHandler,
    pub is_edit_mode: bool,
    pub desired_column: usize,
    // Validation state
    pub validation_attempted: bool,
    pub date_error: Option<String>,
    pub time_error: Option<String>,
    // Original values for change detection
    pub original_title: String,
    pub original_description: String,
    pub original_date: String,
    pub original_time: String,
}

pub struct TaskListUI {
    state: ListState,
    pub current_tab: ViewTab,
    pub visual_range: Option<(usize, usize)>,
    pub current_modal: Option<Box<dyn Modal>>,
    pub task_editor: TaskEditor,
}

impl TaskEditor {
    pub fn new() -> Self {
        Self {
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
            original_title: String::new(),
            original_description: String::new(),
            original_date: String::new(),
            original_time: String::new(),
        }
    }

    #[allow(dead_code)]
    pub fn clear_inputs(&mut self) {
        self.input_title_editor = EditorState::default();
        self.input_description_editor = EditorState::default();
        self.input_date_editor = EditorState::default();
        self.input_time_editor = EditorState::default();
        self.current_input_field = InputField::Title;
        self.validation_attempted = false;
        self.date_error = None;
        self.time_error = None;
        self.original_title = String::new();
        self.original_description = String::new();
        self.original_date = String::new();
        self.original_time = String::new();
    }

    pub fn set_values(&mut self, title: &str, description: &str, date: &str, time: &str) {
        self.input_title_editor = EditorState::new(Lines::from(title.to_string()));
        self.input_description_editor = EditorState::new(Lines::from(description.to_string()));
        self.input_date_editor = EditorState::new(Lines::from(date.to_string()));
        self.input_time_editor = EditorState::new(Lines::from(time.to_string()));

        // Store original values for change detection
        self.original_title = title.to_string();
        self.original_description = description.to_string();
        self.original_date = date.to_string();
        self.original_time = time.to_string();
    }

    pub fn get_current_editor_mut(&mut self) -> &mut EditorState {
        match self.current_input_field {
            InputField::Title => &mut self.input_title_editor,
            InputField::Description => &mut self.input_description_editor,
            InputField::Date => &mut self.input_date_editor,
            InputField::Time => &mut self.input_time_editor,
        }
    }

    pub fn next_input_field(&mut self) {
        self.current_input_field = match self.current_input_field {
            InputField::Title => InputField::Description,
            InputField::Description => InputField::Date,
            InputField::Date => InputField::Time,
            InputField::Time => InputField::Title,
        };
    }

    pub fn previous_input_field(&mut self) {
        self.current_input_field = match self.current_input_field {
            InputField::Title => InputField::Time,
            InputField::Description => InputField::Title,
            InputField::Date => InputField::Description,
            InputField::Time => InputField::Date,
        };
    }

    pub fn is_current_editor_in_insert_mode(&self) -> bool {
        let editor = match self.current_input_field {
            InputField::Title => &self.input_title_editor,
            InputField::Description => &self.input_description_editor,
            InputField::Date => &self.input_date_editor,
            InputField::Time => &self.input_time_editor,
        };
        editor.mode == EditorMode::Insert
    }

    #[allow(dead_code)]
    pub fn set_current_editor_to_insert_mode(&mut self) {
        let editor = self.get_current_editor_mut();
        editor.mode = EditorMode::Insert;
    }

    #[allow(dead_code)]
    pub fn set_current_editor_to_normal_mode(&mut self) {
        let editor = self.get_current_editor_mut();
        editor.mode = EditorMode::Normal;
    }

    pub fn position_cursor_at_end(&mut self) {
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
    }

    pub fn handle_input_key_event(&mut self, key_event: crossterm::event::KeyEvent) -> Result<()> {
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

    pub fn handle_input_key_event_and_update_column(
        &mut self,
        key_event: crossterm::event::KeyEvent,
    ) -> Result<()> {
        self.handle_input_key_event(key_event)?;
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

    pub fn update_desired_column(&mut self) {
        let editor = match self.current_input_field {
            InputField::Title => &self.input_title_editor,
            InputField::Description => &self.input_description_editor,
            InputField::Date => &self.input_date_editor,
            InputField::Time => &self.input_time_editor,
        };
        self.desired_column = editor.cursor.col;
    }

    pub fn position_cursor_at_desired_column(&mut self) {
        let desired_column = self.desired_column;
        let editor = self.get_current_editor_mut();
        if let Some(line_len) = editor.lines.len_col(editor.cursor.row) {
            editor.cursor.col = desired_column.min(line_len);
        }
    }

    pub fn is_at_first_line_in_multiline_field(&self) -> bool {
        match self.current_input_field {
            InputField::Description => self.input_description_editor.cursor.row == 0,
            _ => true, // Single-line fields are always at "first line"
        }
    }

    pub fn is_at_last_line_in_multiline_field(&self) -> bool {
        match self.current_input_field {
            InputField::Description => {
                let lines_len = self.input_description_editor.lines.len();
                self.input_description_editor.cursor.row >= lines_len.saturating_sub(1)
            }
            _ => true, // Single-line fields are always at "last line"
        }
    }

    pub fn navigate_to_first_field_or_line(&mut self) {
        if self.current_input_field == InputField::Description {
            // Move to first line in description field
            let editor = self.get_current_editor_mut();
            editor.cursor.row = 0;
            self.position_cursor_at_desired_column();
        } else {
            // Move to first field
            self.current_input_field = InputField::Title;
            self.position_cursor_at_end();
        }
    }

    pub fn navigate_to_last_field_or_line(&mut self) {
        if self.current_input_field == InputField::Description {
            // Move to last line in description field
            let editor = self.get_current_editor_mut();
            let lines_len = editor.lines.len();
            if lines_len > 0 {
                editor.cursor.row = lines_len - 1;
                self.position_cursor_at_desired_column();
            }
        } else {
            // Move to last field
            self.current_input_field = InputField::Time;
            self.position_cursor_at_end();
        }
    }

    pub fn handle_j_navigation(&mut self) -> bool {
        if self.current_input_field == InputField::Description
            && !self.is_at_last_line_in_multiline_field()
        {
            // Stay in description field, let editor handle j
            return false;
        }

        // Move to next field
        match self.current_input_field {
            InputField::Title => {
                self.current_input_field = InputField::Description;
                // Set cursor to first row of new field, then position at desired column
                let editor = self.get_current_editor_mut();
                editor.cursor.row = 0;
                self.position_cursor_at_desired_column();
                true
            }
            InputField::Description => {
                self.current_input_field = InputField::Date;
                // Set cursor to first row of new field, then position at desired column
                let editor = self.get_current_editor_mut();
                editor.cursor.row = 0;
                self.position_cursor_at_desired_column();
                true
            }
            InputField::Date => {
                self.current_input_field = InputField::Time;
                // Set cursor to first row of new field, then position at desired column
                let editor = self.get_current_editor_mut();
                editor.cursor.row = 0;
                self.position_cursor_at_desired_column();
                true
            }
            InputField::Time => {
                // Don't wrap around, stay at time field
                true
            }
        }
    }

    pub fn handle_k_navigation(&mut self) -> bool {
        if self.current_input_field == InputField::Description
            && !self.is_at_first_line_in_multiline_field()
        {
            // Stay in description field, let editor handle k
            return false;
        }

        // Move to previous field
        match self.current_input_field {
            InputField::Title => {
                // Don't wrap around, stay at title field
                true
            }
            InputField::Description => {
                self.current_input_field = InputField::Title;
                // For single-line fields, position at row 0
                let editor = self.get_current_editor_mut();
                editor.cursor.row = 0;
                self.position_cursor_at_desired_column();
                true
            }
            InputField::Date => {
                self.current_input_field = InputField::Description;
                // For description field, go to last line and position at desired column
                let editor = self.get_current_editor_mut();
                let lines_len = editor.lines.len();
                if lines_len > 0 {
                    editor.cursor.row = lines_len - 1;
                } else {
                    editor.cursor.row = 0;
                }
                self.position_cursor_at_desired_column();
                true
            }
            InputField::Time => {
                self.current_input_field = InputField::Date;
                // For single-line fields, position at row 0
                let editor = self.get_current_editor_mut();
                editor.cursor.row = 0;
                self.position_cursor_at_desired_column();
                true
            }
        }
    }

    pub fn get_input_title(&self) -> String {
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

    pub fn validate(&mut self) -> bool {
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

    pub fn has_changes(&self) -> bool {
        self.get_input_title() != self.original_title
            || self.get_input_description() != self.original_description
            || self.get_input_date() != self.original_date
            || self.get_input_time() != self.original_time
    }

    pub fn is_at_line_start(&self) -> bool {
        let editor = match self.current_input_field {
            InputField::Title => &self.input_title_editor,
            InputField::Description => &self.input_description_editor,
            InputField::Date => &self.input_date_editor,
            InputField::Time => &self.input_time_editor,
        };
        editor.cursor.col == 0
    }
}

impl TaskListUI {
    pub fn new() -> Self {
        Self {
            state: ListState::default(),
            current_tab: ViewTab::Today,
            visual_range: None,
            current_modal: None,
            task_editor: TaskEditor::new(),
        }
    }

    pub fn set_tasks(&mut self, tasks: &[Task]) {
        // Maintain selection if possible
        if let Some(selected) = self.state.selected() {
            if selected >= tasks.len() && !tasks.is_empty() {
                self.state.select(Some(tasks.len().saturating_sub(1)));
            }
        }
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.state.selected()
    }

    pub fn enter_visual_mode(&mut self) {
        if self.visual_range.is_none() {
            if let Some(idx) = self.state.selected() {
                self.visual_range = Some((idx, idx));
            } else {
                self.visual_range = Some((0, 0));
            }
        }
    }

    fn update_visual_range(&mut self) {
        if let Some(idx) = self.state.selected() {
            if let Some((start, _end)) = self.visual_range {
                self.visual_range = Some((start, idx));
            }
        }
    }

    pub fn exit_visual_mode(&mut self) {
        self.visual_range = None;
    }

    pub fn select_previous(&mut self, task_count: usize) {
        // Custom implementation that doesn't deselect at boundaries
        if task_count == 0 {
            return;
        }

        match self.state.selected() {
            Some(i) => {
                if i > 0 {
                    self.state.select(Some(i - 1));
                }
                // If i == 0, stay at first item (don't deselect)
            }
            None => {
                // No selection, select last item
                if task_count > 0 {
                    self.state.select(Some(task_count - 1));
                }
            }
        }
        if self.visual_range.is_some() {
            self.update_visual_range();
        }
    }

    pub fn select_next(&mut self, task_count: usize) {
        // Custom implementation that doesn't deselect at boundaries
        if task_count == 0 {
            return;
        }

        match self.state.selected() {
            Some(i) => {
                if i < task_count - 1 {
                    self.state.select(Some(i + 1));
                }
                // If at last item, stay at last item (don't deselect)
            }
            None => {
                // No selection, select first item
                if task_count > 0 {
                    self.state.select(Some(0));
                }
            }
        }
        if self.visual_range.is_some() {
            self.update_visual_range();
        }
    }

    pub fn select_previous_cycling(&mut self, task_count: usize) {
        if task_count == 0 {
            return;
        }

        match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    // At first item, cycle to last
                    self.state.select(Some(task_count - 1));
                } else {
                    self.state.select(Some(i - 1));
                }
            }
            None => {
                // No selection, select last item
                self.state.select(Some(task_count - 1));
            }
        }

        if self.visual_range.is_some() {
            self.update_visual_range();
        }
    }

    pub fn select_next_cycling(&mut self, task_count: usize) {
        if task_count == 0 {
            return;
        }

        match self.state.selected() {
            Some(i) => {
                if i >= task_count - 1 {
                    // At last item, cycle to first
                    self.state.select(Some(0));
                } else {
                    self.state.select(Some(i + 1));
                }
            }
            None => {
                // No selection, select first item
                self.state.select(Some(0));
            }
        }

        if self.visual_range.is_some() {
            self.update_visual_range();
        }
    }

    pub fn select_first(&mut self, task_count: usize) {
        if task_count > 0 {
            self.state.select(Some(0));
        }
        if self.visual_range.is_some() {
            self.update_visual_range();
        }
    }

    pub fn select_last(&mut self, task_count: usize) {
        if task_count > 0 {
            self.state.select(Some(task_count - 1));
        }
        if self.visual_range.is_some() {
            self.update_visual_range();
        }
    }

    pub fn select_none(&mut self) {
        self.state.select(None);
        if self.visual_range.is_some() {
            self.visual_range = Some((0, 0));
        }
    }

    pub fn start_modal<M: Modal + 'static>(&mut self, mut modal: M) {
        // Initialize the modal if it has methods for cursor positioning
        if let Some(task_modal) =
            (&mut modal as &mut dyn std::any::Any).downcast_mut::<crate::modal::TaskModal>()
        {
            // Position cursor at the end
            // Note: editor mode is already set correctly in TaskModal::new_with_defaults
            // based on is_edit_mode flag, so we don't override it here
            task_modal.position_cursor_at_end();
        }
        self.current_modal = Some(Box::new(modal));
    }

    pub fn close_modal(&mut self) {
        self.current_modal = None;
    }

    pub fn has_modal(&self) -> bool {
        self.current_modal.is_some()
    }

    pub fn get_modal_values(&self) -> Vec<String> {
        self.current_modal
            .as_ref()
            .map_or(Vec::new(), |modal| modal.get_values())
    }

    pub fn handle_modal_key_event(&mut self, key_event: KeyEvent) -> Result<bool> {
        if let Some(modal) = &mut self.current_modal {
            modal.handle_key_event(key_event)
        } else {
            Ok(false)
        }
    }

    pub fn validate_modal(&mut self) -> bool {
        if let Some(modal) = &mut self.current_modal {
            modal.validate()
        } else {
            true
        }
    }

    pub fn get_confirmation_type(&self) -> Option<&crate::modal::ConfirmationType> {
        if let Some(modal) = &self.current_modal {
            if let Some(confirmation_modal) = modal
                .as_ref()
                .as_any()
                .downcast_ref::<crate::modal::ConfirmationModal>()
            {
                Some(confirmation_modal.confirmation_type())
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn next_tab(&mut self) {
        self.current_tab = match self.current_tab {
            ViewTab::Today => ViewTab::Week,
            ViewTab::Week => ViewTab::Inbox,
            ViewTab::Inbox => ViewTab::Today,
        };
        // Clear selection when switching tabs
        self.select_none();
    }

    pub fn previous_tab(&mut self) {
        self.current_tab = match self.current_tab {
            ViewTab::Today => ViewTab::Inbox,
            ViewTab::Week => ViewTab::Today,
            ViewTab::Inbox => ViewTab::Week,
        };
        // Clear selection when switching tabs
        self.select_none();
    }

    pub fn get_current_tab(&self) -> ViewTab {
        self.current_tab
    }

    pub fn get_selected_indices(&self) -> Vec<usize> {
        if let Some((start, end)) = self.visual_range {
            let min_idx = start.min(end);
            let max_idx = start.max(end);
            (min_idx..=max_idx).collect()
        } else if let Some(idx) = self.state.selected() {
            vec![idx]
        } else {
            vec![]
        }
    }

    pub fn draw(
        &mut self,
        f: &mut TuiFrame,
        area: Rect,
        mode: Mode,
        tasks: &[Task],
        error_message: &Option<String>,
        tasks_loaded: bool,
        task_editor_focused: bool,
    ) -> Result<()> {
        // Set consistent background for entire screen
        let background = Block::default().style(Style::default().bg(NORMAL_BG));
        f.render_widget(background, area);

        // Main vertical layout: Header, Content, Footer
        let main_chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(10),   // Content area (task list + details)
                Constraint::Length(3), // Footer
            ])
            .split(area);

        // Content area split horizontally: Task list on left, Details on right
        let content_chunks = Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                Constraint::Percentage(60), // Task list
                Constraint::Percentage(40), // Task details
            ])
            .split(main_chunks[1]);

        self.render_header(f, main_chunks[0], mode, error_message);
        self.render_task_list(
            f,
            content_chunks[0],
            mode,
            tasks,
            tasks_loaded,
            task_editor_focused,
        );
        self.render_task_details(f, content_chunks[1], tasks, task_editor_focused);
        self.render_footer(f, main_chunks[2], mode);

        // Render overlays
        if let Some(modal) = &mut self.current_modal {
            modal.render(f, area);
        } else if mode == Mode::Help {
            self.render_help_overlay(f, area);
        }

        Ok(())
    }

    fn render_header(
        &self,
        f: &mut TuiFrame,
        area: Rect,
        mode: Mode,
        error_message: &Option<String>,
    ) {
        let title = if let Some(err) = error_message {
            format!("❌ Error: {}", err)
        } else {
            match mode {
                Mode::Processing => "⏳ Automatick - Processing...".to_string(),
                Mode::Insert => "✏️ Automatick - Insert Mode".to_string(),
                Mode::Visual => "👁️ Automatick - Visual Mode".to_string(),
                Mode::Help => "❓ Automatick - Help".to_string(),
                Mode::Normal => "📋 Automatick".to_string(),
            }
        };

        let style = if error_message.is_some() {
            Style::default().fg(TEXT_WHITE).bg(ACCENT_RED).bold()
        } else {
            Style::default().fg(HEADER_FG).bg(HEADER_BG).bold()
        };

        // Split header into two rows: title and tabs
        let header_chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Title
                Constraint::Length(1), // Tabs
            ])
            .split(area);

        let header = Paragraph::new(title)
            .style(style)
            .block(Block::default().padding(Padding::new(1, 0, 0, 0)));

        f.render_widget(header, header_chunks[0]);

        // Render tabs
        let today_style = if self.current_tab == ViewTab::Today {
            Style::default().fg(TEXT_WHITE).bg(SELECTED_BG).bold()
        } else {
            Style::default().fg(HEADER_FG).bg(HEADER_BG)
        };

        let week_style = if self.current_tab == ViewTab::Week {
            Style::default().fg(TEXT_WHITE).bg(SELECTED_BG).bold()
        } else {
            Style::default().fg(HEADER_FG).bg(HEADER_BG)
        };

        let inbox_style = if self.current_tab == ViewTab::Inbox {
            Style::default().fg(TEXT_WHITE).bg(SELECTED_BG).bold()
        } else {
            Style::default().fg(HEADER_FG).bg(HEADER_BG)
        };

        let tabs_line = Line::from(vec![
            Span::raw("│"),
            Span::styled(" 📅 Today ", today_style),
            Span::raw("│"),
            Span::styled(" 📆 Week ", week_style),
            Span::raw("│"),
            Span::styled(" 📥 Inbox ", inbox_style),
            Span::raw("│"),
        ]);

        let tabs = Paragraph::new(tabs_line).style(Style::default().bg(HEADER_BG));

        f.render_widget(tabs, header_chunks[1]);
    }

    fn render_task_list(
        &mut self,
        f: &mut TuiFrame,
        area: Rect,
        _mode: Mode,
        tasks: &[Task],
        tasks_loaded: bool,
        task_editor_focused: bool,
    ) {
        let tab_name = match self.current_tab {
            ViewTab::Today => "Today",
            ViewTab::Week => "Week",
            ViewTab::Inbox => "Inbox",
        };
        let border_color = if task_editor_focused {
            BORDER_NORMAL
        } else {
            // Task list is active - use brighter border
            BORDER_INSERT
        };

        let block = Block::default()
            .title(format!(" {} Tasks ", tab_name))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .style(Style::default().bg(NORMAL_BG));

        if tasks.is_empty() {
            if tasks_loaded {
                let empty_msg = Paragraph::new("No tasks found")
                    .style(Style::default().fg(TEXT_FG))
                    .block(block)
                    .alignment(ratatui::layout::Alignment::Center);
                f.render_widget(empty_msg, area);
            } else {
                let loading_msg = Paragraph::new("Loading tasks...")
                    .style(Style::default().fg(TEXT_FG))
                    .block(block)
                    .alignment(ratatui::layout::Alignment::Center);
                f.render_widget(loading_msg, area);
            }
            return;
        }

        let selected = self.state.selected();
        let format_date = |dt: &DateTime<Utc>, is_all_day: bool| -> Option<String> {
            if dt.timestamp() == 0 {
                None
            } else {
                let local: DateTime<Local> = dt.with_timezone(&Local);
                if self.current_tab == ViewTab::Today {
                    // For "Today" tab, only show time
                    if is_all_day {
                        None
                    } else {
                        Some(local.format("%I:%M %p").to_string())
                    }
                } else if is_all_day {
                    Some(local.format("%m/%d/%Y").to_string())
                } else {
                    Some(local.format("%m/%d/%Y %I:%M %p").to_string())
                }
            }
        };
        let items: Vec<ListItem> = tasks
            .iter()
            .enumerate()
            .map(|(i, task)| {
                let is_selected = if let Some(range) = self.visual_range {
                    if range.0 <= range.1 {
                        i >= range.0 && i <= range.1
                    } else {
                        i >= range.1 && i <= range.0
                    }
                } else {
                    Some(i) == selected
                };
                let bg_color = if i % 2 == 0 { NORMAL_BG } else { ALT_BG };
                let status_icon = "○";

                let priority_color = match task.priority {
                    TaskPriority::High => PRIORITY_HIGH,
                    TaskPriority::Medium => PRIORITY_MEDIUM,
                    TaskPriority::Low => PRIORITY_LOW,
                    TaskPriority::None => PRIORITY_NONE,
                };

                let text_color = TEXT_FG;
                let row1 = Line::from("");
                let mut row2_spans = vec![];
                if is_selected {
                    row2_spans.push(Span::styled("▶ ", Style::default().fg(TEXT_FG)));
                } else {
                    row2_spans.push(Span::raw("  "));
                }
                row2_spans.push(Span::styled(status_icon, Style::default().fg(text_color)));
                row2_spans.push(Span::raw(" "));
                row2_spans.push(Span::styled("●", Style::default().fg(priority_color)));
                row2_spans.push(Span::raw(" "));
                row2_spans.push(Span::styled(&task.title, Style::default().fg(text_color)));

                let row2 = Line::from(row2_spans);

                // Row 3 with date information
                let mut row3_spans = vec![Span::raw("    ")];

                if let Some(due_str) = format_date(&task.due_date, task.is_all_day) {
                    // Check if task is overdue
                    let is_overdue = {
                        let now_local = chrono::Local::now();
                        let due_date_local = task.due_date.with_timezone(&chrono::Local);

                        if task.is_all_day {
                            // For all-day tasks, compare dates only (not time)
                            // They're only overdue if the due date is before today's date
                            due_date_local.date_naive() < now_local.date_naive()
                        } else {
                            // For timed tasks, compare full datetime
                            due_date_local < now_local
                        }
                    };

                    let date_color = if is_overdue {
                        DATE_OVERDUE
                    } else {
                        DATE_NORMAL
                    };

                    // row3_spans.push(Span::styled(
                    //     "📅 Due: ",
                    //     Style::default().fg(Color::Rgb(150, 150, 150)),
                    // ));
                    row3_spans.push(Span::styled(due_str, Style::default().fg(date_color)));
                    row3_spans.push(Span::raw("  "));
                }

                // if let Some(start_str) = format_date(&task.start_date) {
                //     row3_spans.push(Span::styled(
                //         "🕐 Start: ",
                //         Style::default().fg(Color::Rgb(150, 150, 150)),
                //     ));
                //     row3_spans.push(Span::styled(
                //         start_str,
                //         Style::default().fg(Color::Rgb(150, 200, 255)),
                //     ));
                // }

                let row3 = Line::from(row3_spans);

                ListItem::new(vec![row1, row2, row3]).style(Style::default().bg(bg_color))
            })
            .collect();

        let list = List::new(items).block(block).highlight_style(
            Style::default()
                .bg(SELECTED_BG)
                .add_modifier(Modifier::BOLD),
        );

        f.render_stateful_widget(list, area, &mut self.state);
    }

    fn render_task_details(
        &mut self,
        f: &mut TuiFrame,
        area: Rect,
        tasks: &[Task],
        task_editor_focused: bool,
    ) {
        self.render_task_editor(f, area, tasks, task_editor_focused);
    }

    fn render_task_editor(
        &mut self,
        f: &mut TuiFrame,
        area: Rect,
        _tasks: &[Task],
        task_editor_focused: bool,
    ) {
        // Render background
        let bg_block = Block::default().style(Style::default().bg(NORMAL_BG));
        f.render_widget(bg_block, area);

        let constraints = if task_editor_focused {
            vec![
                Constraint::Length(3), // Title field
                Constraint::Length(8), // Description field
                Constraint::Length(4), // Date field + error message
                Constraint::Length(4), // Time field + error message
            ]
        } else {
            vec![
                Constraint::Length(3), // Title field
                Constraint::Length(8), // Description field
                Constraint::Length(4), // Date field + error message
                Constraint::Length(4), // Time field + error message
            ]
        };

        let chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .margin(1)
            .constraints(constraints)
            .split(area);

        // Title field
        let title_border_color = if task_editor_focused
            && self.task_editor.current_input_field == InputField::Title
            && self.task_editor.is_current_editor_in_insert_mode()
        {
            ACCENT_YELLOW
        } else if task_editor_focused && self.task_editor.current_input_field == InputField::Title {
            BORDER_INSERT
        } else {
            BORDER_NORMAL
        };

        let title_block = Block::default()
            .title("Title")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(title_border_color))
            .style(Style::default().bg(NORMAL_BG));

        let title_inner = title_block.inner(chunks[0]);
        f.render_widget(title_block, chunks[0]);

        let title_theme =
            if task_editor_focused && self.task_editor.current_input_field == InputField::Title {
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
        let title_editor_view =
            EditorView::new(&mut self.task_editor.input_title_editor).theme(title_theme);
        f.render_widget(title_editor_view, title_inner);

        // Description field
        let description_border_color = if task_editor_focused
            && self.task_editor.current_input_field == InputField::Description
            && self.task_editor.is_current_editor_in_insert_mode()
        {
            ACCENT_YELLOW
        } else if task_editor_focused
            && self.task_editor.current_input_field == InputField::Description
        {
            BORDER_INSERT
        } else {
            BORDER_NORMAL
        };

        let description_block = Block::default()
            .title("Description")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(description_border_color))
            .style(Style::default().bg(NORMAL_BG));

        let description_inner = description_block.inner(chunks[1]);
        f.render_widget(description_block, chunks[1]);

        let description_theme = if task_editor_focused
            && self.task_editor.current_input_field == InputField::Description
        {
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
            EditorView::new(&mut self.task_editor.input_description_editor)
                .theme(description_theme);
        f.render_widget(description_editor_view, description_inner);

        // Date field with error message layout
        let date_field_layout = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Date input
                Constraint::Length(1), // Error message
            ])
            .split(chunks[2]);

        let date_has_error =
            self.task_editor.validation_attempted && self.task_editor.date_error.is_some();
        let date_border_color = if date_has_error {
            ACCENT_RED
        } else if task_editor_focused
            && self.task_editor.current_input_field == InputField::Date
            && self.task_editor.is_current_editor_in_insert_mode()
        {
            ACCENT_YELLOW
        } else if task_editor_focused && self.task_editor.current_input_field == InputField::Date {
            BORDER_INSERT
        } else {
            BORDER_NORMAL
        };

        let date_block = Block::default()
            .title("Date (MM/DD/YYYY)")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(date_border_color))
            .style(Style::default().bg(NORMAL_BG));

        let date_inner = date_block.inner(date_field_layout[0]);
        f.render_widget(date_block, date_field_layout[0]);

        let date_theme =
            if task_editor_focused && self.task_editor.current_input_field == InputField::Date {
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
        let date_editor_view =
            EditorView::new(&mut self.task_editor.input_date_editor).theme(date_theme);
        f.render_widget(date_editor_view, date_inner);

        // Render date error message if present
        if let Some(error) = &self.task_editor.date_error {
            let error_paragraph =
                Paragraph::new(error.as_str()).style(Style::default().fg(ACCENT_RED).bg(NORMAL_BG));
            f.render_widget(error_paragraph, date_field_layout[1]);
        }

        // Time field with error message layout
        let time_field_layout = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Time input
                Constraint::Length(1), // Error message
            ])
            .split(chunks[3]);

        let time_has_error =
            self.task_editor.validation_attempted && self.task_editor.time_error.is_some();
        let time_border_color = if time_has_error {
            ACCENT_RED
        } else if task_editor_focused
            && self.task_editor.current_input_field == InputField::Time
            && self.task_editor.is_current_editor_in_insert_mode()
        {
            ACCENT_YELLOW
        } else if task_editor_focused && self.task_editor.current_input_field == InputField::Time {
            BORDER_INSERT
        } else {
            BORDER_NORMAL
        };

        let time_block = Block::default()
            .title("Time (HH:MM AM/PM)")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(time_border_color))
            .style(Style::default().bg(NORMAL_BG));

        let time_inner = time_block.inner(time_field_layout[0]);
        f.render_widget(time_block, time_field_layout[0]);

        let time_theme =
            if task_editor_focused && self.task_editor.current_input_field == InputField::Time {
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
        let time_editor_view =
            EditorView::new(&mut self.task_editor.input_time_editor).theme(time_theme);
        f.render_widget(time_editor_view, time_inner);

        // Render time error message if present
        if let Some(error) = &self.task_editor.time_error {
            let error_paragraph =
                Paragraph::new(error.as_str()).style(Style::default().fg(ACCENT_RED).bg(NORMAL_BG));
            f.render_widget(error_paragraph, time_field_layout[1]);
        }

        // Render the main border with title
        let border_color = if task_editor_focused {
            BORDER_INSERT
        } else {
            BORDER_NORMAL
        };

        let main_block = Block::default()
            .title(" Task Details ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        f.render_widget(main_block, area);
    }

    fn render_footer(&self, f: &mut TuiFrame, area: Rect, mode: Mode) {
        let footer_text = match mode {
            Mode::Processing => "Processing request...",
            Mode::Help => "Press ? or Esc to close help",
            _ => "?: Help | q: Quit",
        };

        let footer = Paragraph::new(footer_text)
            .style(Style::default().fg(TEXT_FG).bg(NORMAL_BG))
            .alignment(ratatui::layout::Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(BORDER_NORMAL)),
            );

        f.render_widget(footer, area);
    }

    fn render_help_overlay(&self, f: &mut TuiFrame, area: Rect) {
        let popup_area = centered_rect(70, 60, area);

        // Clear the area
        f.render_widget(Clear, popup_area);

        let block = Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(ACCENT_YELLOW))
            .style(Style::default().bg(NORMAL_BG))
            .padding(Padding::uniform(1));

        let help_text = vec![
            Line::from(Span::styled(
                "Navigation",
                Style::default().fg(ACCENT_YELLOW).bold(),
            )),
            Line::from(""),
            Line::from("  ↑ / k          Move selection up"),
            Line::from("  ↓ / j          Move selection down"),
            Line::from("  g / Home       Jump to first task"),
            Line::from("  G / End        Jump to last task"),
            Line::from("  h / l / ← / →  Switch between tabs"),
            Line::from("  Esc            Clear selection"),
            Line::from(""),
            Line::from(Span::styled(
                "Task Actions",
                Style::default().fg(Color::Yellow).bold(),
            )),
            Line::from(""),
            Line::from("  e              Complete task"),
            Line::from("  Enter          Edit task"),
            Line::from("  n              Create new task (with date/time)"),
            Line::from("  d              Delete selected task"),
            Line::from("  r              Refresh task list"),
            Line::from(""),
            Line::from(Span::styled(
                "Task Creation",
                Style::default().fg(Color::Yellow).bold(),
            )),
            Line::from(""),
            Line::from("  Tab            Next field"),
            Line::from("  Shift+Tab      Previous field"),
            Line::from("  Enter          Create task"),
            Line::from("  Esc            Cancel"),
            Line::from(""),
            Line::from("  Note: Date defaults to today when in Today view"),
            Line::from(""),
            Line::from(Span::styled(
                "General",
                Style::default().fg(Color::Yellow).bold(),
            )),
            Line::from(""),
            Line::from("  ?              Toggle this help screen"),
            Line::from("  q              Quit application"),
            Line::from(""),
            Line::from(Span::styled(
                "Legend",
                Style::default().fg(Color::Yellow).bold(),
            )),
            Line::from(""),
            Line::from("  ○              Task"),
            Line::from("  🔴             High priority"),
            Line::from("  🟡             Medium priority"),
            Line::from("  🔵             Low priority"),
        ];

        let paragraph = Paragraph::new(help_text)
            .block(block)
            .style(Style::default().fg(TEXT_FG));

        f.render_widget(paragraph, popup_area);
    }
}

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    // Cut the given rectangle into three vertical pieces
    let popup_layout = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    // Then cut the middle vertical piece into three width-wise pieces
    Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1] // Return the middle chunk
}
