//! # Reducto
//!
//! A `no_std` Redux-like state management framework for embedded systems.
//!
//! ## Design Principles
//!
//! 1. **Mutable reducers** - In-place state mutation, no cloning
//! 2. **Explicit effects** - Reducer returns side effects for the main loop to handle
//! 3. **Exhaustive matching** - Rust's type system enforces handling all action variants
//! 4. **Stack-based App** - Single buffer, no cloning, embassy/RTIC compatible
//! 5. **Framework renders** - dispatch() handles rendering internally
//!
//! ## Example
//!
//! ```rust
//! use reducto::{Effect, Reducer, App, View, TextView};
//! use core::fmt::Write;
//!
//! #[derive(Default)]
//! struct AppState { count: i32 }
//!
//! enum Action { Increment, Decrement }
//!
//! #[derive(Clone, Copy)]
//! enum AppEffect { None, Unchanged }
//!
//! impl Effect for AppEffect {
//!     fn is_unchanged(&self) -> bool {
//!         matches!(self, AppEffect::Unchanged)
//!     }
//! }
//!
//! struct AppReducer;
//!
//! impl Reducer for AppReducer {
//!     type State = AppState;
//!     type Action = Action;
//!     type Effect = AppEffect;
//!
//!     fn reduce(state: &mut Self::State, action: Self::Action) -> Self::Effect {
//!         match action {
//!             Action::Increment => { state.count += 1; AppEffect::None }
//!             Action::Decrement => { state.count -= 1; AppEffect::None }
//!         }
//!     }
//! }
//!
//! struct AppView { buffer: TextView<64> }
//!
//! impl View for AppView {
//!     type State = AppState;
//!     fn render(&mut self, state: &Self::State) {
//!         self.buffer.clear();
//!         write!(self.buffer.buffer_mut(), "Count: {}", state.count).ok();
//!     }
//!     fn text(&self) -> &str { self.buffer.as_str() }
//! }
//!
//! let mut app = App::<AppState, Action, AppReducer, AppView>::new(
//!     AppView { buffer: TextView::new() },
//!     AppState::default(),
//! );
//! app.dispatch(Action::Increment);
//! assert_eq!(app.state().count, 1);
//! ```

#![no_std]

use core::marker::PhantomData;
use heapless::String;

/// Trait for reducer return types that indicate side effects.
///
/// Implement this trait on your effect enum to tell the framework
/// whether to skip rendering (when `is_unchanged()` returns true).
///
/// # Example
///
/// ```rust
/// use reducto::Effect;
///
/// #[derive(Clone, Copy)]
/// enum AppEffect {
///     Unchanged,          // Skip render
///     None,               // Render only (common case)
///     Save,               // Render + save to storage
///     StartAnimation,     // Render + start animation
/// }
///
/// impl Effect for AppEffect {
///     fn is_unchanged(&self) -> bool {
///         matches!(self, AppEffect::Unchanged)
///     }
/// }
/// ```
pub trait Effect {
    /// Returns true if state was not modified (skip rendering).
    fn is_unchanged(&self) -> bool;
}

/// Mutable state transformation: (&mut State, Action) -> Effect
///
/// Implement this trait to define how actions transform state.
/// Rust's exhaustive `match` on the Action enum ensures all variants are handled.
///
/// Unlike traditional Redux, state is mutated in-place for zero-copy performance.
/// The reducer returns an Effect that describes any side effects the main loop
/// should perform.
///
/// # Example
///
/// ```rust,ignore
/// impl Reducer for AppReducer {
///     type State = AppState;
///     type Action = Action;
///     type Effect = AppEffect;
///
///     fn reduce(state: &mut Self::State, action: Self::Action) -> Self::Effect {
///         match action {
///             Action::Tick1s => {
///                 state.uptime_seconds += 1;
///                 AppEffect::None
///             }
///             Action::BrightnessUp => {
///                 state.brightness = (state.brightness + 1).min(10);
///                 AppEffect::Save  // Signal main loop to save
///             }
///             Action::ButtonNext if state.at_end() => {
///                 AppEffect::Unchanged  // No-op, skip render
///             }
///         }
///     }
/// }
/// ```
pub trait Reducer {
    /// The state type this reducer operates on
    type State;
    /// The action type this reducer handles
    type Action;
    /// The effect type returned by reduce (user-defined enum)
    type Effect: Effect;

    /// Transform state based on an action.
    ///
    /// Mutate state in-place and return an Effect describing any
    /// side effects. Return an effect where `is_unchanged()` is true
    /// to skip rendering.
    fn reduce(state: &mut Self::State, action: Self::Action) -> Self::Effect;
}

