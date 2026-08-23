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
//!     fn changed() -> Self { AppEffect::None }
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
//! }
//!
//! let mut app = App::<AppReducer, AppView>::new(
//!     AppView { buffer: TextView::new() },
//!     AppState::default(),
//! );
//! app.dispatch(Action::Increment);
//! assert_eq!(app.state().count, 1);
//! ```

#![no_std]

use core::marker::PhantomData;
use heapless::String;

#[cfg(feature = "embassy")]
mod channel;

#[cfg(feature = "embassy")]
pub use channel::ActionChannel;

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
///     fn changed() -> Self {
///         AppEffect::None
///     }
/// }
/// ```
pub trait Effect {
    /// Returns true if state was not modified (skip rendering).
    fn is_unchanged(&self) -> bool;

    /// Returns the default "state changed" effect.
    ///
    /// Typically returns the variant that means "state changed, render needed"
    /// with no additional side effects.
    fn changed() -> Self;
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
/// }
///
/// let mut view = CounterView::new();
/// view.render(&42);
/// assert!(view.buffer.contains("42"));
/// ```
pub trait View {
    /// The state type this view renders
    type State;

    /// Render the state to the internal buffer.
    ///
    /// For hardware views, this can also flush to the display.
    fn render(&mut self, state: &Self::State);
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

/// An application that owns its state, view, and action queue.
///
/// App provides the recommended way to structure an embedded application.
/// It owns the state and view, providing a clean `dispatch()` API that
/// automatically renders when state changes.
///
/// ## Two Dispatch Patterns
///
/// **Direct dispatch** - for async runtimes (embassy) where actions come through channels:
/// ```rust,ignore
/// loop {
///     let action = ACTION_CHANNEL.receive().await;
///     let effect = app.dispatch(action);
///     // handle effect...
/// }
/// ```
///
/// **Queue dispatch** - for bare-metal ISRs where you need fast enqueue:
/// ```rust,ignore
/// // In ISR (fast - just enqueue):
/// critical_section::with(|cs| {
///     APP.borrow_ref_mut(cs).enqueue(Action::ButtonPressed).ok();
/// });
///
/// // In main loop (process all queued actions):
/// critical_section::with(|cs| {
///     let effects = APP.borrow_ref_mut(cs).process_queue();
///     for effect in effects {
///         // handle effect...
///     }
/// });
/// ```
///
/// The queue pattern keeps ISRs fast by deferring the actual dispatch+render
/// to the main loop.
pub struct App<R, V, const Q: usize = 8>
where
    R: Reducer,
    V: View<State = R::State>,
{
    state: R::State,
    view: V,
    queue: heapless::Deque<R::Action, Q>,
    _reducer: PhantomData<R>,
}

impl<R, V, const Q: usize> App<R, V, Q>
where
    R: Reducer,
    V: View<State = R::State>,
{
    /// Create a new application with the given view and initial state.
    pub fn new(view: V, initial_state: R::State) -> Self {
        Self {
            state: initial_state,
            view,
            queue: heapless::Deque::new(),
            _reducer: PhantomData,
        }
    }

    /// Dispatch an action immediately through the reducer.
    ///
    /// The view is automatically rendered if the effect indicates state changed
    /// (i.e., `effect.is_unchanged()` returns false).
    ///
    /// Returns the Effect from the reducer for side effect handling.
    ///
    /// Use this when actions come from an async channel (embassy pattern).
    pub fn dispatch(&mut self, action: R::Action) -> R::Effect {
        let effect = R::reduce(&mut self.state, action);
        if !effect.is_unchanged() {
            self.view.render(&self.state);
        }
        effect
    }

    /// Enqueue an action for later processing.
    ///
    /// Use this from ISRs or interrupt handlers where you want to keep
    /// execution time minimal. The action will be processed when
    /// `process_queue()` is called from the main loop.
    ///
    /// Returns `Err(action)` if the queue is full.
    ///
    /// Note: For ISR safety, wrap the App in `critical_section::Mutex<RefCell<App>>`.
    pub fn enqueue(&mut self, action: R::Action) -> Result<(), R::Action> {
        self.queue.push_back(action)
    }

    /// Process all queued actions.
    ///
    /// Dispatches each queued action through the reducer, rendering after
    /// each state change. Returns a list of effects for the main loop to handle.
    ///
    /// Call this from your main loop after ISRs have enqueued actions.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// loop {
    ///     // Wait for interrupt or timeout...
    ///
    ///     let effects = app.process_queue();
    ///     for effect in effects {
    ///         match effect {
    ///             AppEffect::Save => storage::save(app.state()),
    ///             _ => {}
    ///         }
    ///     }
    /// }
    /// ```
    pub fn process_queue(&mut self) -> heapless::Vec<R::Effect, Q> {
        let mut effects = heapless::Vec::new();
        while let Some(action) = self.queue.pop_front() {
            let effect = self.dispatch(action);
            effects.push(effect).ok();
        }
        effects
    }

    /// Check if the action queue is empty.
    pub fn is_queue_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Get a reference to the current state.
    pub fn state(&self) -> &R::State {
        &self.state
    }

    /// Get a mutable reference to the view.
    pub fn view(&mut self) -> &mut V {
        &mut self.view
    }
}
