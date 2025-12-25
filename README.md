# Reducto

A `no_std` Redux-like state management framework for embedded systems.

## Features

- **Mutable reducers** - In-place state mutation, no cloning required
- **Explicit effects** - Reducer returns side effects for the main loop to handle
- **Exhaustive matching** - Rust's type system enforces handling all action variants
- **Built-in action queue** - ISR-safe enqueueing with deferred processing
- **Zero-copy** - State is never cloned, mutated in place

## Quick Start

```rust
use reducto::{App, Effect, Reducer, View, TextView};
use core::fmt::Write;

// Define your state
#[derive(Default)]
struct AppState { count: i32 }

// Define your actions
enum Action { Increment, Decrement }

// Define your effects
#[derive(Clone, Copy)]
enum AppEffect { None, Unchanged }

impl Effect for AppEffect {
    fn is_unchanged(&self) -> bool {
        matches!(self, AppEffect::Unchanged)
    }
    fn changed() -> Self { AppEffect::None }
}

// Define your reducer
struct AppReducer;

impl Reducer for AppReducer {
    type State = AppState;
    type Action = Action;
    type Effect = AppEffect;

    fn reduce(state: &mut Self::State, action: Self::Action) -> Self::Effect {
        match action {
            Action::Increment => state.count += 1,
            Action::Decrement => state.count -= 1,
        }
        AppEffect::None
    }
}

// Define your view
struct AppView { buffer: TextView<64> }

impl View for AppView {
    type State = AppState;
    fn render(&mut self, state: &Self::State) {
        self.buffer.clear();
        write!(self.buffer.buffer_mut(), "Count: {}", state.count).ok();
    }
}

// Create and use your app
let mut app: App<AppReducer, AppView> = App::new(
    AppView { buffer: TextView::new() },
    AppState::default(),
);
app.dispatch(Action::Increment);
assert_eq!(app.state().count, 1);
```

## Effects

Effects signal side effects to the main loop. The framework uses `is_unchanged()` for render skipping - all other variants are for your main loop to handle.

```rust
#[derive(Clone, Copy)]
enum AppEffect {
    Unchanged,  // Skip render
    None,       // Render only
    Save,       // Render + save to storage
}

impl Effect for AppEffect {
    fn is_unchanged(&self) -> bool {
        matches!(self, AppEffect::Unchanged)
    }
    fn changed() -> Self {
        AppEffect::None
    }
}
```

## Reducer Implementation

Reducers mutate state and return an effect. Rust's exhaustive match ensures all actions are handled:

```rust
impl Reducer for AppReducer {
    type State = AppState;
    type Action = Action;
    type Effect = AppEffect;

    fn reduce(state: &mut Self::State, action: Self::Action) -> Self::Effect {
        match action {
            Action::Increment => {
                state.count += 1;
                AppEffect::None
            }
            Action::SetValue(v) if v == state.value => {
                AppEffect::Unchanged  // No change, skip render
            }
            Action::SetValue(v) => {
                state.value = v;
                AppEffect::Save  // Persist this change
            }
            Action::Reset => {
                *state = AppState::default();
                AppEffect::None
            }
        }
    }
}
```

## Two Dispatch Patterns

### Direct Dispatch (async runtimes)

For embassy or RTIC where actions come through channels:

```rust
loop {
    let action = ACTION_CHANNEL.receive().await;
    let effect = app.dispatch(action);

    match effect {
        AppEffect::Save => storage::save(app.state()),
        _ => {}
    }
}
```

### Queue Dispatch (bare-metal ISRs)

For interrupt handlers where you need fast enqueue and deferred processing:

```rust
// In ISR (fast - just enqueue):
critical_section::with(|cs| {
    APP.borrow_ref_mut(cs).enqueue(Action::ButtonPressed).ok();
});

// In main loop (process all queued actions):
loop {
    // Wait for interrupt...

    critical_section::with(|cs| {
        let effects = APP.borrow_ref_mut(cs).process_queue();
        for effect in effects {
            match effect {
                AppEffect::Save => storage::save(&APP.borrow_ref(cs).state()),
                _ => {}
            }
        }
    });
}
```

**When to use the queue:**
- ISRs should be fast - `enqueue()` just pushes to a queue and returns
- Calling `dispatch()` from an ISR would run the reducer and render, blocking other interrupts
- The queue defers processing to the main loop where timing is less critical

**When to use direct dispatch:**
- Async runtimes (embassy) already have channels that act as queues
- Single-threaded code where actions come from polling

## View Composition (Optional)

The `reducto-view` crate provides a macro for declarative view composition:

```rust
use reducto_view::view;

view! {
    AppView<D: Write> for AppState {
        <Header />
        @if state.loading { <Spinner /> } @else { <Content /> }
        @match state.screen {
            Screen::Home => <HomeScreen />,
            Screen::Settings => <SettingsScreen />,
        }
        <Footer />
    }
}
```

Components are structs with a `render` method:

```rust
struct Header;
impl Header {
    fn render<D: Write>(display: &mut D, state: &AppState) {
        writeln!(display, "=== {} ===", state.title).ok();
    }
}
```

See `reducto-view` crate for details.

## License

MIT OR Apache-2.0
