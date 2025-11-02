use anyhow::Result;
use chrono::{DateTime, Local};
use crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use std::sync::{Arc, Mutex};
use ticks::{
    projects::ProjectID,
    tasks::{Task, TaskID},
    TickTick,
};
use tokio::sync::mpsc::{self, UnboundedSender};

use crate::{
    action::Action,
    modal::{ConfirmationModal, PostponeModal, TaskModal},
    tasks::{self, fetch_all_tasks},
    tui::{self, Event, Tui},
    ui::{TaskListUI, ViewTab},
    utils::{self, parse_date_us_format, parse_time_us_format},
};

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Normal,
    Insert,
    Visual,
    Processing,
    Help,
}

pub struct App {
    pub should_quit: bool,
    pub mode: Mode,
    pub ui: TaskListUI,
    pub client: Arc<TickTick>,
    pub error_message: Option<String>,
    pub error_ticks: u8,
    pub today_cache: Vec<Task>,
    pub week_cache: Vec<Task>,
    pub inbox_cache: Vec<Task>,
    pub tasks_loaded: bool,
    pub pending_tasks: Arc<Mutex<Option<(Vec<Task>, Vec<Task>, Vec<Task>)>>>,
    pub current_tab: ViewTab,
    pub editing_task: Option<(ProjectID, TaskID)>,
    pub task_editor_focused: bool,
}

