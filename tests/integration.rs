use core::fmt::Write;
use reducto::{App, EffectApp, Reducer, TextView, TransitionEffect, View};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct State {
    count: i32,
    label: &'static str,
}

#[derive(Clone, Debug)]
enum Action {
    Increment,
    Decrement,
    Set(i32),
    SetLabel(&'static str),
}

struct TestReducer;

impl Reducer for TestReducer {
    type State = State;
    type Action = Action;

    fn reduce(state: &State, action: Action) -> State {
        let mut next = state.clone();
        match action {
            Action::Increment => {
                next.count += 1;
            }
            Action::Decrement => {
                next.count -= 1;
            }
            Action::Set(value) if value == state.count => {}
            Action::Set(value) => {
                next.count = value;
            }
            Action::SetLabel(label) if label == state.label => {}
            Action::SetLabel(label) => {
                next.label = label;
            }
        }
        next
    }
}

struct TestView {
    buffer: TextView<64>,
    full_renders: usize,
    transitions: usize,
    old_count: i32,
    new_count: i32,
}

impl TestView {
    fn new() -> Self {
        Self {
            buffer: TextView::new(),
            full_renders: 0,
            transitions: 0,
            old_count: 0,
            new_count: 0,
        }
    }
}

impl View for TestView {
    type State = State;

    fn render(&mut self, state: &State) {
        self.buffer.clear();
        write!(self.buffer.buffer_mut(), "{}:{}", state.label, state.count).ok();
        self.full_renders += 1;
    }

    fn render_transition(&mut self, old: &State, new: &State) {
        self.transitions += 1;
        self.old_count = old.count;
        self.new_count = new.count;
        self.buffer.clear();
        write!(self.buffer.buffer_mut(), "{}:{}", new.label, new.count).ok();
    }
}

#[test]
fn reducer_returns_new_state_and_preserves_old_state() {
    let old = State::default();
    let new = TestReducer::reduce(&old, Action::Increment);
    assert_eq!(old.count, 0);
    assert_eq!(new.count, 1);
}

#[test]
fn no_op_skips_view() {
    let mut app: App<TestReducer, TestView> = App::new(TestView::new(), State::default());
    assert!(!app.dispatch(Action::Set(0)));
    assert_eq!(app.view().transitions, 0);
}

#[test]
fn view_receives_exact_old_and_new_state() {
    let mut app: App<TestReducer, TestView> = App::new(TestView::new(), State::default());
    assert!(app.dispatch(Action::Set(42)));
    assert_eq!(app.view().transitions, 1);
    assert_eq!(app.view().old_count, 0);
    assert_eq!(app.view().new_count, 42);
}

#[test]
fn mutable_view_access_services_deferred_view_work() {
    let mut app: App<TestReducer, TestView> = App::new(TestView::new(), State::default());
    app.view_mut().transitions = 7;
    assert_eq!(app.view().transitions, 7);

    let mut effect_app: EffectApp<TestReducer, TestView, CountEffect> =
        EffectApp::new(TestView::new(), State::default());
    effect_app.view_mut().transitions = 9;
    assert_eq!(effect_app.view().transitions, 9);
}

struct CountEffect;

impl TransitionEffect<State> for CountEffect {
    type Effect = (i32, i32);

    fn plan(old: &State, new: &State) -> Option<Self::Effect> {
        (old.count != new.count).then_some((old.count, new.count))
    }
}

#[test]
fn typed_effect_receives_exact_old_and_new_state_only_on_change() {
    let mut app: EffectApp<TestReducer, TestView, CountEffect> =
        EffectApp::new(TestView::new(), State::default());
    let outcome = app.dispatch(Action::Set(42));
    assert!(outcome.changed());
    assert_eq!(outcome.effect(), Some((0, 42)));
    assert_eq!(app.state().count, 42);

    let outcome = app.dispatch(Action::Set(42));
    assert!(!outcome.changed());
    assert_eq!(outcome.effect(), None);
}

struct NoEffectForLabel;

impl TransitionEffect<State> for NoEffectForLabel {
    type Effect = ();

    fn plan(_old: &State, _new: &State) -> Option<Self::Effect> {
        None
    }
}

#[test]
fn changed_transition_renders_when_planner_returns_no_effect() {
    let mut app: EffectApp<TestReducer, TestView, NoEffectForLabel> =
        EffectApp::new(TestView::new(), State::default());
    let outcome = app.dispatch(Action::SetLabel("ready"));
    assert!(outcome.changed());
    assert_eq!(outcome.effect(), None);
    assert_eq!(app.view().transitions, 1);
}

#[test]
fn equality_detection_catches_a_forgotten_version_bump() {
    let mut app: App<TestReducer, TestView> = App::new(TestView::new(), State::default());
    assert!(app.dispatch(Action::SetLabel("ready")));
    assert_eq!(app.view().transitions, 1);
}

#[test]
fn full_render_is_explicit() {
    let state = State {
        count: 7,
        label: "count",
    };
    let mut app: App<TestReducer, TestView> = App::new(TestView::new(), state);
    app.render_full();
    assert_eq!(app.view().full_renders, 1);
    assert_eq!(app.view().buffer.as_str(), "count:7");
}

#[test]
fn queue_can_render_each_transition() {
    let mut app: App<TestReducer, TestView, 4> = App::new(TestView::new(), State::default());
    app.enqueue(Action::Increment).unwrap();
    app.enqueue(Action::Set(1)).unwrap();
    app.enqueue(Action::Increment).unwrap();
    assert_eq!(app.process_queue(), 2);
    assert_eq!(app.state().count, 2);
    assert_eq!(app.view().transitions, 2);
    assert!(app.is_queue_empty());
}

#[test]
fn queue_can_coalesce_to_one_old_new_transition() {
    let mut app: App<TestReducer, TestView, 4> = App::new(TestView::new(), State::default());
    app.enqueue(Action::Increment).unwrap();
    app.enqueue(Action::Increment).unwrap();
    app.enqueue(Action::SetLabel("ready")).unwrap();
    assert_eq!(app.process_queue_coalesced(), 3);
    assert_eq!(app.view().transitions, 1);
    assert_eq!(app.view().old_count, 0);
    assert_eq!(app.view().new_count, 2);
    assert_eq!(app.view().buffer.as_str(), "ready:2");
}

#[test]
fn coalescing_can_cancel_intermediate_transitions() {
    let mut app: App<TestReducer, TestView, 2> = App::new(TestView::new(), State::default());
    app.enqueue(Action::Increment).unwrap();
    app.enqueue(Action::Decrement).unwrap();
    assert_eq!(app.process_queue_coalesced(), 2);
    assert_eq!(app.state(), &State::default());
    assert_eq!(app.view().transitions, 0);
}

#[test]
fn enqueue_is_bounded() {
    let mut app: App<TestReducer, TestView, 1> = App::new(TestView::new(), State::default());
    assert!(app.enqueue(Action::Increment).is_ok());
    assert!(app.enqueue(Action::Increment).is_err());
}
