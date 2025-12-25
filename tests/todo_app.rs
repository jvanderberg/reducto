//! Todo App - integration test for reducto framework
//!
//! Demonstrates: State, Actions, Reducer, TextView rendering, Effect-based side effects,
//! and view composition with the view! macro.

use core::fmt::Write;
use reducto::{App, Effect, Reducer, TextView, View};
use reducto_view::view;

// ============================================================================
// Effect type for Todo app
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq)]
enum TodoEffect {
    Unchanged,
    None,
}

impl Effect for TodoEffect {
    fn is_unchanged(&self) -> bool {
        matches!(self, TodoEffect::Unchanged)
    }
    fn changed() -> Self {
        TodoEffect::None
    }
}

// ============================================================================
// State
// ============================================================================

#[derive(Clone, Debug, Default)]
struct Todo {
    id: u32,
    text: heapless::String<32>,
    completed: bool,
}

#[derive(Clone, Debug, Default)]
struct TodoState {
    todos: heapless::Vec<Todo, 8>,
    next_id: u32,
    filter: Filter,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Filter {
    #[default]
    All,
    Active,
    Completed,
}

// ============================================================================
// Actions
// ============================================================================

#[derive(Clone, Debug)]
enum TodoAction {
    Add(heapless::String<32>),
    Toggle(u32),
    Delete(u32),
    SetFilter(Filter),
    ClearCompleted,
}

// ============================================================================
// Reducer
// ============================================================================

struct TodoReducer;

impl Reducer for TodoReducer {
    type State = TodoState;
    type Action = TodoAction;
    type Effect = TodoEffect;

    fn reduce(state: &mut Self::State, action: Self::Action) -> Self::Effect {
        match action {
            TodoAction::Add(text) => {
                if text.is_empty() {
                    return TodoEffect::Unchanged;
                }
                let todo = Todo {
                    id: state.next_id,
                    text,
                    completed: false,
                };
                state.todos.push(todo).ok();
                state.next_id += 1;
                TodoEffect::None
            }
            TodoAction::Toggle(id) => {
                if let Some(todo) = state.todos.iter_mut().find(|t| t.id == id) {
                    todo.completed = !todo.completed;
                    TodoEffect::None
                } else {
                    TodoEffect::Unchanged
                }
            }
            TodoAction::Delete(id) => {
                let original_len = state.todos.len();
                state.todos.retain(|t| t.id != id);
                if state.todos.len() != original_len {
                    TodoEffect::None
                } else {
                    TodoEffect::Unchanged
                }
            }
            TodoAction::SetFilter(filter) => {
                if filter == state.filter {
                    TodoEffect::Unchanged
                } else {
                    state.filter = filter;
                    TodoEffect::None
                }
            }
            TodoAction::ClearCompleted => {
                let original_len = state.todos.len();
                state.todos.retain(|t| !t.completed);
                if state.todos.len() != original_len {
                    TodoEffect::None
                } else {
                    TodoEffect::Unchanged
                }
            }
        }
    }
}

// ============================================================================
// View (using View trait)
// ============================================================================

struct TodoView {
    buffer: TextView<512>,
}

impl TodoView {
    fn new() -> Self {
        Self {
            buffer: TextView::new(),
        }
    }
}

impl View for TodoView {
    type State = TodoState;

