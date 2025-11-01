use anyhow::Result;
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
}

impl App {
    pub fn new(client: TickTick) -> Result<Self> {
        let mode = Mode::Normal;
        let ui = TaskListUI::new();
        Ok(Self {
            should_quit: false,
            mode,
            ui,
            client: Arc::new(client),
            error_message: None,
            error_ticks: 0,
            today_cache: Vec::new(),
            week_cache: Vec::new(),
            inbox_cache: Vec::new(),
            tasks_loaded: false,
            pending_tasks: Arc::new(Mutex::new(None)),
            current_tab: ViewTab::Today,
            editing_task: None,
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

                    Action::SelectPrevious => self.ui.select_previous(),
                    Action::SelectNext => self.ui.select_next(),
                    Action::SelectPreviousCycling => {
                        let current_tab = self.ui.current_tab;
                        let tasks = self.get_view_tasks(current_tab);
                        self.ui.select_previous_cycling(tasks.len());
                    }
                    Action::SelectNextCycling => {
                        let current_tab = self.ui.current_tab;
                        let tasks = self.get_view_tasks(current_tab);
                        self.ui.select_next_cycling(tasks.len());
                    }
                    Action::SelectFirst => self.ui.select_first(),
                    Action::SelectLast => self.ui.select_last(),
                    Action::SelectNone => self.ui.select_none(),

                    Action::PreviousTab => self.previous_tab(),
                    Action::NextTab => self.next_tab(),

                    Action::CompleteTask => self.complete_task(action_tx.clone()),
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
            let _ = ui.draw(f, f.area(), mode, tasks, error_message, tasks_loaded);
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
                let modal =
                    PostponeModal::new_with_default("Postpone Task", Some("1 day".to_string()));

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
        // Handle confirmation modal (for delete confirmation)
        if self.ui.has_modal() && self.mode != Mode::Insert {
            // This is a confirmation modal, trigger the delete action
            self.ui.close_modal();
            tx.send(Action::DeleteTask).unwrap();
            return;
        }

        if self.mode == Mode::Insert {
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
    }

    fn previous_tab(&mut self) {
        self.ui.previous_tab();
        self.current_tab = self.ui.get_current_tab();
        match self.current_tab {
            ViewTab::Today => self.ui.set_tasks(&self.today_cache),
            ViewTab::Week => self.ui.set_tasks(&self.week_cache),
            ViewTab::Inbox => self.ui.set_tasks(&self.inbox_cache),
        }
    }

    fn toggle_help(&mut self) {
        if self.mode == Mode::Help {
            self.mode = Mode::Normal;
        } else {
            self.mode = Mode::Help;
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
            Mode::Normal => match key.code {
                KeyCode::Char('q') => action_tx.send(Action::Quit)?,
                KeyCode::Esc => action_tx.send(Action::SelectNone)?,
                KeyCode::Char('r') => action_tx.send(Action::RefreshTasks)?,
                KeyCode::Char('n') => action_tx.send(Action::StartCreateTask)?,
                KeyCode::Char('e') => action_tx.send(Action::StartEditTask)?,
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
                KeyCode::Char('j') | KeyCode::Down => action_tx.send(Action::SelectNextCycling)?,
                KeyCode::Char('k') | KeyCode::Up => {
                    action_tx.send(Action::SelectPreviousCycling)?
                }
                KeyCode::Tab => action_tx.send(Action::SelectNextCycling)?,
                KeyCode::BackTab => action_tx.send(Action::SelectPreviousCycling)?,
                KeyCode::Char('g') | KeyCode::Home => action_tx.send(Action::SelectFirst)?,
                KeyCode::Char('G') | KeyCode::End => action_tx.send(Action::SelectLast)?,
                KeyCode::Char('h') | KeyCode::Left => action_tx.send(Action::PreviousTab)?,
                KeyCode::Char('l') | KeyCode::Right => action_tx.send(Action::NextTab)?,
                KeyCode::Char(' ') => action_tx.send(Action::CompleteTask)?,
                KeyCode::Enter => action_tx.send(Action::StartEditTask)?,
                KeyCode::Char('v') => action_tx.send(Action::EnterVisual)?,
                _ => {}
            },
            Mode::Insert => {
                // Check for special keys first
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
                                let handled = self.ui.handle_modal_key_event(key).unwrap_or(false);
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
            Mode::Visual => match key.code {
                KeyCode::Esc => action_tx.send(Action::EnterNormal)?,
                KeyCode::Char('j') | KeyCode::Down => action_tx.send(Action::SelectNextCycling)?,
                KeyCode::Char('k') | KeyCode::Up => {
                    action_tx.send(Action::SelectPreviousCycling)?
                }
                KeyCode::Tab => action_tx.send(Action::SelectNextCycling)?,
                KeyCode::BackTab => action_tx.send(Action::SelectPreviousCycling)?,
                KeyCode::Char('g') | KeyCode::Home => action_tx.send(Action::SelectFirst)?,
                KeyCode::Char('G') | KeyCode::End => action_tx.send(Action::SelectLast)?,
                KeyCode::Char('d') => action_tx.send(Action::StartDeleteTask)?,
                KeyCode::Char(' ') => action_tx.send(Action::CompleteTask)?,
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
