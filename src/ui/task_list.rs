use super::modal::Modal;
use super::{ConfirmationModal, ConfirmationType, TaskModal};
use anyhow::Result;
use crossterm::event::KeyEvent;
use ratatui::widgets::ListState;
use ticks::tasks::Task;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewTab {
    Today,
    Week,
    Inbox,
}

pub struct TaskList {
    state: ListState,
    pub current_tab: ViewTab,
    pub visual_range: Option<(usize, usize)>,
    pub current_modal: Option<Box<dyn Modal>>,
}

#[allow(dead_code)]
impl TaskList {
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
        if let Some(task_modal) = (&mut modal as &mut dyn std::any::Any).downcast_mut::<TaskModal>()
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

    pub fn get_confirmation_type(&self) -> Option<&ConfirmationType> {
        if let Some(modal) = &self.current_modal {
            if let Some(confirmation_modal) =
                modal.as_ref().as_any().downcast_ref::<ConfirmationModal>()
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

    pub fn get_list_state(&self) -> &ListState {
        &self.state
    }

    pub fn get_list_state_mut(&mut self) -> &mut ListState {
        &mut self.state
    }

    pub fn get_visual_range(&self) -> Option<(usize, usize)> {
        self.visual_range
    }
}
