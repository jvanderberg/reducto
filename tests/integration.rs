//! Integration tests for reducto
//!
//! TDD: Write failing tests first (RED), then implement to pass (GREEN)

use core::fmt::Write;
use reducto::{App, Effect, Reducer, TextView, View};

// ============================================================================
// Common Effect type for tests
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq)]
enum TestEffect {
    None,
    Unchanged,
}

impl Effect for TestEffect {
    fn is_unchanged(&self) -> bool {
        matches!(self, TestEffect::Unchanged)
    }
    fn changed() -> Self {
        TestEffect::None
    }
}

// Test state and action types
#[derive(Debug, Default)]
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
    type Effect = TestEffect;

    fn reduce(state: &mut Self::State, action: Self::Action) -> Self::Effect {
        match action {
            TestAction::Increment => {
                state.count += 1;
                TestEffect::None
            }
            TestAction::Decrement => {
                state.count -= 1;
                TestEffect::None
            }
            TestAction::Set(val) if val == state.count => TestEffect::Unchanged,
            TestAction::Set(val) => {
                state.count = val;
                TestEffect::None
            }
        }
    }
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
        Self {
            buffer: TextView::new(),
        }
    }
}

impl View for CounterView {
    type State = TestState;

    fn render(&mut self, state: &Self::State) {
        self.buffer.clear();
        write!(self.buffer.buffer_mut(), "Count: {}", state.count).ok();
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

    assert_eq!(view.buffer.as_str(), "Count: 42");
}

#[test]
fn view_text_contains_works() {
    let state = TestState { count: 99 };
    let mut view = CounterView::new();

    view.render(&state);

    assert!(view.buffer.contains("Count:"));
    assert!(view.buffer.contains("99"));
    assert!(!view.buffer.contains("42"));
}

#[test]
fn text_view_clear_works() {
    let mut text_view = TextView::<128>::new();
    write!(text_view.buffer_mut(), "some text").ok();
    assert!(!text_view.as_str().is_empty());

    text_view.clear();
    assert!(text_view.as_str().is_empty());
}

#[test]
fn reducer_handles_all_variants() {
    let mut state = TestState { count: 10 };

    let effect = TestReducer::reduce(&mut state, TestAction::Increment);
    assert_eq!(state.count, 11);
    assert!(!effect.is_unchanged());

    let effect = TestReducer::reduce(&mut state, TestAction::Decrement);
    assert_eq!(state.count, 10);
    assert!(!effect.is_unchanged());

    let effect = TestReducer::reduce(&mut state, TestAction::Set(100));
    assert_eq!(state.count, 100);
    assert!(!effect.is_unchanged());
}

// ============================================================================
// App tests
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
}

#[test]
fn app_new_creates_with_initial_state() {
    let app: App<TestReducer, CounterView> =
        App::new(CounterView::new(), TestState { count: 42 });
    assert_eq!(app.state().count, 42);
}

#[test]
fn app_dispatch_updates_state() {
    let mut app: App<TestReducer, CounterView> =
        App::new(CounterView::new(), TestState::default());

    let effect = app.dispatch(TestAction::Increment);

    assert_eq!(effect, TestEffect::None);
    assert_eq!(app.state().count, 1);
}

#[test]
fn app_dispatch_returns_unchanged_when_state_unchanged() {
    let mut app: App<TestReducer, CounterView> =
        App::new(CounterView::new(), TestState { count: 5 });

    // Setting to same value should return Unchanged
    let effect = app.dispatch(TestAction::Set(5));

    assert_eq!(effect, TestEffect::Unchanged);
    assert_eq!(app.state().count, 5);
}

#[test]
fn app_dispatch_renders_and_returns_effect() {
    let view = TestAppView::new();
    let mut app: App<TestReducer, _> =
        App::new(view, TestState { count: 5 });

    let effect = app.dispatch(TestAction::Increment);

    assert_eq!(effect, TestEffect::None);
    assert_eq!(app.state().count, 6);
    assert_eq!(app.view().render_count, 1);
    assert!(app.view().buffer.contains("Count: 6"));
}

#[test]
fn app_unchanged_skips_render() {
    let view = TestAppView::new();
    let mut app: App<TestReducer, _> =
        App::new(view, TestState { count: 10 });

    // Setting to same value returns unchanged - should NOT render
    let effect = app.dispatch(TestAction::Set(10));

    assert_eq!(effect, TestEffect::Unchanged);
    assert_eq!(app.state().count, 10);
    assert_eq!(app.view().render_count, 0); // No render!
}