impl App {
    pub fn new(client: Arc<TickTick>) -> Result<Self> {
        let ui = TaskListUI::new();
        Ok(Self {
            should_quit: false,
            mode: Mode::Normal,
            ui,
            client,
            error_message: None,
            error_ticks: 0,
            today_cache: Vec::new(),
            week_cache: Vec::new(),
            inbox_cache: Vec::new(),
            tasks_loaded: false,
            pending_tasks: Arc::new(Mutex::new(None)),
            current_tab: ViewTab::Today,
            editing_task: None,
            task_editor_focused: false,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let (action_tx, mut action_rx) = mpsc::unbounded_channel();

        let mut tui = tui::Tui::new()?;
        tui.enter()?;

        // Initial load of tasks
        action_tx.send(Action::RefreshTasks)?;

        loop {
            if let Some(e) = tui.next().await {
                match e {
                    Event::Quit => action_tx.send(Action::Quit)?,
                    Event::Tick => action_tx.send(Action::Tick)?,
                    Event::Render => action_tx.send(Action::Render)?,
                    Event::Resize(x, y) => action_tx.send(Action::Resize(x, y))?,
                    Event::Key(key) => {
                        self.handle_key_event(key, &action_tx)?;
                    }
                    _ => {}
                }
            }

            while let Ok(action) = action_rx.try_recv() {
                match action {
                    Action::Tick => self.next_tick(),
                    Action::Render => self.render(&mut tui)?,
                    Action::Resize(w, h) => tui.terminal.resize(Rect::new(0, 0, w, h))?,
                    Action::Quit => self.should_quit = true,
                    Action::RefreshTasks => self.refresh_tasks(action_tx.clone()),
                    Action::Error(msg) => self.error(msg),
                    Action::ToggleHelp => self.toggle_help(),

                    Action::SelectPrevious => {
                        self.save_task_before_changing_selection(action_tx.clone());
                        let current_tab = self.ui.current_tab;
                        let tasks = self.get_view_tasks(current_tab);
                        self.ui.select_previous(tasks.len());
                        self.sync_task_editor_with_selection();
                    }
                    Action::SelectNext => {
                        self.save_task_before_changing_selection(action_tx.clone());
                        let current_tab = self.ui.current_tab;
                        let tasks = self.get_view_tasks(current_tab);
                        self.ui.select_next(tasks.len());
                        self.sync_task_editor_with_selection();
                    }
                    Action::SelectPreviousCycling => {
                        self.save_task_before_changing_selection(action_tx.clone());
                        let current_tab = self.ui.current_tab;
                        let tasks = self.get_view_tasks(current_tab);
                        self.ui.select_previous_cycling(tasks.len());
                        self.sync_task_editor_with_selection();
                    }
                    Action::SelectNextCycling => {
                        self.save_task_before_changing_selection(action_tx.clone());
                        let current_tab = self.ui.current_tab;
                        let tasks = self.get_view_tasks(current_tab);
                        self.ui.select_next_cycling(tasks.len());
                        self.sync_task_editor_with_selection();
                    }
                    Action::SelectFirst => {
                        self.save_task_before_changing_selection(action_tx.clone());
                        let current_tab = self.ui.current_tab;
                        let tasks = self.get_view_tasks(current_tab);
                        self.ui.select_first(tasks.len());
                        self.sync_task_editor_with_selection();
                    }
                    Action::SelectLast => {
                        self.save_task_before_changing_selection(action_tx.clone());
                        let current_tab = self.ui.current_tab;
                        let tasks = self.get_view_tasks(current_tab);
                        self.ui.select_last(tasks.len());
                        self.sync_task_editor_with_selection();
                    }
                    Action::SelectNone => {
                        self.save_task_before_changing_selection(action_tx.clone());
                        self.ui.select_none();
                        self.sync_task_editor_with_selection();
                    }

                    Action::PreviousTab => {
                        self.save_task_before_changing_selection(action_tx.clone());
                        self.previous_tab();
                    }
                    Action::NextTab => {
                        self.save_task_before_changing_selection(action_tx.clone());
                        self.next_tab();
                    }

                    Action::CompleteTask => self.complete_task(action_tx.clone()),
                    Action::StartCompleteTask => self.start_complete_task(),
                    Action::StartDeleteTask => self.start_delete_task(),
                    Action::DeleteTask => self.delete_task(action_tx.clone()),
                    Action::StartPostponeTask => self.start_postpone_task(),
                    Action::StartCreateTask => self.start_create_task(),
                    Action::StartEditTask => self.start_edit_task(),
                    Action::CancelInput => self.cancel_input(),
                    Action::ConfirmInput => self.confirm_input(action_tx.clone()),

                    Action::EnterNormal => {
                        self.mode = Mode::Normal;
                        self.ui.exit_visual_mode();
                    }
                    Action::EnterInsert => {
                        self.mode = Mode::Insert;
                        self.ui.exit_visual_mode();
                    }
                    Action::EnterVisual => {
                        self.mode = Mode::Visual;
                        self.ui.enter_visual_mode();
                    }
                    Action::EnterProcessing => self.mode = Mode::Processing,
                    Action::ExitProcessing => {
                        self.mode = Mode::Normal;
                        self.ui.exit_visual_mode();
                    }

                    Action::EnterTaskEditor => self.enter_task_editor(),
                    Action::ExitTaskEditor => self.exit_task_editor(action_tx.clone()),

                    Action::TaskOperationComplete => todo!(),
                    Action::TasksFetched => self.tasks_fetched(),
                }
            }

            if self.should_quit {
                tui.stop()?;
                break;
            }
        }

        tui.exit()?;
        Ok(())
    }

    /// Get the tasks for a specific view from cache
    fn get_view_tasks(&self, tab: ViewTab) -> &Vec<Task> {
        match tab {
            ViewTab::Today => &self.today_cache,
            ViewTab::Week => &self.week_cache,
            ViewTab::Inbox => &self.inbox_cache,
        }
    }

    /// Update the cache with new tasks and refresh the UI
    fn update_cache(
        &mut self,
        today_tasks: Vec<Task>,
        week_tasks: Vec<Task>,
        inbox_tasks: Vec<Task>,
    ) {
        // Update the caches
        self.today_cache = today_tasks;
        self.week_cache = week_tasks;
        self.inbox_cache = inbox_tasks;
        self.tasks_loaded = true;

        // Update UI with current view's tasks
        let current_tasks = match self.ui.get_current_tab() {
            ViewTab::Today => &self.today_cache,
            ViewTab::Week => &self.week_cache,
            ViewTab::Inbox => &self.inbox_cache,
        };
        self.ui.set_tasks(current_tasks);
        self.sync_task_editor_with_selection();
    }

    fn error(&mut self, msg: String) {
        self.error_message = Some(msg);
        self.error_ticks = 0;
        self.mode = Mode::Normal;
    }

    fn next_tick(&mut self) {
        if self.error_message.is_some() {
            self.error_ticks += 1;
            if self.error_ticks > 12 {
                // Clear after ~3 seconds (at 4 ticks/second)
                self.error_message = None;
                self.error_ticks = 0;
            }
        }
    }

    fn render(&mut self, tui: &mut Tui) -> Result<()> {
        self.current_tab = self.ui.get_current_tab();
        let tasks: &[Task] = match self.current_tab {
            ViewTab::Today => &self.today_cache,
            ViewTab::Week => &self.week_cache,
            ViewTab::Inbox => &self.inbox_cache,
        };
        let mode = self.mode;
        let error_message = &self.error_message;
        let tasks_loaded = self.tasks_loaded;
        let ui = &mut self.ui;
        tui.draw(|f| {
            let _ = ui.draw(
                f,
                f.area(),
                mode,
                tasks,
                error_message,
                tasks_loaded,
                self.task_editor_focused,
            );
        })
    }

    fn refresh_tasks(&mut self, tx: UnboundedSender<Action>) {
        self.mode = Mode::Processing;
        let client = Arc::clone(&self.client);
        let pending = Arc::clone(&self.pending_tasks);
        tokio::spawn(async move {
            match fetch_all_tasks(&client).await {
                Ok((today, week, inbox)) => {
                    // Store the tasks in pending storage
                    if let Ok(mut guard) = pending.lock() {
                        *guard = Some((today, week, inbox));
                    }
                    let _ = tx.send(Action::TasksFetched);
                }
                Err(e) => {
                    let _ = tx.send(Action::Error(e));
                }
            }
            let _ = tx.send(Action::ExitProcessing);
        });
    }

    fn tasks_fetched(&mut self) {
        let tasks_opt = if let Ok(mut guard) = self.pending_tasks.lock() {
            guard.take()
        } else {
            None
        };

        if let Some((today, week, inbox)) = tasks_opt {
            self.update_cache(today, week, inbox);
        }
    }

    fn complete_task(&mut self, tx: UnboundedSender<Action>) {
        let selected_indices = self.ui.get_selected_indices();
        if !selected_indices.is_empty() {
            let current_tab = self.ui.get_current_tab();
            let tasks_to_complete: Vec<_> = selected_indices
                .iter()
                .filter_map(|&index| self.get_view_tasks(current_tab).get(index))
                .map(|task| (task.get_id().clone(), task.project_id.clone()))
                .collect();

            if !tasks_to_complete.is_empty() {
                let client = Arc::clone(&self.client);
                self.mode = Mode::Processing;

                tokio::spawn(async move {
                    let mut errors = Vec::new();

                    // Complete all selected tasks
                    for (task_id, project_id) in tasks_to_complete {
                        let result =
                            tasks::complete_task_with_client(&client, &project_id, &task_id).await;
                        if let Err(e) = result {
                            errors.push(e);
                        }
                    }

                    // Send error if any tasks failed
                    if !errors.is_empty() {
                        let combined_error = format!(
                            "Failed to complete {} task(s): {}",
                            errors.len(),
                            errors.join(", ")
                        );
                        let _ = tx.send(Action::Error(combined_error));
                    } else {
                        let _ = tx.send(Action::RefreshTasks);
                    }
                    let _ = tx.send(Action::ExitProcessing);
                });
            }
        }
    }

    fn delete_task(&mut self, tx: UnboundedSender<Action>) {
        let selected_indices = self.ui.get_selected_indices();
        if !selected_indices.is_empty() {
            let current_tab = self.ui.get_current_tab();
            let tasks_to_delete: Vec<_> = selected_indices
                .iter()
                .filter_map(|&index| self.get_view_tasks(current_tab).get(index))
                .map(|task| (task.get_id().clone(), task.project_id.clone()))
                .collect();

            if !tasks_to_delete.is_empty() {
                let client = Arc::clone(&self.client);
                self.mode = Mode::Processing;

                tokio::spawn(async move {
                    let mut errors = Vec::new();

                    // Delete all selected tasks
                    for (task_id, project_id) in tasks_to_delete {
                        match ticks::tasks::Task::get(&client, &project_id, &task_id).await {
                            Ok(task) => {
                                let result = utils::delete_task(task).await;
                                if let Err(e) = result {
                                    errors.push(e);
                                }
                            }
                            Err(e) => {
                                errors.push(format!("Failed to fetch task: {:?}", e));
                            }
                        }
                    }

                    // Send error if any tasks failed
                    if !errors.is_empty() {
                        let combined_error = format!(
                            "Failed to delete {} task(s): {}",
                            errors.len(),
                            errors.join(", ")
                        );
                        let _ = tx.send(Action::Error(combined_error));
                    } else {
                        let _ = tx.send(Action::RefreshTasks);
                    }
                    let _ = tx.send(Action::ExitProcessing);
                });
            }
        }
    }

    fn start_postpone_task(&mut self) {
        if let Some(task_index) = self.ui.selected_index() {
            let current_tab = self.ui.get_current_tab();
            if let Some(_task) = self.get_view_tasks(current_tab).get(task_index) {
                // Default to 1 day from now
                let mut modal = PostponeModal::new("Postpone Task");
                modal.set_editor_to_insert_mode();

                self.mode = Mode::Insert;
                self.ui.start_modal(modal);
            }
        }
    }

    fn start_create_task(&mut self) {
        self.mode = Mode::Insert;
        // Set default date to today if in Today view
        let default_date = if self.current_tab == ViewTab::Today {
            Some(chrono::Local::now().format("%m/%d/%Y").to_string())
        } else {
            None
        };
        let modal = TaskModal::new_with_defaults("New Task", None, None, default_date, None, false);
        self.ui.start_modal(modal);
    }

    fn start_delete_task(&mut self) {
        let selected_indices = self.ui.get_selected_indices();
        if selected_indices.is_empty() {
            return;
        }

        let count = selected_indices.len();
        let message = if count == 1 {
            "Are you sure you want to delete this task?".to_string()
        } else {
            format!("Are you sure you want to delete these {} tasks?", count)
        };

        let modal = ConfirmationModal::new("Delete Task", &message);
        self.ui.start_modal(modal);
    }

    fn start_complete_task(&mut self) {
        let selected_indices = self.ui.get_selected_indices();
        if selected_indices.is_empty() {
            return;
        }

        let count = selected_indices.len();
        let message = if count == 1 {
            "Are you sure you want to mark this task as complete?".to_string()
        } else {
            format!(
                "Are you sure you want to mark these {} tasks as complete?",
                count
            )
        };

        let modal = ConfirmationModal::new_with_type(
            "Complete Task",
            &message,
            crate::modal::ConfirmationType::Complete,
        );
        self.ui.start_modal(modal);
    }

    fn start_edit_task(&mut self) {
        if let Some(task_index) = self.ui.selected_index() {
            let current_tab = self.ui.get_current_tab();

            let task_data = self
                .get_view_tasks(current_tab)
                .get(task_index)
                .map(|task| {
                    let task_title = task.title.clone();
                    let task_description = if !task.content.is_empty() {
                        Some(task.content.clone())
                    } else {
                        None
                    };
                    let task_id = task.get_id().clone();
                    let project_id = task.project_id.clone();

                    let task_date = if task.due_date.timestamp() > 0 {
                        Some(
                            task.due_date
                                .with_timezone(&chrono::Local)
                                .format("%m/%d/%Y")
                                .to_string(),
                        )
                    } else {
                        None
                    };

                    let task_time = if task.due_date.timestamp() > 0 && !task.is_all_day {
                        Some(
                            task.due_date
                                .with_timezone(&chrono::Local)
                                .format("%-I:%M %p")
                                .to_string()
                                .to_lowercase(),
                        )
                    } else {
                        None
                    };

                    (
                        task_title,
                        task_description,
                        task_date,
                        task_time,
                        project_id,
                        task_id,
                    )
                });

            if let Some((task_title, task_description, task_date, task_time, project_id, task_id)) =
                task_data
            {
                self.mode = Mode::Insert;
                self.editing_task = Some((project_id, task_id));
                let modal = TaskModal::new_with_defaults(
                    "Edit Task",
                    Some(task_title),
                    task_description,
                    task_date,
                    task_time,
                    true,
                );
                self.ui.start_modal(modal);
            }
        }
    }
    fn confirm_input(&mut self, tx: UnboundedSender<Action>) {
        // Handle task editor save
        if self.task_editor_focused {
            // Only save if there are actual changes
            if self.ui.task_editor.has_changes() {
                self.save_task_from_editor(tx);
            }
            return;
        }

        // Handle confirmation modal (for delete or complete confirmation)
        if self.ui.has_modal() && self.mode != Mode::Insert {
            // Check what type of confirmation modal this is
            let confirmation_type = self.ui.get_confirmation_type().cloned();
            self.ui.close_modal();

            if let Some(confirmation_type) = confirmation_type {
                match confirmation_type {
                    crate::modal::ConfirmationType::Delete => {
                        tx.send(Action::DeleteTask).unwrap();
                    }
                    crate::modal::ConfirmationType::Complete => {
                        tx.send(Action::CompleteTask).unwrap();
                    }
                }
            } else {
                // Fallback for other modal types - assume delete for backward compatibility
                tx.send(Action::DeleteTask).unwrap();
            }
            return;
        }

        if self.mode == Mode::Insert {
            // Validate modal input before processing
            if self.ui.has_modal() && !self.ui.validate_modal() {
                // Validation failed, don't process the input
                // The modal will display error messages to the user
                return;
            }

            let values = if self.ui.has_modal() {
                self.ui.get_modal_values()
            } else {
                // No overlay system anymore, this shouldn't happen
                vec![]
            };

            // Check if this is a postpone modal (has 1 value: duration string)
            if self.ui.has_modal() && values.len() == 1 && !values[0].contains('\n') {
                // This might be a postpone operation - try to parse as duration
                if let Ok(postpone_target) = utils::parse_duration(&values[0]) {
                    // This is a postpone operation - handle multiple selected tasks
                    let selected_indices = self.ui.get_selected_indices();
                    if !selected_indices.is_empty() {
                        let current_tab = self.ui.get_current_tab();
                        let tasks_to_postpone: Vec<_> = selected_indices
                            .iter()
                            .filter_map(|&index| self.get_view_tasks(current_tab).get(index))
                            .map(|task| (task.get_id().clone(), task.project_id.clone()))
                            .collect();

                        if !tasks_to_postpone.is_empty() {
                            let client = Arc::clone(&self.client);
                            self.mode = Mode::Processing;

                            tokio::spawn(async move {
                                let mut errors = Vec::new();

                                // First, fetch all tasks to calculate relative offsets for absolute time targets
                                let mut tasks_with_data = Vec::new();
                                for (task_id, project_id) in tasks_to_postpone {
                                    match ticks::tasks::Task::get(&client, &project_id, &task_id)
                                        .await
                                    {
                                        Ok(task) => {
                                            tasks_with_data.push(task);
                                        }
                                        Err(e) => {
                                            errors.push(format!("Failed to fetch task: {:?}", e));
                                        }
                                    }
                                }

                                // Calculate the base time for absolute targets
                                let base_datetime_utc = match &postpone_target {
                                    utils::PostponeTarget::RelativeToDueDate(_) => {
                                        // For relative targets, we don't need a base time
                                        None
                                    }
                                    utils::PostponeTarget::AbsoluteTime(datetime) => {
                                        // For absolute targets, find the earliest task's due date
                                        if let Some(earliest_task) =
                                            tasks_with_data.iter().min_by_key(|task| task.due_date)
                                        {
                                            Some((
                                                datetime.with_timezone(&chrono::Utc),
                                                earliest_task.due_date,
                                            ))
                                        } else {
                                            None
                                        }
                                    }
                                };

                                // Postpone all selected tasks
                                for mut task in tasks_with_data {
                                    // Calculate the new due datetime based on the postpone target
                                    let new_datetime_utc =
                                        match (&postpone_target, &base_datetime_utc) {
                                            (
                                                utils::PostponeTarget::RelativeToDueDate(duration),
                                                _,
                                            ) => {
                                                // Add duration to the task's original due_date
                                                task.due_date + *duration
                                            }
                                            (
                                                utils::PostponeTarget::AbsoluteTime(_),
                                                Some((target_time, earliest_due_date)),
                                            ) => {
                                                // Calculate the offset from the earliest task and apply it to the target time
                                                let offset_from_earliest =
                                                    task.due_date - *earliest_due_date;
                                                *target_time + offset_from_earliest
                                            }
                                            (
                                                utils::PostponeTarget::AbsoluteTime(datetime),
                                                None,
                                            ) => {
                                                // Fallback: use the absolute datetime (shouldn't happen with proper logic)
                                                datetime.with_timezone(&chrono::Utc)
                                            }
                                        };

                                    // Convert to local timezone before extracting naive date/time
                                    // so the API interprets them correctly as local time
                                    let new_datetime_local =
                                        new_datetime_utc.with_timezone(&chrono::Local);
                                    let due_date = new_datetime_local.date_naive();
                                    let due_time = new_datetime_local.time();

                                    let result = tasks::edit_task(
                                        &mut task,
                                        None,
                                        None,
                                        None,
                                        None,
                                        None,
                                        Some(due_date),
                                        Some(due_time),
                                    )
                                    .await;

                                    if let Err(e) = result {
                                        errors.push(e);
                                    }
                                }

                                // Send error if any tasks failed
                                if !errors.is_empty() {
                                    let combined_error = format!(
                                        "Failed to postpone {} task(s): {}",
                                        errors.len(),
                                        errors.join(", ")
                                    );
                                    let _ = tx.send(Action::Error(combined_error));
                                } else {
                                    let _ = tx.send(Action::RefreshTasks);
                                }
                                let _ = tx.send(Action::ExitProcessing);
                            });
                        }
                    }

                    self.ui.close_modal();
                    self.mode = Mode::Normal;
                    return;
                }
            }

            // Handle create/edit task modal (has 4 values: title, description, date, time)
            if !values.is_empty() && !values[0].is_empty() {
                let title = values[0].clone();
                let description = if values.len() > 1 {
                    values[1].clone()
                } else {
                    String::new()
                };
                let date = if values.len() > 2 {
                    values[2].clone()
                } else {
                    String::new()
                };
                let time = if values.len() > 3 {
                    values[3].clone()
                } else {
                    String::new()
                };

                let client = Arc::clone(&self.client);
                self.mode = Mode::Processing;
                let editing_task = self.editing_task.take();

                tokio::spawn(async move {
                    let due_date = parse_date_us_format(&date).ok();
                    let due_time = parse_time_us_format(&time).ok();

                    let result = if let Some((project_id, task_id)) = editing_task {
                        // Editing existing task
                        match ticks::tasks::Task::get(&client, &project_id, &task_id).await {
                            Ok(mut task) => {
                                let content = if !description.is_empty() {
                                    Some(description)
                                } else {
                                    None
                                };
                                tasks::edit_task(
                                    &mut task,
                                    Some(title),
                                    None,
                                    content,
                                    None,
                                    None,
                                    due_date,
                                    due_time,
                                )
                                .await
                            }
                            Err(e) => Err(format!("Failed to fetch task: {:?}", e)),
                        }
                    } else {
                        // Creating new task
                        let content = if !description.is_empty() {
                            Some(description)
                        } else {
                            None
                        };
                        tasks::create_task(
                            &client, title, None, content, None, None, due_date, due_time,
                        )
                        .await
                    };

                    if let Err(e) = result {
                        let _ = tx.send(Action::Error(e));
                    } else {
                        let _ = tx.send(Action::RefreshTasks);
                    }
                    let _ = tx.send(Action::ExitProcessing);
                });
            }

            if self.ui.has_modal() {
                self.ui.close_modal();
            }
            // No overlay system anymore, should always have modal in Insert mode
            self.editing_task = None;
            self.mode = Mode::Normal;
        }
    }

    fn cancel_input(&mut self) {
        // Handle modal cancellation (including confirmation modals)
        if self.ui.has_modal() {
            self.ui.close_modal();
            // For confirmation modals, don't change mode or exit visual mode
            if self.mode == Mode::Insert {
                self.editing_task = None;
                self.mode = Mode::Normal;
            }
            return;
        }

        if self.mode == Mode::Insert {
            // No overlay system anymore, should always have modal in Insert mode
            self.editing_task = None;
            self.mode = Mode::Normal;
        }
    }

    fn next_tab(&mut self) {
        self.ui.next_tab();
        self.current_tab = self.ui.get_current_tab();
        match self.current_tab {
            ViewTab::Today => self.ui.set_tasks(&self.today_cache),
            ViewTab::Week => self.ui.set_tasks(&self.week_cache),
            ViewTab::Inbox => self.ui.set_tasks(&self.inbox_cache),
        }
        self.sync_task_editor_with_selection();
    }

    fn previous_tab(&mut self) {
        self.ui.previous_tab();
        self.current_tab = self.ui.get_current_tab();
        match self.current_tab {
            ViewTab::Today => self.ui.set_tasks(&self.today_cache),
            ViewTab::Week => self.ui.set_tasks(&self.week_cache),
            ViewTab::Inbox => self.ui.set_tasks(&self.inbox_cache),
        }
        self.sync_task_editor_with_selection();
    }

    fn toggle_help(&mut self) {
        if self.mode == Mode::Help {
            self.mode = Mode::Normal;
        } else {
            self.mode = Mode::Help;
        }
    }

    fn sync_task_editor_with_selection(&mut self) {
        if let Some(selected_index) = self.ui.selected_index() {
            let tasks = match self.current_tab {
                ViewTab::Today => &self.today_cache,
                ViewTab::Week => &self.week_cache,
                ViewTab::Inbox => &self.inbox_cache,
            };
            if let Some(task) = tasks.get(selected_index) {
                let date_str = if task.due_date.timestamp() > 0 {
                    let local: DateTime<Local> = task.due_date.with_timezone(&Local);
                    local.format("%m/%d/%Y").to_string()
                } else {
                    String::new()
                };
                let time_str = if task.due_date.timestamp() > 0 && !task.is_all_day {
                    let local: DateTime<Local> = task.due_date.with_timezone(&Local);
                    local.format("%I:%M %p").to_string()
                } else {
                    String::new()
                };
                self.ui
                    .task_editor
                    .set_values(&task.title, &task.content, &date_str, &time_str);
                self.ui.task_editor.is_edit_mode = true;
            } else {
                // Clear editor fields when selected index is out of range
                self.ui.task_editor.set_values("", "", "", "");
                self.ui.task_editor.is_edit_mode = false;
            }
        } else {
            // Clear editor fields when no task is selected
            self.ui.task_editor.set_values("", "", "", "");
            self.ui.task_editor.is_edit_mode = false;
        }
    }

    fn enter_task_editor(&mut self) {
        if self.ui.selected_index().is_some() {
            self.task_editor_focused = true;
            // Task data is already synced by sync_task_editor_with_selection
        }
    }

    fn save_task_before_changing_selection(&mut self, tx: UnboundedSender<Action>) {
        // Only save if we're currently focused on the task editor and have valid input and changes
        if self.task_editor_focused {
            if self.ui.task_editor.validate() && self.ui.task_editor.has_changes() {
                self.save_task_from_editor(tx);
            }
            // Note: If validation fails or no changes, we still allow the selection change
            // but don't save invalid data or unchanged data
        }
    }

    fn exit_task_editor(&mut self, tx: UnboundedSender<Action>) {
        // Validate before saving changes
        if self.ui.task_editor.validate() {
            // Valid input - check if there are changes to save
            if self.ui.task_editor.has_changes() {
                self.save_task_from_editor(tx);
            }
            self.task_editor_focused = false;
            self.mode = Mode::Normal;
        } else {
            // Validation failed - stay in editor to let user see errors and fix them
            // The validation method has already set the error messages which will be displayed
        }
    }

    fn save_task_from_editor(&mut self, tx: UnboundedSender<Action>) {
        if let Some(selected_index) = self.ui.selected_index() {
            let tasks = match self.current_tab {
                ViewTab::Today => &self.today_cache,
                ViewTab::Week => &self.week_cache,
                ViewTab::Inbox => &self.inbox_cache,
            };

            if let Some(task) = tasks.get(selected_index) {
                let task_id = task.get_id().clone();
                let project_id = task.project_id.clone();

                let title = self.ui.task_editor.get_input_title();
                let description = self.ui.task_editor.get_input_description();
                let date_str = self.ui.task_editor.get_input_date();
                let time_str = self.ui.task_editor.get_input_time();

                // Parse date and time
                let parsed_date = if !date_str.trim().is_empty() {
                    parse_date_us_format(&date_str).ok()
                } else {
                    None
                };

                let parsed_time = if !time_str.trim().is_empty() {
                    parse_time_us_format(&time_str).ok()
                } else {
                    None
                };

                let client = Arc::clone(&self.client);
                self.mode = Mode::Processing;
                self.task_editor_focused = false;

                tokio::spawn(async move {
                    // Get fresh task data
                    match client.get_project_data(&project_id).await {
                        Ok(project_data) => {
                            let task_id_str = format!("{:?}", task_id);
                            if let Some(mut task) = project_data.tasks.into_iter().find(|t| {
                                let t_id_str = format!("{:?}", t.get_id());
                                t_id_str == task_id_str
                            }) {
                                let result = tasks::edit_task(
                                    &mut task,
                                    if !title.trim().is_empty() {
                                        Some(title)
                                    } else {
                                        None
                                    },
                                    None, // project
                                    if !description.trim().is_empty() {
                                        Some(description)
                                    } else {
                                        None
                                    },
                                    None, // description (legacy)
                                    None, // priority
                                    parsed_date,
                                    parsed_time,
                                )
                                .await;

                                if let Err(e) = result {
                                    let _ = tx.send(Action::Error(e));
                                } else {
                                    let _ = tx.send(Action::RefreshTasks);
                                }
                            } else {
                                let _ = tx.send(Action::Error("Task not found".to_string()));
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(Action::Error(format!(
                                "Failed to get project data: {:?}",
                                e
                            )));
                        }
                    }
                    let _ = tx.send(Action::ExitProcessing);
                });
            }
        }
    }

    fn handle_key_event(
        &mut self,
        key: KeyEvent,
        action_tx: &mpsc::UnboundedSender<Action>,
    ) -> Result<()> {
        use crossterm::event::KeyCode;

        // Handle modal key events first, regardless of mode
        if self.ui.has_modal() && self.mode != Mode::Insert {
            // This is a confirmation modal
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    action_tx.send(Action::ConfirmInput)?;
                    return Ok(());
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    action_tx.send(Action::CancelInput)?;
                    return Ok(());
                }
                _ => {
                    // Ignore other keys for confirmation modals
                    return Ok(());
                }
            }
        }

        match self.mode {
            Mode::Normal => {
                if self.task_editor_focused {
                    // Handle keys when task editor is focused
                    match key.code {
                        KeyCode::Char('q') => action_tx.send(Action::Quit)?,
                        KeyCode::Esc => {
                            // Check current editor mode before handling Esc
                            let editor = match self.ui.task_editor.current_input_field {
                                crate::ui::InputField::Title => {
                                    &self.ui.task_editor.input_title_editor
                                }
                                crate::ui::InputField::Description => {
                                    &self.ui.task_editor.input_description_editor
                                }
                                crate::ui::InputField::Date => {
                                    &self.ui.task_editor.input_date_editor
                                }
                                crate::ui::InputField::Time => {
                                    &self.ui.task_editor.input_time_editor
                                }
                            };

                            // If in Insert, Visual, or Search mode, let edtui handle Esc
                            if editor.mode != edtui::EditorMode::Normal {
                                let _ = self.ui.task_editor.handle_input_key_event(key);
                            } else {
                                // Already in Normal mode, exit task editor
                                action_tx.send(Action::ExitTaskEditor)?
                            }
                        }
                        KeyCode::Char('h') | KeyCode::Left => {
                            // Try edtui first, fallback to pane navigation if at line start
                            if self.ui.task_editor.is_at_line_start() {
                                action_tx.send(Action::ExitTaskEditor)?
                            } else {
                                let _ = self.ui.task_editor.handle_input_key_event(key);
                                // Update desired column after horizontal movement
                                self.ui.task_editor.update_desired_column();
                            }
                        }
                        KeyCode::Char('l') | KeyCode::Right => {
                            // Always let edtui handle 'l' - it handles cursor movement within fields
                            let _ = self.ui.task_editor.handle_input_key_event(key);
                            // Update desired column after horizontal movement
                            self.ui.task_editor.update_desired_column();
                        }
                        KeyCode::Char('j') | KeyCode::Down => {
                            // Check if current editor is in insert mode first
                            if self.ui.task_editor.is_current_editor_in_insert_mode() {
                                // In insert mode, just let edtui handle it normally (insert 'j' character)
                                let _ = self.ui.task_editor.handle_input_key_event(key);
                            } else {
                                // In normal mode, handle navigation logic
                                // Handle j: try moving within multi-line field first, then between fields
                                if self.ui.task_editor.current_input_field
                                    == crate::ui::InputField::Description
                                    && !self.ui.task_editor.is_at_last_line_in_multiline_field()
                                {
                                    // Let edtui handle line movement within description
                                    let _ = self.ui.task_editor.handle_input_key_event(key);
                                    // Update desired column after vertical movement within same field
                                    self.ui.task_editor.update_desired_column();
                                } else {
                                    // Move to next field
                                    if !self.ui.task_editor.handle_j_navigation() {
                                        let _ = self.ui.task_editor.handle_input_key_event(key);
                                        self.ui.task_editor.update_desired_column();
                                    }
                                }
                            }
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            // Check if current editor is in insert mode first
                            if self.ui.task_editor.is_current_editor_in_insert_mode() {
                                // In insert mode, just let edtui handle it normally (insert 'k' character)
                                let _ = self.ui.task_editor.handle_input_key_event(key);
                            } else {
                                // In normal mode, handle navigation logic
                                // Handle k: try moving within multi-line field first, then between fields
                                if self.ui.task_editor.current_input_field
                                    == crate::ui::InputField::Description
                                    && !self.ui.task_editor.is_at_first_line_in_multiline_field()
                                {
                                    // Let edtui handle line movement within description
                                    let _ = self.ui.task_editor.handle_input_key_event(key);
                                    // Update desired column after vertical movement within same field
                                    self.ui.task_editor.update_desired_column();
                                } else {
                                    // Move to previous field
                                    if !self.ui.task_editor.handle_k_navigation() {
                                        let _ = self.ui.task_editor.handle_input_key_event(key);
                                        self.ui.task_editor.update_desired_column();
                                    }
                                }
                            }
                        }
                        KeyCode::Enter => {
                            // For description field, let edtui handle Enter (newline)
                            // For other fields, save the task
                            match self.ui.task_editor.current_input_field {
                                crate::ui::InputField::Description => {
                                    let _ = self.ui.task_editor.handle_input_key_event(key);
                                }
                                _ => {
                                    action_tx.send(Action::ConfirmInput)?;
                                }
                            }
                        }
                        KeyCode::Tab => {
                            self.ui.task_editor.next_input_field();
                            self.ui.task_editor.position_cursor_at_desired_column();
                        }
                        KeyCode::BackTab => {
                            self.ui.task_editor.previous_input_field();
                            self.ui.task_editor.position_cursor_at_desired_column();
                        }
                        KeyCode::Char('g') => {
                            // Check if current editor is in insert mode first
                            if self.ui.task_editor.is_current_editor_in_insert_mode() {
                                // In insert mode, just let edtui handle it normally (insert 'g' character)
                                let _ = self.ui.task_editor.handle_input_key_event(key);
                            } else {
                                // In normal mode, move to first line of first field or first line of current field if in description
                                self.ui.task_editor.navigate_to_first_field_or_line();
                            }
                        }
                        KeyCode::Char('G') => {
                            // Check if current editor is in insert mode first
                            if self.ui.task_editor.is_current_editor_in_insert_mode() {
                                // In insert mode, just let edtui handle it normally (insert 'G' character)
                                let _ = self.ui.task_editor.handle_input_key_event(key);
                            } else {
                                // In normal mode, move to last line of last field or last line of current field if in description
                                self.ui.task_editor.navigate_to_last_field_or_line();
                            }
                        }
                        KeyCode::Char('s') => {
                            // Ctrl+S saves the task
                            if key
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL)
                            {
                                action_tx.send(Action::ConfirmInput)?;
                            } else {
                                // Regular 's' command - let edtui handle it
                                let _ = self.ui.task_editor.handle_input_key_event(key);
                                self.ui.task_editor.update_desired_column();
                            }
                        }
                        _ => {
                            // Handle different types of keys for proper desired column behavior
                            match key.code {
                                // Keys that should preserve desired column (vertical movement)
                                KeyCode::Up | KeyCode::Down => {
                                    let _ = self.ui.task_editor.handle_input_key_event(key);
                                    // Don't update desired_column for pure vertical movement
                                }
                                // Keys that should update desired column (horizontal movement, word movement, etc.)
                                KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End => {
                                    let _ = self.ui.task_editor.handle_input_key_event(key);
                                    self.ui.task_editor.update_desired_column();
                                }
                                // Character keys like 'w', 'b', 'e', '0', '$', etc.
                                KeyCode::Char(c) => match c {
                                    // Horizontal movement commands that should update desired column
                                    'w' | 'b' | 'e' | '0' | '$' | '_' | 'f' | 'F' | 't' | 'T' => {
                                        let _ = self.ui.task_editor.handle_input_key_event(key);
                                        self.ui.task_editor.update_desired_column();
                                    }
                                    // Commands that change to insert mode and should update desired column
                                    'i' | 'a' | 'I' | 'A' | 'c' | 's' | 'x' | 'r' => {
                                        let _ = self.ui.task_editor.handle_input_key_event(key);
                                        self.ui.task_editor.update_desired_column();
                                    }
                                    // Vertical movement commands that should preserve desired column
                                    'j' | 'k' => {
                                        let _ = self.ui.task_editor.handle_input_key_event(key);
                                        // Don't update desired_column for these
                                    }
                                    // All other character commands
                                    _ => {
                                        let _ = self.ui.task_editor.handle_input_key_event(key);
                                        // For safety, update desired column for unknown commands
                                        self.ui.task_editor.update_desired_column();
                                    }
                                },
                                // For other keys, use the safe method that updates desired column
                                _ => {
                                    let _ = self
                                        .ui
                                        .task_editor
                                        .handle_input_key_event_and_update_column(key);
                                }
                            }
                        }
                    }
                } else {
                    // Handle keys when task list is focused
                    match key.code {
                        KeyCode::Char('q') => action_tx.send(Action::Quit)?,
                        KeyCode::Esc => action_tx.send(Action::SelectNone)?,
                        KeyCode::Char('r') => action_tx.send(Action::RefreshTasks)?,
                        KeyCode::Char('n') => action_tx.send(Action::StartCreateTask)?,
                        KeyCode::Char('e') => action_tx.send(Action::StartCompleteTask)?,
                        KeyCode::Char('d') => action_tx.send(Action::StartDeleteTask)?,
                        KeyCode::Char('p') => {
                            if key
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL)
                                || key
                                    .modifiers
                                    .contains(crossterm::event::KeyModifiers::SUPER)
                            {
                                action_tx.send(Action::StartPostponeTask)?
                            }
                        }
                        KeyCode::Char('?') => action_tx.send(Action::ToggleHelp)?,
                        KeyCode::Char('j') => action_tx.send(Action::SelectNext)?,
                        KeyCode::Char('k') => action_tx.send(Action::SelectPrevious)?,
                        KeyCode::Down => action_tx.send(Action::SelectNextCycling)?,
                        KeyCode::Up => action_tx.send(Action::SelectPreviousCycling)?,
                        KeyCode::Char('g') | KeyCode::Home => {
                            action_tx.send(Action::SelectFirst)?
                        }
                        KeyCode::Char('G') | KeyCode::End => action_tx.send(Action::SelectLast)?,

                        // Tab/BackTab now switch tabs
                        KeyCode::Tab => action_tx.send(Action::NextTab)?,
                        KeyCode::BackTab => action_tx.send(Action::PreviousTab)?,

                        // 'l' enters task editor when task is selected
                        KeyCode::Char('l') | KeyCode::Right => {
                            if self.ui.selected_index().is_some() {
                                action_tx.send(Action::EnterTaskEditor)?;
                            }
                        }

                        KeyCode::Enter => action_tx.send(Action::StartEditTask)?,
                        KeyCode::Char('v') => action_tx.send(Action::EnterVisual)?,
                        _ => {}
                    }
                }
            }
            Mode::Insert => {
                if self.task_editor_focused {
                    // Task editor handles its own modes via edtui - we should not reach here
                    // All task editor keys are handled in Normal mode now
                    // This is a fallback for safety
                    let _ = self.ui.task_editor.handle_input_key_event(key);
                } else {
                    // Handle modal insert mode
                    match key.code {
                        KeyCode::Esc => {
                            // If there's a modal, let it handle the Esc key first
                            if self.ui.has_modal() {
                                // Pass Esc to modal - it will return false if it wants the app to handle it
                                let handled = self.ui.handle_modal_key_event(key).unwrap_or(false);
                                if !handled {
                                    // Modal didn't handle it (already in normal mode), close the modal
                                    action_tx.send(Action::CancelInput)?;
                                }
                            } else {
                                // No overlay system anymore, should always have modal in Insert mode
                                action_tx.send(Action::CancelInput)?;
                            }
                        }
                        KeyCode::Enter => {
                            // Cmd+Enter or Ctrl+Enter always confirms
                            if key
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL)
                                || key
                                    .modifiers
                                    .contains(crossterm::event::KeyModifiers::SUPER)
                            {
                                action_tx.send(Action::ConfirmInput)?;
                            }
                            // For modals, pass Enter to the modal to handle
                            else {
                                if self.ui.has_modal() {
                                    // Pass Enter to modal - it will return false if it wants the app to handle it
                                    let handled =
                                        self.ui.handle_modal_key_event(key).unwrap_or(false);
                                    if !handled {
                                        // Modal didn't handle it (in normal mode), confirm the input
                                        action_tx.send(Action::ConfirmInput)?;
                                    }
                                }
                            }
                        }
                        _ => {
                            if self.mode == Mode::Insert {
                                if self.ui.has_modal() {
                                    let _ = self.ui.handle_modal_key_event(key);
                                }
                                // No overlay system anymore
                            }
                        }
                    }
                }
            }
            Mode::Visual => match key.code {
                KeyCode::Esc => action_tx.send(Action::EnterNormal)?,
                KeyCode::Char('j') => action_tx.send(Action::SelectNext)?,
                KeyCode::Char('k') => action_tx.send(Action::SelectPrevious)?,
                KeyCode::Down => action_tx.send(Action::SelectNextCycling)?,
                KeyCode::Up => action_tx.send(Action::SelectPreviousCycling)?,
                KeyCode::Tab => action_tx.send(Action::SelectNextCycling)?,
                KeyCode::BackTab => action_tx.send(Action::SelectPreviousCycling)?,
                KeyCode::Char('g') | KeyCode::Home => action_tx.send(Action::SelectFirst)?,
                KeyCode::Char('G') | KeyCode::End => action_tx.send(Action::SelectLast)?,
                KeyCode::Char('e') => action_tx.send(Action::StartCompleteTask)?,
                KeyCode::Char('d') => action_tx.send(Action::StartDeleteTask)?,

                KeyCode::Char('p') => {
                    if key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL)
                        || key
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::SUPER)
                    {
                        action_tx.send(Action::StartPostponeTask)?
                    }
                }
                _ => {}
            },
            Mode::Processing => {
                // Ignore input while processing
            }
            Mode::Help => match key.code {
                KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q') => {
                    action_tx.send(Action::ToggleHelp)?
                }
                _ => {}
            },
        }

        Ok(())
    }
}
