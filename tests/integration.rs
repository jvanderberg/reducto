//! Integration tests for reducto
//!
//! TDD: Write failing tests first (RED), then implement to pass (GREEN)

use core::fmt::Write;
use reducto::{App, Effect, Reducer, Store, TextView, View};

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

#[test]
fn store_new_creates_with_initial_state() {
    let store: Store<TestState, TestAction, 8> = Store::new(TestState { count: 42 });
    assert_eq!(store.state().count, 42);
}

#[test]
fn store_dispatch_updates_state() {
    let mut store: Store<TestState, TestAction, 8> = Store::new(TestState::default());

    let effect = store.dispatch::<TestReducer>(TestAction::Increment);

    assert_eq!(effect, TestEffect::None);
    assert_eq!(store.state().count, 1);
}

#[test]
fn store_dispatch_returns_unchanged_when_state_unchanged() {
    let mut store: Store<TestState, TestAction, 8> = Store::new(TestState { count: 5 });

    // Setting to same value should return Unchanged
    let effect = store.dispatch::<TestReducer>(TestAction::Set(5));

    assert_eq!(effect, TestEffect::Unchanged);
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
fn store_processes_queue_dispatches_all_actions() {
    let mut store: Store<TestState, TestAction, 8> = Store::new(TestState::default());

    store.enqueue(TestAction::Increment).unwrap();
    store.enqueue(TestAction::Increment).unwrap();
    store.enqueue(TestAction::Set(10)).unwrap();

    let mut changes = 0;
    while let Some(action) = store.pop_action() {
        let effect = store.dispatch::<TestReducer>(action);
        if !effect.is_unchanged() {
            changes += 1;
        }
    }

    assert_eq!(changes, 3); // 3 state changes
    assert_eq!(store.state().count, 10);
    assert!(store.is_queue_empty());
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

    fn text(&self) -> &str {
        self.buffer.as_str()
    }
}

#[test]
fn static_app_dispatch_renders_and_returns_effect() {
    let view = TestAppView::new();
    let mut app: App<TestState, TestAction, TestReducer, _> =
        App::new(view, TestState { count: 5 });

    let effect = app.dispatch(TestAction::Increment);

    assert_eq!(effect, TestEffect::None);
    assert_eq!(app.state().count, 6);
    assert_eq!(app.view().render_count, 1);
    assert!(app.view().text().contains("Count: 6"));
}

#[test]
fn static_app_unchanged_skips_render() {
    let view = TestAppView::new();
    let mut app: App<TestState, TestAction, TestReducer, _> =
        App::new(view, TestState { count: 10 });

    // Setting to same value returns unchanged - should NOT render
    let effect = app.dispatch(TestAction::Set(10));

    assert_eq!(effect, TestEffect::Unchanged);
    assert_eq!(app.state().count, 10);
    assert_eq!(app.view().render_count, 0); // No render!
}

#[test]
fn static_app_processes_multiple_actions() {
    let view = TestAppView::new();
    let mut app: App<TestState, TestAction, TestReducer, _> =
        App::new(view, TestState::default());

    app.dispatch(TestAction::Increment);
    app.dispatch(TestAction::Increment);
    app.dispatch(TestAction::Set(100));

    assert_eq!(app.state().count, 100);
    assert_eq!(app.view().render_count, 3);
    assert!(app.view().text().contains("Count: 100"));
}

// ============================================================================
// reducer! macro tests
// ============================================================================

// State for macro-generated reducer
#[derive(Debug, Default)]
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

// Generate reducer using macro - implicit effect returns
reducto::reducer! {
    MacroReducer for MacroState, MacroAction, TestEffect {
        MacroAction::Add(n) => |state| state.value += n,
        MacroAction::Subtract(n) => |state| state.value -= n,
        MacroAction::SetName(s) => |state| state.name = s,
        MacroAction::Reset => |state| *state = MacroState::default(),
    }
}

#[test]
fn macro_reducer_handles_all_variants() {
    let mut state = MacroState {
        value: 10,
        name: "test",
    };

    MacroReducer::reduce(&mut state, MacroAction::Add(5));
    assert_eq!(state.value, 15);
    assert_eq!(state.name, "test");

    MacroReducer::reduce(&mut state, MacroAction::Subtract(3));
    assert_eq!(state.value, 12);

    MacroReducer::reduce(&mut state, MacroAction::SetName("new"));
    assert_eq!(state.name, "new");
    assert_eq!(state.value, 12);

    MacroReducer::reduce(&mut state, MacroAction::Reset);
    assert_eq!(state.value, 0);
    assert_eq!(state.name, "");
}

#[test]
fn macro_reducer_works_with_store() {
    let mut store: Store<MacroState, MacroAction, 8> = Store::new(MacroState::default());

    let effect = store.dispatch::<MacroReducer>(MacroAction::Add(42));
    assert!(!effect.is_unchanged());
    assert_eq!(store.state().value, 42);

    let effect = store.dispatch::<MacroReducer>(MacroAction::SetName("hello"));
    assert!(!effect.is_unchanged());
    assert_eq!(store.state().name, "hello");
}

// ============================================================================
// Effect-based conditional reducer tests
// ============================================================================

#[derive(Debug, Default)]
struct ConditionalState {
    value: i32,
}

#[derive(Clone, Debug)]
enum ConditionalAction {
    SetValue(i32),
}

struct ConditionalReducer;

impl Reducer for ConditionalReducer {
    type State = ConditionalState;
    type Action = ConditionalAction;
    type Effect = TestEffect;

    fn reduce(state: &mut Self::State, action: Self::Action) -> Self::Effect {
        match action {
            ConditionalAction::SetValue(n) if n == state.value => TestEffect::Unchanged,
            ConditionalAction::SetValue(n) => {
                state.value = n;
                TestEffect::None
            }
        }
    }
}

#[test]
fn conditional_reducer_if_else_works() {
    let mut store: Store<ConditionalState, ConditionalAction, 8> =
        Store::new(ConditionalState { value: 10 });

    // Setting to same value - unchanged
    let effect = store.dispatch::<ConditionalReducer>(ConditionalAction::SetValue(10));
    assert!(effect.is_unchanged());
    assert_eq!(store.state().value, 10);

    // Setting to different value - changed
    let effect = store.dispatch::<ConditionalReducer>(ConditionalAction::SetValue(42));
    assert!(!effect.is_unchanged());
    assert_eq!(store.state().value, 42);
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
    BrightnessDown,
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
            AppAction::BrightnessDown if state.brightness == 0 => AppEffect::Unchanged,
            AppAction::BrightnessDown => {
                state.brightness -= 1;
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

#[test]
fn custom_effects_signal_side_effects() {
    let mut store: Store<AppState, AppAction, 8> = Store::new(AppState::default());

    // Brightness up should signal Save
    let effect = store.dispatch::<AppReducer>(AppAction::BrightnessUp);
    assert_eq!(effect, AppEffect::Save);
    assert_eq!(store.state().brightness, 1);

    // At max, should signal Unchanged
    store.state_mut().brightness = 10;
    let effect = store.dispatch::<AppReducer>(AppAction::BrightnessUp);
    assert_eq!(effect, AppEffect::Unchanged);

    // Toggle animation should signal StartAnimation
    let effect = store.dispatch::<AppReducer>(AppAction::ToggleAnimation);
    assert_eq!(effect, AppEffect::StartAnimation);
    assert!(store.state().animation_running);

    // Toggle again should signal StopAnimation
    let effect = store.dispatch::<AppReducer>(AppAction::ToggleAnimation);
    assert_eq!(effect, AppEffect::StopAnimation);
    assert!(!store.state().animation_running);
}

#[test]
fn main_loop_handles_effects() {
    struct MockView {
        buffer: TextView<64>,
    }
    impl View for MockView {
        type State = AppState;
        fn render(&mut self, state: &Self::State) {
            self.buffer.clear();
            write!(self.buffer.buffer_mut(), "Brightness: {}", state.brightness).ok();
        }
        fn text(&self) -> &str {
            self.buffer.as_str()
        }
    }

    let mut app: App<AppState, AppAction, AppReducer, MockView> = App::new(
        MockView {
            buffer: TextView::new(),
        },
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

