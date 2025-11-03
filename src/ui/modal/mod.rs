use anyhow::Result;
use crossterm::event::KeyEvent;
use ratatui::layout::Rect;

use super::tui::Frame as TuiFrame;

/// Trait for modal dialogs that can be displayed as overlays
#[allow(dead_code)]
pub trait Modal {
    /// Get the title of the modal
    fn title(&self) -> &str;

    /// Allow downcasting to concrete types
    fn as_any(&self) -> &dyn std::any::Any;

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

    /// Validate inputs and return true if valid (default implementation always returns true)
    fn validate(&mut self) -> bool {
        true
    }

    /// Check if there are any validation errors (default implementation always returns false)
    fn has_validation_errors(&self) -> bool {
        false
    }
}

pub mod confirmation_modal;
pub mod postpone_modal;
pub mod task_modal;

pub use confirmation_modal::{ConfirmationModal, ConfirmationType};
pub use postpone_modal::PostponeModal;
pub use task_modal::TaskModal;