/// Store holds application state and an action queue.
///
/// The Store is responsible for:
/// - Holding the current state
/// - Queueing actions (typically from ISRs)
/// - Dispatching queued actions through the reducer
/// - Returning effects for side effect handling
///
/// # Queue Design
///
/// ISRs and other event sources call `enqueue()` to add actions.
/// The main loop calls `process_queue()` to drain all queued actions.
///
/// # Example
///
/// ```rust,ignore
/// // In ISR:
/// store.enqueue(Action::ButtonPressed).ok();
///
/// // In main loop:
/// while let Some(action) = store.pop_action() {
///     let effect = store.dispatch::<MyReducer>(action);
///     match effect {
///         Effect::Save => save_state(store.state()),
///         _ => {}
///     }
/// }
/// ```
pub struct Store<S, A, const N: usize = 8> {
    state: S,
    queue: heapless::Deque<A, N>,
}

impl<S, A, const N: usize> Store<S, A, N> {
    /// Create a new store with the given initial state and empty action queue.
    pub fn new(initial: S) -> Self {
        Self {
            state: initial,
            queue: heapless::Deque::new(),
        }
    }

    /// Get a reference to the current state.
    pub fn state(&self) -> &S {
        &self.state
    }

    /// Get a mutable reference to the current state.
    pub fn state_mut(&mut self) -> &mut S {
        &mut self.state
    }

    /// Enqueue an action for later processing.
    ///
    /// Call this from ISRs or event handlers. Returns `Err(action)` if
    /// the queue is full.
    ///
    /// Note: For ISR safety, wrap the Store in appropriate synchronization
    /// primitives (e.g., `Mutex<RefCell<Store>>` or use Embassy channels).
    pub fn enqueue(&mut self, action: A) -> Result<(), A> {
        self.queue.push_back(action)
    }

    /// Check if the action queue is empty.
    pub fn is_queue_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Pop the next action from the queue, if any.
    ///
    /// Used by `run_loop` to process actions one at a time.
    pub fn pop_action(&mut self) -> Option<A> {
        self.queue.pop_front()
    }

    /// Dispatch a single action through the reducer immediately.
    ///
    /// Returns the Effect from the reducer. State is mutated in-place.
    pub fn dispatch<R>(&mut self, action: A) -> R::Effect
    where
        R: Reducer<State = S, Action = A>,
    {
        R::reduce(&mut self.state, action)
    }
}

/// View renders state to some output.
///
/// Views own their internal buffer and handle rendering. For hardware displays,
/// the View implementation can also flush to the display driver.
/// For testing, use `text()` to inspect the rendered output.
///
/// # Example
///
/// ```rust
/// use reducto::{View, TextView};
/// use core::fmt::Write;
///
/// struct CounterView {
///     buffer: TextView<128>,
/// }
///
/// impl CounterView {
///     fn new() -> Self {
///         Self { buffer: TextView::new() }
///     }
/// }
///
/// impl View for CounterView {
///     type State = i32;
///
///     fn render(&mut self, state: &Self::State) {
///         self.buffer.clear();
///         write!(self.buffer.buffer_mut(), "Count: {}", state).ok();
///     }
///
///     fn text(&self) -> &str {
///         self.buffer.as_str()
///     }
/// }
///
/// let mut view = CounterView::new();
/// view.render(&42);
/// assert!(view.text().contains("42"));
/// ```
pub trait View {
    /// The state type this view renders
    type State;

    /// Render the state to the internal buffer.
    ///
    /// For hardware views, this can also flush to the display.
    fn render(&mut self, state: &Self::State);

    /// Get the rendered text for inspection (primarily for testing).
    fn text(&self) -> &str;
}

/// A text buffer for testing views without hardware.
///
/// `TextView` wraps a `heapless::String` to provide a no-allocation
/// text buffer that views can render to. In tests, you can inspect
/// the buffer contents to verify correct rendering.
///
/// # Example
///
/// ```rust
/// use reducto::TextView;
/// use core::fmt::Write;
///
/// let mut view = TextView::<64>::new();
/// write!(view.buffer_mut(), "Hello, {}!", "world").ok();
/// assert!(view.contains("Hello"));
/// assert_eq!(view.as_str(), "Hello, world!");
/// ```
pub struct TextView<const N: usize> {
    buffer: String<N>,
}

impl<const N: usize> TextView<N> {
    /// Create a new empty text view.
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    /// Clear the text buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Get the buffer contents as a string slice.
    pub fn as_str(&self) -> &str {
        self.buffer.as_str()
    }

    /// Check if the buffer contains the given substring.
    pub fn contains(&self, s: &str) -> bool {
        self.buffer.as_str().contains(s)
    }

