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
use reducto::{App, Effect, View, TextView};
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

// Define your reducer using the macro
reducto::reducer! {
    AppReducer for AppState, Action, AppEffect {
        Action::Increment => |state| state.count += 1,
        Action::Decrement => |state| state.count -= 1,
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
    fn text(&self) -> &str { self.buffer.as_str() }
}

// Create and use your app
let mut app: App<AppState, Action, AppReducer, AppView> = App::new(
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

## Reducer Macro

The `reducer!` macro provides implicit effect returns:

- If the body returns `()`, it becomes `Effect::changed()`
- If the body returns an explicit `Effect`, it passes through

```rust
reducto::reducer! {
    pub AppReducer for AppState, Action, AppEffect {
        // Implicit: returns () which becomes changed()
        Action::Increment => |state| state.count += 1,

        // Explicit: returns specific effect
        Action::Save => |state| {
            state.dirty = false;
            AppEffect::Save
        },

        // Conditional logic
        Action::SetValue(v) => |state| {
            if v == state.value {
                AppEffect::Unchanged
            } else {
                state.value = v;
                AppEffect::Save
            }
        },
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

## License

MIT OR Apache-2.0