    fn render(&mut self, state: &Self::State) {
        self.buffer.clear();
        let buf = self.buffer.buffer_mut();

        writeln!(buf, "=== TODO APP ===").ok();
        writeln!(buf, "Filter: {:?}", state.filter).ok();
        writeln!(buf, "----------------").ok();

        let filtered: heapless::Vec<&Todo, 8> = state
            .todos
            .iter()
            .filter(|t| match state.filter {
                Filter::All => true,
                Filter::Active => !t.completed,
                Filter::Completed => t.completed,
            })
            .collect();

        if filtered.is_empty() {
            writeln!(buf, "(no items)").ok();
        } else {
            for todo in &filtered {
                let mark = if todo.completed { "x" } else { " " };
                writeln!(buf, "[{}] {} (id:{})", mark, todo.text, todo.id).ok();
            }
        }

        let active_count = state.todos.iter().filter(|t| !t.completed).count();
        let completed_count = state.todos.iter().filter(|t| t.completed).count();
        writeln!(buf, "----------------").ok();
        writeln!(buf, "{} active, {} completed", active_count, completed_count).ok();
    }
}

// ============================================================================
// Tests
// ============================================================================

#[test]
fn todo_app_full_workflow() {
    let mut app: App<TodoReducer, TodoView> =
        App::new(TodoView::new(), TodoState::default());

    // Initial state - add an empty task (no-op) to check initial behavior
    let empty = heapless::String::<32>::new();
    let effect = app.dispatch(TodoAction::Add(empty.clone()));
    assert!(effect.is_unchanged()); // empty text doesn't add

    // Add some todos
    let mut text = heapless::String::<32>::new();
    text.push_str("Buy milk").ok();
    let effect = app.dispatch(TodoAction::Add(text));
    assert!(!effect.is_unchanged());
    assert!(app.view().buffer.as_str().contains("[ ] Buy milk"));

    let mut text = heapless::String::<32>::new();
    text.push_str("Write code").ok();
    app.dispatch(TodoAction::Add(text));

    let mut text = heapless::String::<32>::new();
    text.push_str("Test reducto").ok();
    app.dispatch(TodoAction::Add(text));

    assert_eq!(app.state().todos.len(), 3);
    assert!(app.view().buffer.as_str().contains("3 active, 0 completed"));

    // Toggle one complete
    app.dispatch(TodoAction::Toggle(1));
    assert!(app.view().buffer.as_str().contains("[x] Write code"));
    assert!(app.view().buffer.as_str().contains("2 active, 1 completed"));

    // Filter to active only
    app.dispatch(TodoAction::SetFilter(Filter::Active));
    assert!(app.view().buffer.as_str().contains("Filter: Active"));
    assert!(!app.view().buffer.as_str().contains("Write code")); // completed, shouldn't show
    assert!(app.view().buffer.as_str().contains("Buy milk"));

    // Filter to completed only
    app.dispatch(TodoAction::SetFilter(Filter::Completed));
    assert!(app.view().buffer.as_str().contains("[x] Write code"));
    assert!(!app.view().buffer.as_str().contains("Buy milk")); // active, shouldn't show

    // Back to all
    app.dispatch(TodoAction::SetFilter(Filter::All));
    assert!(app.view().buffer.as_str().contains("Buy milk"));
    assert!(app.view().buffer.as_str().contains("Write code"));

    // Delete a todo
    app.dispatch(TodoAction::Delete(0));
    assert!(!app.view().buffer.as_str().contains("Buy milk"));
    assert_eq!(app.state().todos.len(), 2);

    // Clear completed
    app.dispatch(TodoAction::ClearCompleted);
    assert!(!app.view().buffer.as_str().contains("Write code"));
    assert_eq!(app.state().todos.len(), 1);
    assert!(app.view().buffer.as_str().contains("Test reducto"));

    // No-op actions shouldn't change state
    let effect = app.dispatch(TodoAction::Add(empty));
    assert!(effect.is_unchanged());

    let effect = app.dispatch(TodoAction::Toggle(999)); // non-existent
    assert!(effect.is_unchanged());

    let effect = app.dispatch(TodoAction::Delete(999));
    assert!(effect.is_unchanged());

    let effect = app.dispatch(TodoAction::SetFilter(Filter::All)); // already set
    assert!(effect.is_unchanged());

    let effect = app.dispatch(TodoAction::ClearCompleted); // none completed
    assert!(effect.is_unchanged());

    println!("Final view:\n{}", app.view().buffer.as_str());
}

#[test]
fn todo_app_with_queue() {
    let mut app: App<TodoReducer, TodoView> =
        App::new(TodoView::new(), TodoState::default());

    // Simulate ISR-style: enqueue multiple actions
    let mut t1 = heapless::String::<32>::new();
    t1.push_str("Task 1").ok();
    let mut t2 = heapless::String::<32>::new();
    t2.push_str("Task 2").ok();

    app.enqueue(TodoAction::Add(t1)).ok();
    app.enqueue(TodoAction::Add(t2)).ok();
    app.enqueue(TodoAction::Toggle(0)).ok();

    // Process queue
    let effects = app.process_queue();

    assert_eq!(effects.len(), 3);
    assert!(app.view().buffer.as_str().contains("[x] Task 1"));
    assert!(app.view().buffer.as_str().contains("[ ] Task 2"));
}

// ============================================================================
// App integration test
// ============================================================================

#[test]
fn todo_app_with_app() {
    let mut app: App<TodoReducer, TodoView> =
        App::new(TodoView::new(), TodoState::default());

    // Add tasks - App handles rendering internally
    let mut text = heapless::String::<32>::new();
    text.push_str("Learn Rust").ok();
    let effect = app.dispatch(TodoAction::Add(text));
    assert!(!effect.is_unchanged());

    let mut text = heapless::String::<32>::new();
    text.push_str("Build firmware").ok();
    app.dispatch(TodoAction::Add(text));

    app.dispatch(TodoAction::Toggle(0));

    // Verify final state
    assert!(app.view().buffer.as_str().contains("[x] Learn Rust"));
    assert!(app.view().buffer.as_str().contains("[ ] Build firmware"));
    assert!(app.view().buffer.as_str().contains("1 active, 1 completed"));
}

// ============================================================================
// Effect with side effects demo
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq)]
enum TodoEffectWithSave {
    Unchanged,
    None,
    SaveRequired, // Signal that state should be persisted
}

