use anyhow::Result;
use crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use std::sync::{Arc, Mutex};
use ticks::{tasks::Task, TickTick};
use tokio::sync::mpsc::{self, UnboundedSender};

use crate::{
    action::Action,
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
    pub inbox_cache: Vec<Task>,
    pub tasks_loaded: bool,
    pub pending_tasks: Arc<Mutex<Option<(Vec<Task>, Vec<Task>)>>>,
    pub current_tab: ViewTab,
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
            inbox_cache: Vec::new(),
            tasks_loaded: false,
            pending_tasks: Arc::new(Mutex::new(None)),
            current_tab: ViewTab::Today,
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
                    Action::SelectFirst => self.ui.select_first(),
                    Action::SelectLast => self.ui.select_last(),
                    Action::SelectNone => self.ui.select_none(),

                    Action::PreviousTab => self.previous_tab(),
                    Action::NextTab => self.next_tab(),

                    Action::CompleteTask => self.complete_task(action_tx.clone()),
                    Action::DeleteTask => self.delete_task(action_tx.clone()),
                    Action::PostponeTask => todo!(),
                    Action::StartCreateTask => self.start_create_task(),
                    Action::StartEditTask => todo!(),
                    Action::CancelInput => self.cancel_input(),
                    Action::ConfirmInput => self.confirm_input(action_tx.clone()),
                    Action::UpdateInput(s) => self.ui.update_input(s),

                    Action::EnterNormal => self.mode = Mode::Normal,
                    Action::EnterInsert => self.mode = Mode::Insert,
                    Action::EnterProcessing => self.mode = Mode::Processing,
                    Action::ExitProcessing => self.mode = Mode::Normal,

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
            ViewTab::Inbox => &self.inbox_cache,
        }
    }

    /// Update the cache with new tasks and refresh the UI
    fn update_cache(&mut self, today_tasks: Vec<Task>, inbox_tasks: Vec<Task>) {
        // Update the caches
        self.today_cache = today_tasks;
        self.inbox_cache = inbox_tasks;
        self.tasks_loaded = true;

        // Update UI with current view's tasks
        let current_tasks = match self.ui.get_current_tab() {
            ViewTab::Today => &self.today_cache,
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
                Ok((today, inbox)) => {
                    // Store the tasks in pending storage
                    if let Ok(mut guard) = pending.lock() {
                        *guard = Some((today, inbox));
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

        if let Some((today, inbox)) = tasks_opt {
            self.update_cache(today, inbox);
        }
    }

    fn complete_task(&mut self, tx: UnboundedSender<Action>) {
        if let Some(task_index) = self.ui.selected_index() {
            let current_tab = self.ui.get_current_tab();
            if let Some(task) = self.get_view_tasks(current_tab).get(task_index) {
                let task_id = task.get_id().clone();
                let project_id = task.project_id.clone();
                let client = Arc::clone(&self.client);

                self.mode = Mode::Processing;
                tokio::spawn(async move {
                    // Complete the task
                    let result =
                        tasks::complete_task_with_client(&client, &project_id, &task_id).await;
                    if let Err(e) = result {
                        let _ = tx.send(Action::Error(e));
                    } else {
                        let _ = tx.send(Action::RefreshTasks);
                    }
                    let _ = tx.send(Action::ExitProcessing);
                });
            }
        }
    }

    fn delete_task(&mut self, tx: UnboundedSender<Action>) {
        if let Some(task_index) = self.ui.selected_index() {
            let current_tab = self.ui.get_current_tab();
            if let Some(task) = self.get_view_tasks(current_tab).get(task_index) {
                let task_id = task.get_id().clone();
                let project_id = task.project_id.clone();
                let client = Arc::clone(&self.client);

                self.mode = Mode::Processing;
                tokio::spawn(async move {
                    // Fetch the task and delete it
                    match ticks::tasks::Task::get(&client, &project_id, &task_id).await {
                        Ok(task) => {
                            let result = utils::delete_task(task).await;
                            if let Err(e) = result {
                                let _ = tx.send(Action::Error(e));
                            } else {
                                let _ = tx.send(Action::RefreshTasks);
                            }
                        }
                        Err(e) => {
                            let _ =
                                tx.send(Action::Error(format!("Failed to fetch task: {:?}", e)));
                        }
                    }
                    let _ = tx.send(Action::ExitProcessing);
                });
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
        self.ui.start_input_mode("New Task", default_date);
    }

    fn confirm_input(&mut self, tx: UnboundedSender<Action>) {
        if self.mode == Mode::Insert {
            let title = self.ui.get_input_value().to_string();
            if !title.is_empty() {
                let client = Arc::clone(&self.client);
                self.mode = Mode::Processing;
                let date = self.ui.get_input_date();
                let time = self.ui.get_input_time();
                tokio::spawn(async move {
                    let due_date = parse_date_us_format(&date).ok();
                    let due_time = parse_time_us_format(&time).ok();
                    let result = tasks::create_task(
                        &client, title, None, None, None, None, due_date, due_time,
                    )
                    .await;
                    if let Err(e) = result {
                        let _ = tx.send(Action::Error(e));
                    } else {
                        let _ = tx.send(Action::RefreshTasks);
                    }
                    let _ = tx.send(Action::ExitProcessing);
                });
            }
            self.ui.clear_input();
            self.mode = Mode::Normal;
        }
    }

    fn cancel_input(&mut self) {
        if self.mode == Mode::Insert {
            self.ui.clear_input();
            self.mode = Mode::Normal;
        }
    }

    fn next_tab(&mut self) {
        self.ui.next_tab();
        self.current_tab = self.ui.get_current_tab();
        match self.current_tab {
            ViewTab::Today => self.ui.set_tasks(&self.today_cache),
            ViewTab::Inbox => self.ui.set_tasks(&self.inbox_cache),
        }
    }

    fn previous_tab(&mut self) {
        self.ui.previous_tab();
        self.current_tab = self.ui.get_current_tab();
        match self.current_tab {
            ViewTab::Today => self.ui.set_tasks(&self.today_cache),
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

        match self.mode {
            Mode::Normal => match key.code {
                KeyCode::Char('q') => action_tx.send(Action::Quit)?,
                KeyCode::Esc => action_tx.send(Action::SelectNone)?,
                KeyCode::Char('r') => action_tx.send(Action::RefreshTasks)?,
                KeyCode::Char('n') => action_tx.send(Action::StartCreateTask)?,
                KeyCode::Char('d') => action_tx.send(Action::DeleteTask)?,
                KeyCode::Char('p') => action_tx.send(Action::PostponeTask)?,
                KeyCode::Char('?') => action_tx.send(Action::ToggleHelp)?,
                KeyCode::Char('j') | KeyCode::Down => action_tx.send(Action::SelectNext)?,
                KeyCode::Char('k') | KeyCode::Up => action_tx.send(Action::SelectPrevious)?,
                KeyCode::Char('g') | KeyCode::Home => action_tx.send(Action::SelectFirst)?,
                KeyCode::Char('G') | KeyCode::End => action_tx.send(Action::SelectLast)?,
                KeyCode::Char('h') | KeyCode::Left => action_tx.send(Action::PreviousTab)?,
                KeyCode::Char('l') | KeyCode::Right => action_tx.send(Action::NextTab)?,
                KeyCode::Char(' ') | KeyCode::Enter => action_tx.send(Action::CompleteTask)?,
                _ => {}
            },
            Mode::Insert => match key.code {
                KeyCode::Esc => action_tx.send(Action::CancelInput)?,
                KeyCode::Enter => action_tx.send(Action::ConfirmInput)?,
                KeyCode::Tab => {
                    self.ui.next_input_field();
                }
                KeyCode::BackTab => {
                    self.ui.previous_input_field();
                }
                KeyCode::Char(c) => {
                    let mut input = self.ui.get_current_input();
                    input.push(c);
                    action_tx.send(Action::UpdateInput(input))?;
                }
                KeyCode::Backspace => {
                    let mut input = self.ui.get_current_input();
                    input.pop();
                    action_tx.send(Action::UpdateInput(input))?;
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
