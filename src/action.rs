use serde::{Deserialize, Serialize};
use strum::Display;

/// Actions that can be performed in the TUI
#[derive(Debug, Clone, Serialize, Display, Deserialize)]
pub enum Action {
    /// Application tick for periodic updates
    Tick,
    /// Render the UI
    Render,
    /// Resize terminal
    Resize(u16, u16),
    /// Quit the application
    Quit,
    /// Refresh task list from server
    RefreshTasks,
    /// Display error message
    Error(String),
    /// Toggle help screen
    ToggleHelp,

    // Navigation actions
    /// Move selection up
    SelectPrevious,
    /// Move selection down
    SelectNext,
    /// Move to first item
    SelectFirst,
    /// Move to last item
    SelectLast,
    /// Clear selection
    SelectNone,

    // Tab navigation
    /// Switch to previous tab
    PreviousTab,
    /// Switch to next tab
    NextTab,

    // Task actions
    /// Mark task as complete
    CompleteTask,
    /// Start delete task confirmation
    StartDeleteTask,
    /// Delete selected task
    DeleteTask,
    /// Start creating a new task
    StartCreateTask,
    /// Start editing selected task
    StartEditTask,
    /// Postpone a task
    StartPostponeTask,
    /// Cancel current input operation
    CancelInput,
    /// Confirm current input operation
    ConfirmInput,

    // Mode changes
    /// Enter normal mode
    EnterNormal,
    /// Enter insert mode (for creating/editing)
    EnterInsert,
    /// Enter visual mode (for multi-select)
    EnterVisual,
    /// Enter processing mode (waiting for API)
    EnterProcessing,
    /// Exit processing mode
    ExitProcessing,

    // Task updates
    /// Task operation completed successfully
    TaskOperationComplete,
    /// Tasks fetched from API - triggers UI update
    TasksFetched,
}