impl Effect for TodoEffectWithSave {
    fn is_unchanged(&self) -> bool {
        matches!(self, TodoEffectWithSave::Unchanged)
    }
    fn changed() -> Self {
        TodoEffectWithSave::None
    }
}

struct TodoReducerWithSave;

impl Reducer for TodoReducerWithSave {
    type State = TodoState;
    type Action = TodoAction;
    type Effect = TodoEffectWithSave;

    fn reduce(state: &mut Self::State, action: Self::Action) -> Self::Effect {
        match action {
            TodoAction::Add(text) => {
                if text.is_empty() {
                    return TodoEffectWithSave::Unchanged;
                }
                let todo = Todo {
                    id: state.next_id,
                    text,
                    completed: false,
                };
                state.todos.push(todo).ok();
                state.next_id += 1;
                TodoEffectWithSave::SaveRequired // Persist new todos
            }
            TodoAction::Toggle(id) => {
                if let Some(todo) = state.todos.iter_mut().find(|t| t.id == id) {
                    todo.completed = !todo.completed;
                    TodoEffectWithSave::SaveRequired // Persist toggle state
                } else {
                    TodoEffectWithSave::Unchanged
                }
            }
            TodoAction::Delete(id) => {
                let original_len = state.todos.len();
                state.todos.retain(|t| t.id != id);
                if state.todos.len() != original_len {
                    TodoEffectWithSave::SaveRequired // Persist deletion
                } else {
                    TodoEffectWithSave::Unchanged
                }
            }
            TodoAction::SetFilter(filter) => {
                if filter == state.filter {
                    TodoEffectWithSave::Unchanged
                } else {
                    state.filter = filter;
                    TodoEffectWithSave::None // Filter is UI-only, no save needed
                }
            }
            TodoAction::ClearCompleted => {
                let original_len = state.todos.len();
                state.todos.retain(|t| !t.completed);
                if state.todos.len() != original_len {
                    TodoEffectWithSave::SaveRequired
                } else {
                    TodoEffectWithSave::Unchanged
                }
            }
        }
    }
}

#[test]
fn todo_app_tracks_save_effects() {
    let mut app: App<TodoReducerWithSave, TodoView> =
        App::new(TodoView::new(), TodoState::default());

    let mut save_count = 0;

    // Add a task - should require save
    let mut text = heapless::String::<32>::new();
    text.push_str("Important task").ok();
    let effect = app.dispatch(TodoAction::Add(text));
    if matches!(effect, TodoEffectWithSave::SaveRequired) {
        save_count += 1;
    }

    // Toggle - should require save
    let effect = app.dispatch(TodoAction::Toggle(0));
    if matches!(effect, TodoEffectWithSave::SaveRequired) {
        save_count += 1;
    }

    // Change filter - should NOT require save
    let effect = app.dispatch(TodoAction::SetFilter(Filter::Active));
    if matches!(effect, TodoEffectWithSave::SaveRequired) {
        save_count += 1;
    }

    assert_eq!(save_count, 2); // Only Add and Toggle required save
}

