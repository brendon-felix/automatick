use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use crossterm::event::KeyEvent;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Padding, Paragraph, Wrap},
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

pub struct TaskListUI {
    state: ListState,
    pub current_tab: ViewTab,
    pub visual_range: Option<(usize, usize)>,
    pub current_modal: Option<Box<dyn Modal>>,
}

impl TaskListUI {
    pub fn new() -> Self {
        Self {
            state: ListState::default(),
            current_tab: ViewTab::Today,
            visual_range: None,
            current_modal: None,
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

    pub fn select_previous(&mut self) {
        self.state.select_previous();
        if self.visual_range.is_some() {
            self.update_visual_range();
        }
    }

    pub fn select_next(&mut self) {
        self.state.select_next();
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

    pub fn select_first(&mut self) {
        self.state.select_first();
        if self.visual_range.is_some() {
            self.update_visual_range();
        }
    }

    pub fn select_last(&mut self) {
        self.state.select_last();
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
        self.render_task_list(f, content_chunks[0], mode, tasks, tasks_loaded);
        self.render_task_details(f, content_chunks[1], tasks);
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
                    Style::default().fg(ACCENT_YELLOW).bold(),
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
                    Style::default().fg(ACCENT_YELLOW).bold(),
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
                    Style::default().fg(ACCENT_YELLOW).bold(),
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
                        DATE_OVERDUE
                    } else {
                        DATE_NORMAL
                    };

                    lines.push(Line::from(vec![Span::styled(
                        "Due Date:",
                        Style::default().fg(ACCENT_YELLOW).bold(),
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
                        Style::default().fg(ACCENT_YELLOW).bold(),
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
                        Style::default().fg(ACCENT_YELLOW).bold(),
                    )]));
                    lines.push(Line::from(""));
                    // Split content by newlines and create separate lines for proper rendering
                    for content_line in task.content.lines() {
                        lines.push(Line::from(Span::styled(
                            content_line,
                            Style::default().fg(TEXT_FG),
                        )));
                    }
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
