//! Integration tests for reducto
//!
//! TDD: Write failing tests first (RED), then implement to pass (GREEN)

use core::fmt::Write;
use reducto::{Application, Reducer, Store, TextView, View};

// Test state and action types
#[derive(Clone, PartialEq, Debug, Default)]
struct TestState {
    count: i32,
}

#[derive(Clone, Debug)]
enum TestAction {
    Increment,
    Decrement,
    Set(i32),
}

// Test reducer - must handle all action variants (exhaustive match)
struct TestReducer;

impl Reducer for TestReducer {
    type State = TestState;
    type Action = TestAction;

    fn reduce(state: Self::State, action: Self::Action) -> Self::State {
        match action {
            TestAction::Increment => TestState { count: state.count + 1 },
            TestAction::Decrement => TestState { count: state.count - 1 },
            TestAction::Set(val) => TestState { count: val },
        }
    }
}

#[test]
fn store_new_creates_with_initial_state() {
    let store: Store<TestState, TestAction, 8> = Store::new(TestState { count: 42 });
    assert_eq!(store.state().count, 42);
}

#[test]
fn store_dispatch_updates_state() {
    let mut store: Store<TestState, TestAction, 8> = Store::new(TestState::default());

    let changed = store.dispatch::<TestReducer>(TestAction::Increment);

    assert!(changed);
    assert_eq!(store.state().count, 1);
}

#[test]
fn store_dispatch_returns_false_when_state_unchanged() {
    let mut store: Store<TestState, TestAction, 8> = Store::new(TestState { count: 5 });

    // Setting to same value should return false
    let changed = store.dispatch::<TestReducer>(TestAction::Set(5));

    assert!(!changed);
    assert_eq!(store.state().count, 5);
}

#[test]
fn store_enqueue_adds_action_to_queue() {
    let mut store: Store<TestState, TestAction, 8> = Store::new(TestState::default());

    assert!(store.enqueue(TestAction::Increment).is_ok());
    assert!(store.enqueue(TestAction::Increment).is_ok());

    // Queue should have 2 actions
    assert!(!store.is_queue_empty());
}

#[test]
fn store_enqueue_fails_when_queue_full() {
    let mut store: Store<TestState, TestAction, 2> = Store::new(TestState::default());

    assert!(store.enqueue(TestAction::Increment).is_ok());
    assert!(store.enqueue(TestAction::Increment).is_ok());
    // Queue is now full (capacity 2)
    assert!(store.enqueue(TestAction::Increment).is_err());
}

#[test]
fn store_process_queue_dispatches_all_actions() {
    let mut store: Store<TestState, TestAction, 8> = Store::new(TestState::default());

    store.enqueue(TestAction::Increment).unwrap();
    store.enqueue(TestAction::Increment).unwrap();
    store.enqueue(TestAction::Set(10)).unwrap();

    let changes = store.process_queue::<TestReducer>();

    assert_eq!(changes, 3);  // 3 state changes
    assert_eq!(store.state().count, 10);
    assert!(store.is_queue_empty());
}

#[test]
fn reducer_handles_all_variants() {
    let state = TestState { count: 10 };

    let inc = TestReducer::reduce(state.clone(), TestAction::Increment);
    assert_eq!(inc.count, 11);

    let dec = TestReducer::reduce(state.clone(), TestAction::Decrement);
    assert_eq!(dec.count, 9);

    let set = TestReducer::reduce(state, TestAction::Set(100));
    assert_eq!(set.count, 100);
}

// ============================================================================
// View trait tests
// ============================================================================

// A test view that renders state to a text buffer
struct CounterView;

impl View for CounterView {
    type State = TestState;

    fn render(&mut self, view: &mut TextView<128>, state: &Self::State) {
        view.clear();
        write!(view.buffer_mut(), "Count: {}", state.count).ok();
    }
}

#[test]
fn text_view_can_be_created() {
    let view = TextView::<128>::new();
    assert!(view.as_str().is_empty());
}

#[test]
fn text_view_renders_state() {
    let state = TestState { count: 42 };
    let mut text_view = TextView::<128>::new();
    let mut counter_view = CounterView;

    counter_view.render(&mut text_view, &state);

    assert_eq!(text_view.as_str(), "Count: 42");
}

#[test]
fn text_view_contains_works() {
    let state = TestState { count: 99 };
    let mut text_view = TextView::<128>::new();
    let mut counter_view = CounterView;

    counter_view.render(&mut text_view, &state);

    assert!(text_view.contains("Count:"));
    assert!(text_view.contains("99"));
    assert!(!text_view.contains("42"));
}

#[test]
fn text_view_clear_works() {
    let mut text_view = TextView::<128>::new();
    write!(text_view.buffer_mut(), "some text").ok();
    assert!(!text_view.as_str().is_empty());

    text_view.clear();
    assert!(text_view.as_str().is_empty());
}

