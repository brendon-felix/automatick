use anyhow::Result;
use crossterm::event::KeyEvent;
use edtui::{EditorEventHandler, EditorMode, EditorState, Index2, Lines};

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