// ============================================================================
// View composition with view! macro
// ============================================================================

// Sub-view components - each has fn render<D: Write>(display: &mut D, state: &State)

struct Header;
impl Header {
    fn render<D: Write>(display: &mut D, _state: &TodoState) {
        writeln!(display, "=== TODO APP ===").ok();
    }
}

struct FilterStatus;
impl FilterStatus {
    fn render<D: Write>(display: &mut D, state: &TodoState) {
        writeln!(display, "Filter: {:?}", state.filter).ok();
        writeln!(display, "----------------").ok();
    }
}

struct EmptyMessage;
impl EmptyMessage {
    fn render<D: Write>(display: &mut D, _state: &TodoState) {
        writeln!(display, "(no items)").ok();
    }
}

struct TodoList;
impl TodoList {
    fn render<D: Write>(display: &mut D, state: &TodoState) {
        let filtered: heapless::Vec<&Todo, 8> = state
            .todos
            .iter()
            .filter(|t| match state.filter {
                Filter::All => true,
                Filter::Active => !t.completed,
                Filter::Completed => t.completed,
            })
            .collect();

        for todo in &filtered {
            let mark = if todo.completed { "x" } else { " " };
            writeln!(display, "[{}] {} (id:{})", mark, todo.text, todo.id).ok();
        }
    }
}

struct Footer;
impl Footer {
    fn render<D: Write>(display: &mut D, state: &TodoState) {
        let active_count = state.todos.iter().filter(|t| !t.completed).count();
        let completed_count = state.todos.iter().filter(|t| t.completed).count();
        writeln!(display, "----------------").ok();
        writeln!(display, "{} active, {} completed", active_count, completed_count).ok();
    }
}

// Composed view using the view! macro
view! {
    TodoBody<D: Write> for TodoState {
        <Header />
        <FilterStatus />
        @if state.todos.iter().filter(|t| match state.filter {
            Filter::All => true,
            Filter::Active => !t.completed,
            Filter::Completed => t.completed,
        }).count() == 0 {
            <EmptyMessage />
        } @else {
            <TodoList />
        }
        <Footer />
    }
}

// Root view that wraps the composed view with setup/teardown
struct ComposedTodoView {
    inner: TodoBody<heapless::String<512>>,
}

impl ComposedTodoView {
    fn new() -> Self {
        Self {
            inner: TodoBody::new(heapless::String::new()),
        }
    }

    fn as_str(&self) -> &str {
        self.inner.display().as_str()
    }
}

impl View for ComposedTodoView {
    type State = TodoState;

    fn render(&mut self, state: &Self::State) {
        // Setup: clear the display
        self.inner.display_mut().clear();
        // Render composed view
        self.inner.render(state);
        // Teardown would go here (e.g., flush)
    }
}

#[test]
fn todo_app_with_composed_view() {
    let mut app: App<TodoReducer, ComposedTodoView> =
        App::new(ComposedTodoView::new(), TodoState::default());

    // Add some todos
    let mut text = heapless::String::<32>::new();
    text.push_str("Buy milk").ok();
    app.dispatch(TodoAction::Add(text));

    let mut text = heapless::String::<32>::new();
    text.push_str("Write code").ok();
    app.dispatch(TodoAction::Add(text));

    // Verify rendered output
    assert!(app.view().as_str().contains("=== TODO APP ==="));
    assert!(app.view().as_str().contains("[ ] Buy milk"));
    assert!(app.view().as_str().contains("[ ] Write code"));
    assert!(app.view().as_str().contains("2 active, 0 completed"));

    // Toggle one
    app.dispatch(TodoAction::Toggle(0));
    assert!(app.view().as_str().contains("[x] Buy milk"));
    assert!(app.view().as_str().contains("1 active, 1 completed"));

    // Filter to completed
    app.dispatch(TodoAction::SetFilter(Filter::Completed));
    assert!(app.view().as_str().contains("[x] Buy milk"));
    assert!(!app.view().as_str().contains("Write code")); // filtered out

    // Empty state test - clear all and check empty message
    app.dispatch(TodoAction::ClearCompleted);
    app.dispatch(TodoAction::SetFilter(Filter::Completed));
    assert!(app.view().as_str().contains("(no items)"));

    println!("Composed view output:\n{}", app.view().as_str());
}
