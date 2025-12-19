//! Todo App - integration test for reducto framework
//!
//! Demonstrates: State, Actions, Reducer, TextView rendering, Application trait

use core::fmt::Write;
use reducto::{Application, Store, TextView, View, changed, process_iteration, unchanged};

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
// Reducer (using macro with complex logic)
// ============================================================================

reducto::reducer! {
    TodoReducer for TodoState, TodoAction {
        TodoAction::Add(text) => |state| {
            if text.is_empty() {
                return unchanged(state);
            }
            let todo = Todo {
                id: state.next_id,
                text,
                completed: false,
            };
            state.todos.push(todo).ok();
            state.next_id += 1;
            changed(state)
        },
        TodoAction::Toggle(id) => |state| {
            if let Some(todo) = state.todos.iter_mut().find(|t| t.id == id) {
                todo.completed = !todo.completed;
                changed(state)
            } else {
                unchanged(state)
            }
        },
        TodoAction::Delete(id) => |state| {
            let original_len = state.todos.len();
            state.todos.retain(|t| t.id != id);
            if state.todos.len() != original_len {
                changed(state)
            } else {
                unchanged(state)
            }
        },
        TodoAction::SetFilter(filter) => |state| {
            if filter == state.filter {
                unchanged(state)
            } else {
                changed(TodoState { filter, ..state })
            }
        },
        TodoAction::ClearCompleted => |state| {
            let original_len = state.todos.len();
            state.todos.retain(|t| !t.completed);
            if state.todos.len() != original_len {
                changed(state)
            } else {
                unchanged(state)
            }
        },
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
        Self { buffer: TextView::new() }
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
                        action: TodoAction| -> bool {
        let did_change = store.dispatch::<TodoReducer>(action);
        if did_change {
            view.render(store.state());
            render_count += 1;
        }
        did_change
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
    assert!(dispatch(&mut store, &mut view, TodoAction::SetFilter(Filter::Active)));
    assert!(view.text().contains("Filter: Active"));
    assert!(!view.text().contains("Write code")); // completed, shouldn't show
    assert!(view.text().contains("Buy milk"));

    // Filter to completed only
    assert!(dispatch(&mut store, &mut view, TodoAction::SetFilter(Filter::Completed)));
    assert!(view.text().contains("[x] Write code"));
    assert!(!view.text().contains("Buy milk")); // active, shouldn't show

    // Back to all
    assert!(dispatch(&mut store, &mut view, TodoAction::SetFilter(Filter::All)));
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
    assert!(!dispatch(&mut store, &mut view, TodoAction::SetFilter(Filter::All))); // already set
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

    // Process queue manually (like run_loop does)
    let mut changes = 0;
    while let Some(action) = store.pop_action() {
        if store.dispatch::<TodoReducer>(action) {
            view.render(store.state());
            changes += 1;
        }
    }

    assert_eq!(changes, 3);
    assert!(view.text().contains("[x] Task 1"));
    assert!(view.text().contains("[ ] Task 2"));
}

// ============================================================================
// Application trait integration test
// ============================================================================

struct TodoApp {
    view: TodoView,
    tick_count: usize,
}

impl TodoApp {
    fn new() -> Self {
        Self {
            view: TodoView::new(),
            tick_count: 0,
        }
    }
}

impl Application for TodoApp {
    type State = TodoState;
    type Action = TodoAction;
    type Reducer = TodoReducer;
    type View = TodoView;

    fn view(&mut self) -> &mut Self::View {
        &mut self.view
    }

    fn tick(&mut self) {
        self.tick_count += 1;
    }
}

#[test]
fn todo_app_with_application_trait() {
    let mut app = TodoApp::new();
    let mut store: Store<TodoState, TodoAction, 16> = Store::new(TodoState::default());

    // Initial render
    app.view().render(store.state());
    assert!(app.view().text().contains("(no items)"));

    // Enqueue actions (simulating button presses / ISR events)
    let mut text = heapless::String::<32>::new();
    text.push_str("Learn Rust").ok();
    store.enqueue(TodoAction::Add(text)).ok();

    let mut text = heapless::String::<32>::new();
    text.push_str("Build firmware").ok();
    store.enqueue(TodoAction::Add(text)).ok();

    store.enqueue(TodoAction::Toggle(0)).ok();

    // Run one iteration using the framework's process_iteration
    let renders = process_iteration(&mut app, &mut store);

    // Verify tick was called and correct number of renders
    assert_eq!(app.tick_count, 1);
    assert_eq!(renders, 3); // 3 actions, all changed state

    // Verify final state via view
    assert!(app.view().text().contains("[x] Learn Rust"));
    assert!(app.view().text().contains("[ ] Build firmware"));
    assert!(app.view().text().contains("1 active, 1 completed"));

    // Run another iteration with no actions - tick still called, no render
    let renders = process_iteration(&mut app, &mut store);
    assert_eq!(app.tick_count, 2);
    assert_eq!(renders, 0); // No actions queued
}