#[test]
fn app_processes_multiple_actions() {
    let view = TestAppView::new();
    let mut app: App<TestReducer, _> =
        App::new(view, TestState::default());

    app.dispatch(TestAction::Increment);
    app.dispatch(TestAction::Increment);
    app.dispatch(TestAction::Set(100));

    assert_eq!(app.state().count, 100);
    assert_eq!(app.view().render_count, 3);
    assert!(app.view().buffer.contains("Count: 100"));
}

// ============================================================================
// App queue tests
// ============================================================================

#[test]
fn app_enqueue_adds_action_to_queue() {
    let mut app: App<TestReducer, CounterView> =
        App::new(CounterView::new(), TestState::default());

    assert!(app.enqueue(TestAction::Increment).is_ok());
    assert!(app.enqueue(TestAction::Increment).is_ok());

    // Queue should have 2 actions
    assert!(!app.is_queue_empty());
}

#[test]
fn app_enqueue_fails_when_queue_full() {
    let mut app: App<TestReducer, CounterView, 2> =
        App::new(CounterView::new(), TestState::default());

    assert!(app.enqueue(TestAction::Increment).is_ok());
    assert!(app.enqueue(TestAction::Increment).is_ok());
    // Queue is now full (capacity 2)
    assert!(app.enqueue(TestAction::Increment).is_err());
}

#[test]
fn app_process_queue_dispatches_all_actions() {
    let mut app: App<TestReducer, CounterView> =
        App::new(CounterView::new(), TestState::default());

    app.enqueue(TestAction::Increment).unwrap();
    app.enqueue(TestAction::Increment).unwrap();
    app.enqueue(TestAction::Set(10)).unwrap();

    let effects = app.process_queue();

    assert_eq!(effects.len(), 3);
    assert_eq!(app.state().count, 10);
    assert!(app.is_queue_empty());
}

#[test]
fn app_process_queue_returns_effects() {
    let mut app: App<TestReducer, CounterView> =
        App::new(CounterView::new(), TestState { count: 5 });

    app.enqueue(TestAction::Increment).unwrap(); // 5 -> 6
    app.enqueue(TestAction::Set(6)).unwrap();    // 6 -> 6, unchanged
    app.enqueue(TestAction::Set(100)).unwrap();  // 6 -> 100

    let effects = app.process_queue();

    assert_eq!(effects.len(), 3);
    assert_eq!(effects[0], TestEffect::None);      // Increment
    assert_eq!(effects[1], TestEffect::Unchanged); // Set(6) when count=6
    assert_eq!(effects[2], TestEffect::None);      // Set(100)
    assert_eq!(app.state().count, 100);
}

// ============================================================================
// Manual reducer implementation tests (replaces old reducer! macro tests)
// ============================================================================

#[derive(Debug, Default)]
struct ManualState {
    value: i32,
    name: &'static str,
}

#[derive(Clone, Debug)]
enum ManualAction {
    Add(i32),
    Subtract(i32),
    SetName(&'static str),
    Reset,
}

struct ManualReducer;

impl Reducer for ManualReducer {
    type State = ManualState;
    type Action = ManualAction;
    type Effect = TestEffect;

    fn reduce(state: &mut Self::State, action: Self::Action) -> Self::Effect {
        match action {
            ManualAction::Add(n) => {
                state.value += n;
                TestEffect::None
            }
            ManualAction::Subtract(n) => {
                state.value -= n;
                TestEffect::None
            }
            ManualAction::SetName(s) => {
                state.name = s;
                TestEffect::None
            }
            ManualAction::Reset => {
                *state = ManualState::default();
                TestEffect::None
            }
        }
    }
}

struct ManualView;
impl View for ManualView {
    type State = ManualState;
    fn render(&mut self, _state: &Self::State) {}
}

#[test]
fn macro_reducer_handles_all_variants() {
    let mut state = ManualState {
        value: 10,
        name: "test",
    };

    ManualReducer::reduce(&mut state, ManualAction::Add(5));
    assert_eq!(state.value, 15);
    assert_eq!(state.name, "test");

    ManualReducer::reduce(&mut state, ManualAction::Subtract(3));
    assert_eq!(state.value, 12);

    ManualReducer::reduce(&mut state, ManualAction::SetName("new"));
    assert_eq!(state.name, "new");
    assert_eq!(state.value, 12);

    ManualReducer::reduce(&mut state, ManualAction::Reset);
    assert_eq!(state.value, 0);
    assert_eq!(state.name, "");
}

#[test]
fn macro_reducer_works_with_app() {
    let mut app: App<ManualReducer, ManualView> =
        App::new(ManualView, ManualState::default());

    let effect = app.dispatch(ManualAction::Add(42));
    assert!(!effect.is_unchanged());
    assert_eq!(app.state().value, 42);

    let effect = app.dispatch(ManualAction::SetName("hello"));
    assert!(!effect.is_unchanged());
    assert_eq!(app.state().name, "hello");
}

// ============================================================================
// Custom Effect with side effects tests
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq)]
enum AppEffect {
    Unchanged,
    None,
    Save,
    StartAnimation,
    StopAnimation,
}