    /// Get a mutable reference to the underlying buffer for writing.
    pub fn buffer_mut(&mut self) -> &mut String<N> {
        &mut self.buffer
    }
}

impl<const N: usize> Default for TextView<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// An application that owns its state and view.
///
/// App provides the recommended way to structure an embedded application.
/// It owns the state and view, providing a clean `dispatch()` API that
/// automatically renders when state changes.
///
/// State is mutated in-place for zero-copy performance.
///
/// # Example
///
/// ```rust,ignore
/// let mut app = App::<AppState, Action, AppReducer, AppView>::new(
///     AppView::new(),
///     AppState::default(),
/// );
///
/// loop {
///     let action = get_action();
///     let effect = app.dispatch(action);  // Renders internally if changed
///
///     match effect {
///         Effect::Save => storage::save(app.state()),
///         Effect::StartAnimation => led::start_animation(),
///         _ => {}
///     }
/// }
/// ```
pub struct App<S, A, R, V>
where
    R: Reducer<State = S, Action = A>,
    V: View<State = S>,
{
    state: S,
    view: V,
    _reducer: PhantomData<R>,
    _action: PhantomData<A>,
}

impl<S, A, R, V> App<S, A, R, V>
where
    R: Reducer<State = S, Action = A>,
    V: View<State = S>,
{
    /// Create a new application with the given view and initial state.
    pub fn new(view: V, initial_state: S) -> Self {
        Self {
            state: initial_state,
            view,
            _reducer: PhantomData,
            _action: PhantomData,
        }
    }

    /// Dispatch an action through the reducer.
    ///
    /// The view is automatically rendered if the effect indicates state changed
    /// (i.e., `effect.is_unchanged()` returns false).
    ///
    /// Returns the Effect from the reducer for side effect handling.
    pub fn dispatch(&mut self, action: A) -> R::Effect {
        let effect = R::reduce(&mut self.state, action);
        if !effect.is_unchanged() {
            self.view.render(&self.state);
        }
        effect
    }

    /// Get a reference to the current state.
    pub fn state(&self) -> &S {
        &self.state
    }

    /// Get a mutable reference to the view.
    pub fn view(&mut self) -> &mut V {
        &mut self.view
    }
}

/// Generate a Reducer implementation from a pattern-matching DSL.
///
/// This macro provides a concise way to define reducers while ensuring
/// that all action variants are handled (Rust's exhaustive match checking
/// applies to the generated code).
///
/// # Syntax
///
/// ```rust,ignore
/// reducto::reducer! {
///     ReducerName for State, Action, Effect {
///         Action::Variant1 => |state| { state.field += 1; Effect::None },
///         Action::Variant2(val) => |state| { state.field = val; Effect::Save },
///     }
/// }
/// ```
///
/// Each arm receives `&mut state` and must return an Effect.
///
/// # Example
///
/// ```rust
/// use reducto::{Effect, Reducer, Store};
///
/// #[derive(Default)]
/// struct Counter { count: i32 }
///
/// enum CounterAction { Increment, Decrement, Set(i32) }
///
/// #[derive(Clone, Copy)]
/// enum CounterEffect { None, Unchanged }
///
/// impl Effect for CounterEffect {
///     fn is_unchanged(&self) -> bool { matches!(self, CounterEffect::Unchanged) }
/// }
///
/// reducto::reducer! {
///     CounterReducer for Counter, CounterAction, CounterEffect {
///         CounterAction::Increment => |state| { state.count += 1; CounterEffect::None },
///         CounterAction::Decrement => |state| { state.count -= 1; CounterEffect::None },
///         CounterAction::Set(n) => |state| { state.count = n; CounterEffect::None },
///     }
/// }
///
/// let mut store: Store<Counter, CounterAction> = Store::new(Counter::default());
/// store.dispatch::<CounterReducer>(CounterAction::Increment);
/// assert_eq!(store.state().count, 1);
/// ```
#[macro_export]
macro_rules! reducer {
    (
        $reducer_name:ident for $state_type:ty, $action_type:ty, $effect_type:ty {
            $( $pattern:pat => |$var:ident| $body:expr ),* $(,)?
        }
    ) => {
        struct $reducer_name;

        impl $crate::Reducer for $reducer_name {
            type State = $state_type;
            type Action = $action_type;
            type Effect = $effect_type;

            #[allow(unused_variables, unused_mut)]
            fn reduce(state: &mut Self::State, action: Self::Action) -> Self::Effect {
                match action {
                    $(
                        $pattern => {
                            let $var = state;
                            $body
                        }
                    ),*
                }
            }
        }
    };
}

