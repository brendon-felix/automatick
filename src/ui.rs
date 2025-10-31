use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Padding, Paragraph, Wrap},
};
use ticks::tasks::{Task, TaskPriority};

use crate::app::Mode;
use crate::tui::Frame as TuiFrame;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewTab {
    Today,
    Week,
    Inbox,
}

// Color scheme
const HEADER_BG: Color = Color::Rgb(36, 36, 36);
const HEADER_FG: Color = Color::Rgb(200, 200, 200);
const NORMAL_BG: Color = Color::Rgb(19, 19, 19);
const ALT_BG: Color = Color::Rgb(25, 25, 25);
const SELECTED_BG: Color = Color::Rgb(36, 36, 36);
const TEXT_FG: Color = Color::Rgb(200, 200, 200);
const BORDER_NORMAL: Color = Color::Rgb(116, 116, 116);
const BORDER_PROCESSING: Color = Color::Rgb(116, 116, 116);
const BORDER_INSERT: Color = Color::Rgb(165, 165, 165);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputField {
    Title,
    Date,
    Time,
}

pub struct TaskListUI {
    state: ListState,
    input_value: String,
    input_date: String,
    input_time: String,
    input_title: String,
    current_input_field: InputField,
    pub current_tab: ViewTab,
}

impl TaskListUI {
    pub fn new() -> Self {
        Self {
            state: ListState::default(),
            input_value: String::new(),
            input_date: String::new(),
            input_time: String::new(),
            input_title: String::new(),
            current_input_field: InputField::Title,
            current_tab: ViewTab::Today,
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

    pub fn select_previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    i
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn select_next(&mut self) {
        self.state.select_next();
    }

    pub fn select_first(&mut self) {
        self.state.select_first();
    }

    pub fn select_last(&mut self) {
        self.state.select_last();
    }

    pub fn select_none(&mut self) {
        self.state.select(None);
    }

    pub fn start_input_mode(&mut self, title: &str, default_date: Option<String>) {
        self.input_title = title.to_string();
        self.input_value.clear();
        self.input_time.clear();
        self.current_input_field = InputField::Title;

        // Set default date if provided, otherwise clear
        if let Some(date) = default_date {
            self.input_date = date;
        } else {
            self.input_date.clear();
        }
    }

    pub fn update_input(&mut self, value: String) {
        match self.current_input_field {
            InputField::Title => self.input_value = value,
            InputField::Date => self.input_date = value,
            InputField::Time => self.input_time = value,
        }
    }

    pub fn get_input_value(&self) -> String {
        self.input_value.clone()
    }

    pub fn get_input_date(&self) -> String {
        self.input_date.clone()
    }

    pub fn get_input_time(&self) -> String {
        self.input_time.clone()
    }

    pub fn get_current_input(&self) -> String {
        match self.current_input_field {
            InputField::Title => self.input_value.clone(),
            InputField::Date => self.input_date.clone(),
            InputField::Time => self.input_time.clone(),
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

    pub fn clear_input(&mut self) {
        self.input_value.clear();
        self.input_date.clear();
        self.input_time.clear();
        self.current_input_field = InputField::Title;
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

    pub fn draw(
        &mut self,
        f: &mut TuiFrame,
        area: Rect,
        mode: Mode,
        tasks: &[Task],
        error_message: &Option<String>,
        tasks_loaded: bool,
    ) -> Result<()> {
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
        self.render_task_list(f, content_chunks[0], mode, tasks, tasks_loaded);
        self.render_task_details(f, content_chunks[1], tasks);
        self.render_footer(f, main_chunks[2], mode);

        // Render overlays
        if mode == Mode::Insert {
            self.render_input_overlay(f, area);
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
                Mode::Insert => "✏️  Automatick - Insert Mode".to_string(),
                Mode::Help => "❓ Automatick - Help".to_string(),
                Mode::Normal => "📋 Automatick".to_string(),
            }
        };

        let style = if error_message.is_some() {
            Style::default().fg(Color::White).bg(Color::Red).bold()
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
            Style::default().fg(Color::White).bg(SELECTED_BG).bold()
        } else {
            Style::default().fg(HEADER_FG).bg(HEADER_BG)
        };

        let week_style = if self.current_tab == ViewTab::Week {
            Style::default().fg(Color::White).bg(SELECTED_BG).bold()
        } else {
            Style::default().fg(HEADER_FG).bg(HEADER_BG)
        };

        let inbox_style = if self.current_tab == ViewTab::Inbox {
            Style::default().fg(Color::White).bg(SELECTED_BG).bold()
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
        mode: Mode,
        tasks: &[Task],
        tasks_loaded: bool,
    ) {
        let tab_name = match self.current_tab {
            ViewTab::Today => "Today",
            ViewTab::Week => "Week",
            ViewTab::Inbox => "Inbox",
        };
        let border_color = match mode {
            Mode::Processing => BORDER_PROCESSING,
            Mode::Insert => BORDER_INSERT,
            _ => BORDER_NORMAL,
        };

        let block = Block::default()
            .title(format!(" {} Tasks ", tab_name))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .style(Style::default().bg(NORMAL_BG));

        if tasks.is_empty() {
            if tasks_loaded {
                let empty_msg = Paragraph::new(
                    "No tasks found. Press 'n' to create a new task, 'r' to refresh.",
                )
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

        let selected_index = self.state.selected();
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
                let is_selected = selected_index == Some(i);
                let bg_color = if i % 2 == 0 { NORMAL_BG } else { ALT_BG };
                let status_icon = "○";

                let priority_color = match task.priority {
                    TaskPriority::High => Color::Red,
                    TaskPriority::Medium => Color::Yellow,
                    TaskPriority::Low => Color::Blue,
                    TaskPriority::None => Color::Gray,
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
                        Color::Rgb(150, 80, 80) // Subtle desaturated red
                    } else {
                        Color::Rgb(100, 100, 100)
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

    fn render_task_details(&self, f: &mut TuiFrame, area: Rect, tasks: &[Task]) {
        let block = Block::default()
            .title(" Task Details ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_NORMAL))
            .style(Style::default().bg(NORMAL_BG))
            .padding(Padding::uniform(1));

        let content = if let Some(i) = self.state.selected() {
            if let Some(task) = tasks.get(i) {
                let mut lines = vec![];

                // Title
                lines.push(Line::from(vec![Span::styled(
                    "Title:",
                    Style::default().fg(Color::Yellow).bold(),
                )]));
                lines.push(Line::from(vec![Span::styled(
                    &task.title,
                    Style::default().fg(TEXT_FG),
                )]));
                lines.push(Line::from(""));

                // Status
                let status_text = "Todo";
                lines.push(Line::from(vec![Span::styled(
                    "Status:",
                    Style::default().fg(Color::Yellow).bold(),
                )]));
                lines.push(Line::from(vec![Span::styled(
                    status_text,
                    Style::default().fg(TEXT_FG),
                )]));
                lines.push(Line::from(""));

                // Priority
                let priority_text = match task.priority {
                    TaskPriority::High => "High 🔴",
                    TaskPriority::Medium => "Medium 🟡",
                    TaskPriority::Low => "Low 🔵",
                    TaskPriority::None => "None",
                };
                lines.push(Line::from(vec![Span::styled(
                    "Priority:",
                    Style::default().fg(Color::Yellow).bold(),
                )]));
                lines.push(Line::from(vec![Span::styled(
                    priority_text,
                    Style::default().fg(TEXT_FG),
                )]));
                lines.push(Line::from(""));

                // Due Date
                if task.due_date.timestamp() != 0 {
                    let local: DateTime<Local> = task.due_date.with_timezone(&Local);
                    let due_str = if task.is_all_day {
                        local.format("%m/%d/%Y").to_string()
                    } else {
                        local.format("%m/%d/%Y %I:%M %p").to_string()
                    };

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
                        Color::Rgb(150, 80, 80) // Subtle desaturated red
                    } else {
                        TEXT_FG
                    };

                    lines.push(Line::from(vec![Span::styled(
                        "Due Date:",
                        Style::default().fg(Color::Yellow).bold(),
                    )]));
                    lines.push(Line::from(vec![Span::styled(
                        due_str,
                        Style::default().fg(date_color),
                    )]));
                    lines.push(Line::from(""));
                }

                // Start Date (only show if different from due date)
                if task.start_date.timestamp() != 0 && task.start_date != task.due_date {
                    let local: DateTime<Local> = task.start_date.with_timezone(&Local);
                    let start_str = if task.is_all_day {
                        local.format("%m/%d/%Y").to_string()
                    } else {
                        local.format("%m/%d/%Y %I:%M %p").to_string()
                    };
                    lines.push(Line::from(vec![Span::styled(
                        "Start Date:",
                        Style::default().fg(Color::Yellow).bold(),
                    )]));
                    lines.push(Line::from(vec![Span::styled(
                        start_str,
                        Style::default().fg(TEXT_FG),
                    )]));
                    lines.push(Line::from(""));
                }

                // Description/Content if available
                if !task.content.is_empty() {
                    lines.push(Line::from(vec![Span::styled(
                        "Description:",
                        Style::default().fg(Color::Yellow).bold(),
                    )]));
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        &task.content,
                        Style::default().fg(TEXT_FG),
                    )));
                }

                Paragraph::new(lines)
                    .block(block)
                    .wrap(Wrap { trim: false })
                    .style(Style::default().fg(TEXT_FG))
            } else {
                Paragraph::new("No task selected")
                    .block(block)
                    .style(Style::default().fg(TEXT_FG))
            }
        } else {
            Paragraph::new("No task selected")
                .block(block)
                .style(Style::default().fg(TEXT_FG))
        };

        f.render_widget(content, area);
    }

    fn render_footer(&self, f: &mut TuiFrame, area: Rect, mode: Mode) {
        let footer_text = match mode {
            Mode::Normal => {
                "↑↓/jk: Navigate | h/l/←→: Switch Tab | Esc: Deselect | Space/Enter: Complete | n: New | d: Delete | r: Refresh | ?: Help | q: Quit"
            }
            Mode::Insert => "Type task title | Enter: Confirm | Esc: Cancel",
            Mode::Processing => "Processing request...",
            Mode::Help => "Press ? or Esc to close help",
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

    fn render_input_overlay(&self, f: &mut TuiFrame, area: Rect) {
        let popup_area = centered_rect(60, 40, area);

        // Clear the area
        f.render_widget(Clear, popup_area);

        let block = Block::default()
            .title(format!(" {} ", self.input_title))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BORDER_INSERT))
            .style(Style::default().bg(NORMAL_BG));

        let inner = block.inner(popup_area);
        f.render_widget(block, popup_area);

        // Create layout for multiple fields
        let chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(1),
            ])
            .split(inner);

        // Title field
        let title_text = if self.input_value.is_empty() {
            "Enter task title..."
        } else {
            &self.input_value
        };

        let title_style = if self.current_input_field == InputField::Title {
            if self.input_value.is_empty() {
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::UNDERLINED)
            } else {
                Style::default()
                    .fg(TEXT_FG)
                    .add_modifier(Modifier::UNDERLINED)
            }
        } else {
            if self.input_value.is_empty() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(TEXT_FG)
            }
        };

        let title_label = Paragraph::new(vec![
            Line::from(Span::styled("Title:", Style::default().fg(Color::Cyan))),
            Line::from(Span::styled(title_text, title_style)),
        ]);
        f.render_widget(title_label, chunks[0]);

        // Date field
        let date_text = if self.input_date.is_empty() {
            "e.g., 10/30, 10/30/25, or 2025-10-30 (optional)"
        } else {
            &self.input_date
        };

        let date_style = if self.current_input_field == InputField::Date {
            if self.input_date.is_empty() {
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::UNDERLINED)
            } else {
                Style::default()
                    .fg(TEXT_FG)
                    .add_modifier(Modifier::UNDERLINED)
            }
        } else {
            if self.input_date.is_empty() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(TEXT_FG)
            }
        };

        let date_label = Paragraph::new(vec![
            Line::from(Span::styled("Date:", Style::default().fg(Color::Cyan))),
            Line::from(Span::styled(date_text, date_style)),
        ]);
        f.render_widget(date_label, chunks[1]);

        // Time field
        let time_text = if self.input_time.is_empty() {
            "e.g., 5pm, 5:30 AM (optional)"
        } else {
            &self.input_time
        };

        let time_style = if self.current_input_field == InputField::Time {
            if self.input_time.is_empty() {
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::UNDERLINED)
            } else {
                Style::default()
                    .fg(TEXT_FG)
                    .add_modifier(Modifier::UNDERLINED)
            }
        } else {
            if self.input_time.is_empty() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(TEXT_FG)
            }
        };

        let time_label = Paragraph::new(vec![
            Line::from(Span::styled("Time:", Style::default().fg(Color::Cyan))),
            Line::from(Span::styled(time_text, time_style)),
        ]);
        f.render_widget(time_label, chunks[2]);

        // Help text
        let help_text = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "Tab/Shift+Tab: Switch fields",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "Enter: Create task",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "Esc: Cancel",
                Style::default().fg(Color::DarkGray),
            )),
        ]);
        f.render_widget(help_text, chunks[3]);

        // Render cursor
        let (cursor_y, cursor_text_len) = match self.current_input_field {
            InputField::Title => (chunks[0].y + 1, self.input_value.len()),
            InputField::Date => (chunks[1].y + 1, self.input_date.len()),
            InputField::Time => (chunks[2].y + 1, self.input_time.len()),
        };

        let cursor_x = chunks[0].x + cursor_text_len as u16;
        if cursor_x < chunks[0].x + chunks[0].width {
            f.set_cursor_position((cursor_x, cursor_y));
        }
    }

    fn render_help_overlay(&self, f: &mut TuiFrame, area: Rect) {
        let popup_area = centered_rect(70, 60, area);

        // Clear the area
        f.render_widget(Clear, popup_area);

        let block = Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .style(Style::default().bg(NORMAL_BG))
            .padding(Padding::uniform(1));

        let help_text = vec![
            Line::from(Span::styled(
                "Navigation",
                Style::default().fg(Color::Yellow).bold(),
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
            Line::from("  Space / Enter  Complete task"),
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

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
