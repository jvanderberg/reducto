//! Todo App - integration test for reducto framework
//!
//! Demonstrates: State, Actions, Reducer, TextView rendering, Effect-based side effects

use core::fmt::Write;
use reducto::{App, Effect, Reducer, Store, TextView, View};

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

    fn text(&self) -> &str {
        self.buffer.as_str()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[test]
fn todo_app_full_workflow() {
    let mut store: Store<TodoState, TodoAction, 16> = Store::new(TodoState::default());
    let mut view = TodoView::new();
    let mut render_count = 0;

    // Helper to dispatch and render if changed
    let mut dispatch = |store: &mut Store<TodoState, TodoAction, 16>,
                        view: &mut TodoView,
                        action: TodoAction|
     -> bool {
        let effect = store.dispatch::<TodoReducer>(action);
        let changed = !effect.is_unchanged();
        if changed {
            view.render(store.state());
            render_count += 1;
        }
        changed
    };

    // Initial render
    view.render(store.state());
    assert!(view.text().contains("(no items)"));

    // Add some todos
    let mut text = heapless::String::<32>::new();
    text.push_str("Buy milk").ok();
    assert!(dispatch(&mut store, &mut view, TodoAction::Add(text)));
    assert!(view.text().contains("[ ] Buy milk"));

    let mut text = heapless::String::<32>::new();
    text.push_str("Write code").ok();
    assert!(dispatch(&mut store, &mut view, TodoAction::Add(text)));

    let mut text = heapless::String::<32>::new();
    text.push_str("Test reducto").ok();
    assert!(dispatch(&mut store, &mut view, TodoAction::Add(text)));

    assert_eq!(store.state().todos.len(), 3);
    assert!(view.text().contains("3 active, 0 completed"));

    // Toggle one complete
    assert!(dispatch(&mut store, &mut view, TodoAction::Toggle(1)));
    assert!(view.text().contains("[x] Write code"));
    assert!(view.text().contains("2 active, 1 completed"));

    // Filter to active only
    assert!(dispatch(
        &mut store,
        &mut view,
        TodoAction::SetFilter(Filter::Active)
    ));
    assert!(view.text().contains("Filter: Active"));
    assert!(!view.text().contains("Write code")); // completed, shouldn't show
    assert!(view.text().contains("Buy milk"));

    // Filter to completed only
    assert!(dispatch(
        &mut store,
        &mut view,
        TodoAction::SetFilter(Filter::Completed)
    ));
    assert!(view.text().contains("[x] Write code"));
    assert!(!view.text().contains("Buy milk")); // active, shouldn't show

    // Back to all
    assert!(dispatch(
        &mut store,
        &mut view,
        TodoAction::SetFilter(Filter::All)
    ));
    assert!(view.text().contains("Buy milk"));
    assert!(view.text().contains("Write code"));

    // Delete a todo
    assert!(dispatch(&mut store, &mut view, TodoAction::Delete(0)));
    assert!(!view.text().contains("Buy milk"));
    assert_eq!(store.state().todos.len(), 2);

    // Clear completed
    assert!(dispatch(&mut store, &mut view, TodoAction::ClearCompleted));
    assert!(!view.text().contains("Write code"));
    assert_eq!(store.state().todos.len(), 1);
    assert!(view.text().contains("Test reducto"));

    // No-op actions shouldn't trigger render
    let empty = heapless::String::<32>::new();
    assert!(!dispatch(&mut store, &mut view, TodoAction::Add(empty)));

    assert!(!dispatch(&mut store, &mut view, TodoAction::Toggle(999))); // non-existent
    assert!(!dispatch(&mut store, &mut view, TodoAction::Delete(999)));
    assert!(!dispatch(
        &mut store,
        &mut view,
        TodoAction::SetFilter(Filter::All)
    )); // already set
    assert!(!dispatch(&mut store, &mut view, TodoAction::ClearCompleted)); // none completed

    println!("Final view:\n{}", view.text());
    println!("Total renders: {}", render_count);
}

#[test]
fn todo_app_with_queue() {
    let mut store: Store<TodoState, TodoAction, 16> = Store::new(TodoState::default());
    let mut view = TodoView::new();

    // Simulate ISR-style: enqueue multiple actions
    let mut t1 = heapless::String::<32>::new();
    t1.push_str("Task 1").ok();
    let mut t2 = heapless::String::<32>::new();
    t2.push_str("Task 2").ok();

    store.enqueue(TodoAction::Add(t1)).ok();
    store.enqueue(TodoAction::Add(t2)).ok();
    store.enqueue(TodoAction::Toggle(0)).ok();

    // Process queue manually
    let mut changes = 0;
    while let Some(action) = store.pop_action() {
        let effect = store.dispatch::<TodoReducer>(action);
        if !effect.is_unchanged() {
            view.render(store.state());
            changes += 1;
        }
    }

    assert_eq!(changes, 3);
    assert!(view.text().contains("[x] Task 1"));
    assert!(view.text().contains("[ ] Task 2"));
}

// ============================================================================
// App integration test
// ============================================================================

#[test]
fn todo_app_with_static_app() {
    let mut app: App<TodoState, TodoAction, TodoReducer, TodoView> =
        App::new(TodoView::new(), TodoState::default());

    // App renders on dispatch, so initially the view is empty
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
    assert!(app.view().text().contains("[x] Learn Rust"));
    assert!(app.view().text().contains("[ ] Build firmware"));
    assert!(app.view().text().contains("1 active, 1 completed"));
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
    let mut app: App<TodoState, TodoAction, TodoReducerWithSave, TodoView> =
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
