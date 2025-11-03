use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use edtui::{EditorTheme, EditorView};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Padding, Paragraph},
};
use ticks::tasks::{Task, TaskPriority};

use super::colors::*;
use super::tui::Frame as TuiFrame;
use super::{centered_rect, InputField, TaskEditor, TaskList, ViewTab};
use crate::app::Mode;

pub struct AppUI {
    pub task_list: TaskList,
    pub task_editor: TaskEditor,
}

impl AppUI {
    pub fn new() -> Self {
        Self {
            task_list: TaskList::new(),
            task_editor: TaskEditor::new(),
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
                Constraint::Length(1), // Header
                Constraint::Min(10),   // Content area (task list + details)
                Constraint::Length(3), // Footer
            ])
            .split(area);

        // Content area split horizontally: Task list on left, Details on right
        // Active pane gets more space (70%), inactive pane gets less (30%)
        let content_chunks = Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints(if task_editor_focused {
                [
                    Constraint::Percentage(40), // Task list (inactive)
                    Constraint::Percentage(60), // Task details (active)
                ]
            } else {
                [
                    Constraint::Percentage(60), // Task list (active)
                    Constraint::Percentage(40), // Task details (inactive)
                ]
            })
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
        if let Some(modal) = &mut self.task_list.current_modal {
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
        let (title, style) = if let Some(err) = error_message {
            (
                Line::from(vec![
                    Span::styled("❌ ", Style::default().fg(ACCENT_RED).bold()),
                    Span::styled("Error: ", Style::default().fg(TEXT_WHITE).bold()),
                    Span::styled(err, Style::default().fg(TEXT_WHITE)),
                ]),
                Style::default().bg(ACCENT_RED),
            )
        } else {
            let (icon, text, accent_color) = match mode {
                Mode::Processing => ("⏳", " Processing...", ACCENT_YELLOW),
                Mode::Insert => ("✏️", " Insert Mode", ACCENT_GREEN),
                Mode::Visual => ("👁️", " Visual Mode", Color::Cyan),
                Mode::Help => ("❓", " Help", Color::Cyan),
                Mode::Normal => ("📋", " Automatick", HEADER_FG),
            };

            (
                Line::from(vec![
                    Span::styled(icon, Style::default().fg(accent_color).bold()),
                    Span::styled(text, Style::default().fg(HEADER_FG).bold()),
                ]),
                Style::default().bg(NORMAL_BG),
            )
        };

        let header = Paragraph::new(title)
            .style(style)
            .alignment(ratatui::layout::Alignment::Center)
            .block(Block::default());

        f.render_widget(header, area);
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
        let border_color = if task_editor_focused {
            BORDER_NORMAL
        } else {
            // Task list is active - use brighter border
            BORDER_INSERT
        };

        // Create overlapping tab effect with dynamic sizing
        let tabs_title = match self.task_list.current_tab {
            ViewTab::Today => Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    "  📅 Today  ",
                    Style::default().fg(TEXT_WHITE).bg(SELECTED_BG).bold(),
                ),
                Span::styled(" Week", Style::default().fg(TEXT_FG).dim()),
                Span::raw(" "),
                Span::styled("Inbox", Style::default().fg(TEXT_FG).dim()),
                Span::raw(" "),
            ]),
            ViewTab::Week => Line::from(vec![
                Span::raw(" "),
                Span::styled("Today", Style::default().fg(TEXT_FG).dim()),
                Span::raw(" "),
                Span::styled(
                    "  📆 Week  ",
                    Style::default().fg(TEXT_WHITE).bg(SELECTED_BG).bold(),
                ),
                Span::styled(" Inbox", Style::default().fg(TEXT_FG).dim()),
                Span::raw(" "),
            ]),
            ViewTab::Inbox => Line::from(vec![
                Span::raw(" "),
                Span::styled("Today", Style::default().fg(TEXT_FG).dim()),
                Span::raw(" "),
                Span::styled("Week ", Style::default().fg(TEXT_FG).dim()),
                Span::styled(
                    "  📥 Inbox  ",
                    Style::default().fg(TEXT_WHITE).bg(SELECTED_BG).bold(),
                ),
                Span::raw(" "),
            ]),
        };

        let block = Block::default()
            .title(tabs_title)
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

        let selected = self.task_list.selected_index();
        let format_date = |dt: &DateTime<Utc>, is_all_day: bool| -> Option<String> {
            if dt.timestamp() == 0 {
                None
            } else {
                let local: DateTime<Local> = dt.with_timezone(&Local);
                if self.task_list.current_tab == ViewTab::Today {
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
                let is_selected = if let Some(range) = self.task_list.visual_range {
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

                    row3_spans.push(Span::styled(due_str, Style::default().fg(date_color)));
                    row3_spans.push(Span::raw("  "));
                }

                let row3 = Line::from(row3_spans);

                ListItem::new(vec![row1, row2, row3]).style(Style::default().bg(bg_color))
            })
            .collect();

        let list = List::new(items).block(block).highlight_style(
            Style::default()
                .bg(SELECTED_BG)
                .add_modifier(Modifier::BOLD),
        );

        f.render_stateful_widget(list, area, self.task_list.get_list_state_mut());
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
                Constraint::Length(4), // Date field + error message
                Constraint::Length(4), // Time field + error message
                Constraint::Min(3),    // Description field (remaining space, min 3 lines)
            ]
        } else {
            vec![
                Constraint::Length(3), // Title field
                Constraint::Length(4), // Date field + error message
                Constraint::Length(4), // Time field + error message
                Constraint::Min(3),    // Description field (remaining space, min 3 lines)
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

        // Date field with error message layout
        let date_field_layout = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Date input
                Constraint::Length(1), // Error message
            ])
            .split(chunks[1]);

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
            .split(chunks[2]);

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

        let description_inner = description_block.inner(chunks[3]);
        f.render_widget(description_block, chunks[3]);

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