impl Effect for AppEffect {
    fn is_unchanged(&self) -> bool {
        matches!(self, AppEffect::Unchanged)
    }
    fn changed() -> Self {
        AppEffect::None
    }
}

#[derive(Debug, Default)]
struct AppState {
    brightness: u8,
    animation_running: bool,
}

#[derive(Clone, Debug)]
enum AppAction {
    BrightnessUp,
    ToggleAnimation,
}

struct AppReducer;

impl Reducer for AppReducer {
    type State = AppState;
    type Action = AppAction;
    type Effect = AppEffect;

    fn reduce(state: &mut Self::State, action: Self::Action) -> Self::Effect {
        match action {
            AppAction::BrightnessUp if state.brightness >= 10 => AppEffect::Unchanged,
            AppAction::BrightnessUp => {
                state.brightness += 1;
                AppEffect::Save
            }
            AppAction::ToggleAnimation => {
                state.animation_running = !state.animation_running;
                if state.animation_running {
                    AppEffect::StartAnimation
                } else {
                    AppEffect::StopAnimation
                }
            }
        }
    }
}

struct MockView {
    buffer: TextView<64>,
}
impl View for MockView {
    type State = AppState;
    fn render(&mut self, state: &Self::State) {
        self.buffer.clear();
        write!(self.buffer.buffer_mut(), "Brightness: {}", state.brightness).ok();
    }
}

#[test]
fn custom_effects_signal_side_effects() {
    let mut app: App<AppReducer, MockView> = App::new(
        MockView { buffer: TextView::new() },
        AppState::default(),
    );

    // Brightness up should signal Save
    let effect = app.dispatch(AppAction::BrightnessUp);
    assert_eq!(effect, AppEffect::Save);
    assert_eq!(app.state().brightness, 1);

    // Toggle animation should signal StartAnimation
    let effect = app.dispatch(AppAction::ToggleAnimation);
    assert_eq!(effect, AppEffect::StartAnimation);
    assert!(app.state().animation_running);

    // Toggle again should signal StopAnimation
    let effect = app.dispatch(AppAction::ToggleAnimation);
    assert_eq!(effect, AppEffect::StopAnimation);
    assert!(!app.state().animation_running);
}

#[test]
fn main_loop_handles_effects() {
    let mut app: App<AppReducer, MockView> = App::new(
        MockView { buffer: TextView::new() },
        AppState::default(),
    );

    let mut save_count = 0;
    let mut animation_started = false;

    // Simulate main loop processing
    let actions = [
        AppAction::BrightnessUp,
        AppAction::BrightnessUp,
        AppAction::ToggleAnimation,
    ];

    for action in actions {
        let effect = app.dispatch(action);
        match effect {
            AppEffect::Save => save_count += 1,
            AppEffect::StartAnimation => animation_started = true,
            _ => {}
        }
    }

    assert_eq!(save_count, 2);
    assert!(animation_started);
    assert_eq!(app.state().brightness, 2);
    assert!(app.state().animation_running);
}

#[test]
fn queue_pattern_for_isr() {
    let mut app: App<AppReducer, MockView> = App::new(
        MockView { buffer: TextView::new() },
        AppState::default(),
    );

    // Simulate ISR enqueueing actions
    app.enqueue(AppAction::BrightnessUp).ok();
    app.enqueue(AppAction::BrightnessUp).ok();
    app.enqueue(AppAction::ToggleAnimation).ok();

    // Main loop processes queue
    let effects = app.process_queue();

    assert_eq!(effects.len(), 3);
    assert_eq!(effects[0], AppEffect::Save);
    assert_eq!(effects[1], AppEffect::Save);
    assert_eq!(effects[2], AppEffect::StartAnimation);
    assert_eq!(app.state().brightness, 2);
    assert!(app.state().animation_running);
}
