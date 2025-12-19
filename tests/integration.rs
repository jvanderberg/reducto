//! Integration tests for reducto
//!
//! TDD: Write failing tests first (RED), then implement to pass (GREEN)

use core::fmt::Write;
use reducto::{Application, Outcome, Reducer, Store, TextView, View, changed, unchanged};

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

    fn reduce(mut state: Self::State, action: Self::Action) -> Outcome<Self::State> {
        match action {
            TestAction::Increment => {
                state.count += 1;
                Outcome::changed(state)
            }
            TestAction::Decrement => {
                state.count -= 1;
                Outcome::changed(state)
            }
            TestAction::Set(val) if val == state.count => Outcome::Unchanged(state),
            TestAction::Set(val) => {
                state.count = val;
                Outcome::changed(state)
            }
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

    let (inc, changed) = TestReducer::reduce(state.clone(), TestAction::Increment).into_parts();
    assert_eq!(inc.count, 11);
    assert!(changed);

    let (dec, changed) = TestReducer::reduce(state.clone(), TestAction::Decrement).into_parts();
    assert_eq!(dec.count, 9);
    assert!(changed);

    let (set, changed) = TestReducer::reduce(state, TestAction::Set(100)).into_parts();
    assert_eq!(set.count, 100);
    assert!(changed);
}

// ============================================================================
// View trait tests
// ============================================================================

// A test view that renders state to a text buffer
struct CounterView {
    buffer: TextView<128>,
}

impl CounterView {
    fn new() -> Self {
        Self { buffer: TextView::new() }
    }
}

impl View for CounterView {
    type State = TestState;

    fn render(&mut self, state: &Self::State) {
        self.buffer.clear();
        write!(self.buffer.buffer_mut(), "Count: {}", state.count).ok();
    }

    fn text(&self) -> &str {
        self.buffer.as_str()
    }
}

#[test]
fn text_view_can_be_created() {
    let view = TextView::<128>::new();
    assert!(view.as_str().is_empty());
}

#[test]
fn view_renders_state() {
    let state = TestState { count: 42 };
    let mut view = CounterView::new();

    view.render(&state);

    assert_eq!(view.text(), "Count: 42");
}

#[test]
fn view_text_contains_works() {
    let state = TestState { count: 99 };
    let mut view = CounterView::new();

    view.render(&state);

    assert!(view.text().contains("Count:"));
    assert!(view.text().contains("99"));
    assert!(!view.text().contains("42"));
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

/// Test view that renders count to text buffer
struct TestAppView {
    buffer: TextView<128>,
    render_count: usize,
}

impl TestAppView {
    fn new() -> Self {
        Self {
            buffer: TextView::new(),
            render_count: 0,
        }
    }
}

impl View for TestAppView {
    type State = TestState;

    fn render(&mut self, state: &Self::State) {
        self.buffer.clear();
        write!(self.buffer.buffer_mut(), "Count: {}", state.count).ok();
        self.render_count += 1;
    }

    fn text(&self) -> &str {
        self.buffer.as_str()
    }
}

/// Test application that records interactions for verification
struct TestApp {
    view: TestAppView,
    /// Number of times tick was called
    tick_count: usize,
}

impl TestApp {
    fn new() -> Self {
        Self {
            view: TestAppView::new(),
            tick_count: 0,
        }
    }
}

impl Application for TestApp {
    type State = TestState;
    type Action = TestAction;
    type Reducer = TestReducer;
    type View = TestAppView;

    fn view(&mut self) -> &mut Self::View {
        &mut self.view
    }

    fn tick(&mut self) {
        self.tick_count += 1;
    }
}

#[test]
fn application_renders_when_state_changes() {
    let mut app = TestApp::new();
    let mut store: Store<TestState, TestAction, 8> = Store::new(TestState::default());

    // Enqueue action (simulating ISR)
    store.enqueue(TestAction::Increment).unwrap();

    // Simulate one iteration of run_loop
    app.tick();
    while let Some(action) = store.pop_action() {
        if store.dispatch::<TestReducer>(action) {
            app.view().render(store.state());
        }
    }

    assert_eq!(app.view.render_count, 1);
    assert!(app.view().text().contains("Count: 1"));
}

#[test]
fn application_does_not_render_when_state_unchanged() {
    let mut app = TestApp::new();
    let mut store: Store<TestState, TestAction, 8> = Store::new(TestState::default());

    // Enqueue action that sets to same value (state won't change)
    store.enqueue(TestAction::Set(0)).unwrap();

    // Simulate one iteration
    app.tick();
    while let Some(action) = store.pop_action() {
        if store.dispatch::<TestReducer>(action) {
            app.view().render(store.state());
        }
    }

    // State didn't change, so render should NOT have been called
    assert_eq!(app.view.render_count, 0);
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
    while let Some(action) = store.pop_action() {
        if store.dispatch::<TestReducer>(action) {
            app.view().render(store.state());
        }
    }

    assert_eq!(app.tick_count, 1);
    assert_eq!(app.view.render_count, 3);  // 3 state changes = 3 renders
    assert_eq!(store.state().count, 100);  // Final state
    assert!(app.view().text().contains("Count: 100"));  // Final render
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

    let (added, _) = MacroReducer::reduce(state.clone(), MacroAction::Add(5)).into_parts();
    assert_eq!(added.value, 15);
    assert_eq!(added.name, "test");

    let (subtracted, _) = MacroReducer::reduce(state.clone(), MacroAction::Subtract(3)).into_parts();
    assert_eq!(subtracted.value, 7);

    let (named, _) = MacroReducer::reduce(state.clone(), MacroAction::SetName("new")).into_parts();
    assert_eq!(named.name, "new");
    assert_eq!(named.value, 10);

    let (reset, _) = MacroReducer::reduce(state, MacroAction::Reset).into_parts();
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

// ============================================================================
// Macro with unchanged() DSL tests
// ============================================================================

#[derive(Clone, PartialEq, Debug, Default)]
struct DslState {
    count: i32,
}

#[derive(Clone, Debug)]
enum DslAction {
    Increment,
    SetCount(i32),
}

// Using unchanged() function for no-op arms
reducto::reducer! {
    DslReducer for DslState, DslAction {
        DslAction::Increment => |state| DslState { count: state.count + 1 },
        // Always returns unchanged for this test
        DslAction::SetCount(_n) => |state| unchanged(state),
    }
}

// Demonstrates if/else with changed()/unchanged() - both branches have same type
#[derive(Clone, PartialEq, Debug, Default)]
struct ConditionalState {
    value: i32,
}

#[derive(Clone, Debug)]
enum ConditionalAction {
    SetValue(i32),
}

reducto::reducer! {
    ConditionalReducer for ConditionalState, ConditionalAction {
        ConditionalAction::SetValue(n) => |state| {
            if n == state.value {
                unchanged(state)
            } else {
                changed(ConditionalState { value: n })
            }
        },
    }
}

#[test]
fn macro_unchanged_dsl_skips_callback() {
    let mut store: Store<DslState, DslAction, 8> = Store::new(DslState { count: 5 });

    // SetCount uses unchanged() - returns false, state untouched
    let changed = store.dispatch::<DslReducer>(DslAction::SetCount(99));
    assert!(!changed);
    assert_eq!(store.state().count, 5);

    // Increment is normal - returns true, state updated
    let changed = store.dispatch::<DslReducer>(DslAction::Increment);
    assert!(changed);
    assert_eq!(store.state().count, 6);
}

#[test]
fn conditional_reducer_if_else_works() {
    let mut store: Store<ConditionalState, ConditionalAction, 8> =
        Store::new(ConditionalState { value: 10 });

    // Setting to same value - unchanged() branch
    let did_change = store.dispatch::<ConditionalReducer>(ConditionalAction::SetValue(10));
    assert!(!did_change);
    assert_eq!(store.state().value, 10);

    // Setting to different value - changed() branch
    let did_change = store.dispatch::<ConditionalReducer>(ConditionalAction::SetValue(42));
    assert!(did_change);
    assert_eq!(store.state().value, 42);
}

// ============================================================================
// App double-buffering tests
// ============================================================================

use reducto::App;

#[test]
fn app_dispatch_returns_old_and_new_state() {
    let view = CounterView::new();
    let mut app: App<TestState, TestAction, TestReducer, _> = App::new(view, TestState { count: 5 });

    let result = app.dispatch(TestAction::Increment);

    assert!(result.changed);
    assert_eq!(result.old.count, 5);
    assert_eq!(result.new.count, 6);
    assert_eq!(app.state().count, 6);
}

#[test]
fn app_dispatch_unchanged_preserves_state() {
    let view = CounterView::new();
    let mut app: App<TestState, TestAction, TestReducer, _> = App::new(view, TestState { count: 10 });

    // Setting to same value returns unchanged
    let result = app.dispatch(TestAction::Set(10));

    assert!(!result.changed);
    assert_eq!(result.old.count, 10);
    assert_eq!(result.new.count, 10);
}

#[test]
fn app_dispatch_double_buffer_alternates() {
    let view = CounterView::new();
    let mut app: App<TestState, TestAction, TestReducer, _> = App::new(view, TestState { count: 0 });

    // First dispatch: old=0, new=1
    let r1 = app.dispatch(TestAction::Increment);
    assert_eq!(r1.old.count, 0);
    assert_eq!(r1.new.count, 1);

    // Second dispatch: old=1, new=2
    let r2 = app.dispatch(TestAction::Increment);
    assert_eq!(r2.old.count, 1);
    assert_eq!(r2.new.count, 2);

    // Third dispatch: old=2, new=3
    let r3 = app.dispatch(TestAction::Increment);
    assert_eq!(r3.old.count, 2);
    assert_eq!(r3.new.count, 3);

    assert_eq!(app.state().count, 3);
}