// ============================================================================
// Application trait tests
// ============================================================================

/// Test application that records interactions for verification
struct TestApp {
    /// Record of (old_state, new_state) passed to on_state_change
    state_changes: Vec<(TestState, TestState)>,
    /// Number of times tick was called
    tick_count: usize,
}

impl TestApp {
    fn new() -> Self {
        Self {
            state_changes: Vec::new(),
            tick_count: 0,
        }
    }
}

impl Application for TestApp {
    type State = TestState;
    type Action = TestAction;
    type Reducer = TestReducer;

    fn on_state_change(&mut self, old: &TestState, new: &TestState) {
        self.state_changes.push((old.clone(), new.clone()));
    }

    fn tick(&mut self) {
        self.tick_count += 1;
    }
}

#[test]
fn application_on_state_change_called_when_state_changes() {
    let mut app = TestApp::new();
    let mut store: Store<TestState, TestAction, 8> = Store::new(TestState::default());

    // Enqueue action (simulating ISR)
    store.enqueue(TestAction::Increment).unwrap();

    // Simulate one iteration of run_loop
    app.tick();
    store.process_queue_with_callback::<TestReducer, _>(|old, new| {
        app.on_state_change(old, new);
    });

    assert_eq!(app.state_changes.len(), 1);
    assert_eq!(app.state_changes[0].0.count, 0);  // old state
    assert_eq!(app.state_changes[0].1.count, 1);  // new state
}

#[test]
fn application_on_state_change_not_called_when_state_unchanged() {
    let mut app = TestApp::new();
    let mut store: Store<TestState, TestAction, 8> = Store::new(TestState { count: 0 });

    // Enqueue action that sets to same value
    store.enqueue(TestAction::Set(0)).unwrap();

    // Simulate one iteration
    app.tick();
    store.process_queue_with_callback::<TestReducer, _>(|old, new| {
        app.on_state_change(old, new);
    });

    // State didn't change, so on_state_change should NOT have been called
    assert_eq!(app.state_changes.len(), 0);
}

#[test]
fn application_processes_multiple_queued_actions() {
    let mut app = TestApp::new();
    let mut store: Store<TestState, TestAction, 8> = Store::new(TestState::default());

    // Enqueue multiple actions (simulating multiple ISR events)
    store.enqueue(TestAction::Increment).unwrap();
    store.enqueue(TestAction::Increment).unwrap();
    store.enqueue(TestAction::Set(100)).unwrap();

    // Single iteration processes all queued actions
    app.tick();
    store.process_queue_with_callback::<TestReducer, _>(|old, new| {
        app.on_state_change(old, new);
    });

    assert_eq!(app.tick_count, 1);
    assert_eq!(app.state_changes.len(), 3);  // 3 state changes
    assert_eq!(store.state().count, 100);  // Final state
}

// ============================================================================
// reducer! macro tests
// ============================================================================

// State for macro-generated reducer
#[derive(Clone, PartialEq, Debug, Default)]
struct MacroState {
    value: i32,
    name: &'static str,
}

// Actions for macro-generated reducer
#[derive(Clone, Debug)]
enum MacroAction {
    Add(i32),
    Subtract(i32),
    SetName(&'static str),
    Reset,
}

// Generate reducer using macro
reducto::reducer! {
    MacroReducer for MacroState, MacroAction {
        MacroAction::Add(n) => |state| MacroState { value: state.value + n, ..state },
        MacroAction::Subtract(n) => |state| MacroState { value: state.value - n, ..state },
        MacroAction::SetName(s) => |state| MacroState { name: s, ..state },
        MacroAction::Reset => |_state| MacroState::default(),
    }
}

#[test]
fn macro_reducer_handles_all_variants() {
    let state = MacroState { value: 10, name: "test" };

    let added = MacroReducer::reduce(state.clone(), MacroAction::Add(5));
    assert_eq!(added.value, 15);
    assert_eq!(added.name, "test");

    let subtracted = MacroReducer::reduce(state.clone(), MacroAction::Subtract(3));
    assert_eq!(subtracted.value, 7);

    let named = MacroReducer::reduce(state.clone(), MacroAction::SetName("new"));
    assert_eq!(named.name, "new");
    assert_eq!(named.value, 10);

    let reset = MacroReducer::reduce(state, MacroAction::Reset);
    assert_eq!(reset.value, 0);
    assert_eq!(reset.name, "");
}

#[test]
fn macro_reducer_works_with_store() {
    let mut store: Store<MacroState, MacroAction, 8> = Store::new(MacroState::default());

    let changed = store.dispatch::<MacroReducer>(MacroAction::Add(42));
    assert!(changed);
    assert_eq!(store.state().value, 42);

    let changed = store.dispatch::<MacroReducer>(MacroAction::SetName("hello"));
    assert!(changed);
    assert_eq!(store.state().name, "hello");
}
