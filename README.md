# Reducto

A `no_std`, Redux-style state container for embedded Rust.

The normative design, prohibitions, embedded constraints, and review checklist
are in [docs/reactive_architecture.md](docs/reactive_architecture.md). In short:

- one application state is the source of truth;
- immutable actions describe events;
- a pure reducer returns new state and performs no I/O;
- dispatch compares complete old/new state values, so no-op detection cannot be
  broken by a forgotten version bump;
- dispatch supplies exact old/new state to transition consumers;
- views compare old/new displayed projections and update only changed widgets;
- hardware side effects compare old/new semantic state outside the reducer.

```rust
use core::fmt::Write;
use reducto::{EffectApp, Reducer, TextView, TransitionEffect, View};

#[derive(Clone, Default, PartialEq)]
struct State {
    count: i32,
}

enum Action { Increment, Set(i32) }

struct AppReducer;

impl Reducer for AppReducer {
    type State = State;
    type Action = Action;

    fn reduce(old: &State, action: Action) -> State {
        let mut new = old.clone();
        match action {
            Action::Increment => new.count += 1,
            Action::Set(value) if value == old.count => return new,
            Action::Set(value) => new.count = value,
        }
        new
    }
}

struct AppView(TextView<64>);

impl View for AppView {
    type State = State;

    fn render(&mut self, state: &State) {
        self.0.clear();
        write!(self.0.buffer_mut(), "Count: {}", state.count).ok();
    }

    fn render_transition(&mut self, old: &State, new: &State) {
        if old.count != new.count {
            self.render(new);
        }
    }
}

struct PersistCount;

impl TransitionEffect<State> for PersistCount {
    type Effect = i32;

    fn plan(old: &State, new: &State) -> Option<i32> {
        (old.count != new.count).then_some(new.count)
    }
}

let mut app: EffectApp<AppReducer, AppView, PersistCount> =
    EffectApp::new(AppView(TextView::new()), State::default());
app.render_full();
let outcome = app.dispatch(Action::Increment);
assert!(outcome.changed());
assert_eq!(outcome.effect(), Some(1));
assert_eq!(app.state().count, 1);
```

`App::enqueue` is a bounded foreground queue and requires `&mut App`; it is not
an ISR-safe API. ISR producers should use the optional `ActionChannel` with the
`embassy` feature, or a platform-specific critical-section queue. After actions
reach the foreground, `process_queue` renders each real transition and
`process_queue_coalesced` reduces the batch into one old/final transition.

Use `EffectApp` and `TransitionEffect`, as above, when hardware or persistence
work must observe every old/new transition. `EffectApp` has no plain dispatch
path that can bypass planning. Consume `outcome.effect()` only after dispatch
returns.

Reducers and effect planners must remain deterministic and free of I/O. Effect
values are executed by the caller only after dispatch returns.

Reducers return owned state values, so each action generally clones or rebuilds
the state. This favors simple, auditable old/new semantics. For very large or
high-rate state, use structural sharing or keep raw sample streams outside the
application state. `PartialEq` must represent application truth; avoid raw
floating-point fields whose `NaN` values are not equal to themselves.

The optional `embassy` feature exposes `ActionChannel` for async applications.
The view-macro experiments in this repository are not part of the 0.1 release.

## License

MIT
