pub mod app_ui;
pub mod colors;
pub mod modal;
pub mod task_editor;
pub mod task_list;
pub mod tui;

pub use app_ui::AppUI;
pub use modal::{ConfirmationModal, ConfirmationType, PostponeModal, TaskModal};
pub use task_editor::{InputField, TaskEditor};
pub use task_list::{TaskList, ViewTab};
pub use tui::{Event, Tui};

use ratatui::layout::{Constraint, Layout, Rect};

// Utility function for centered rectangles
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
