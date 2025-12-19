//! # Reducto
//!
//! A `no_std` Redux-like state management framework for embedded systems.
//!
//! ## Design Principles
//!
//! 1. **Pure reducers** - State transformation with no side effects
//! 2. **Exhaustive matching** - Rust's type system enforces handling all action variants
//! 3. **Framework-owned loop** - The `Application` trait provides a bullet-proof main loop
//! 4. **Testable** - `TextView` enables testing views without hardware
//!
//! ## Example
//!
//! ```rust
//! use reducto::{Outcome, Reducer, Store};
//!
//! #[derive(Clone, PartialEq, Default)]
//! struct AppState { count: i32 }
//!
//! enum Action { Increment, Decrement }
//!
//! struct AppReducer;
//!
//! impl Reducer for AppReducer {
//!     type State = AppState;
//!     type Action = Action;
//!
//!     fn reduce(mut state: Self::State, action: Self::Action) -> Outcome<Self::State> {
//!         match action {
//!             Action::Increment => { state.count += 1; Outcome::changed(state) }
//!             Action::Decrement => { state.count -= 1; Outcome::changed(state) }
//!         }
//!     }
//! }
//!
//! let mut store: Store<AppState, Action> = Store::new(AppState::default());
//! store.dispatch::<AppReducer>(Action::Increment);
//! assert_eq!(store.state().count, 1);
//! ```

#![no_std]

use core::marker::PhantomData;
use heapless::String;

/// Result of a reducer - indicates whether state changed.
///
/// Most actions change state, so `Changed` is the common case.
/// Use `Unchanged` to short-circuit when an action is a no-op.
///
/// # Example
///
/// ```rust,ignore
/// fn reduce(state: State, action: Action) -> Outcome<State> {
///     match action {
///         Action::Increment => Outcome::changed(State { count: state.count + 1 }),
///         Action::SetValue(v) if v == state.value => Outcome::unchanged(state),
///         Action::SetValue(v) => Outcome::changed(State { value: v, ..state }),
///     }
/// }
/// ```
#[derive(Debug)]
pub enum Outcome<S> {
    /// State was modified - triggers on_state_change callback
    Changed(S),
    /// State was not modified - callback is skipped
    Unchanged(S),
}

impl<S> Outcome<S> {
    /// Create a Changed outcome (most common case)
    pub fn changed(state: S) -> Self {
        Outcome::Changed(state)
    }

    /// Extract the state and whether it changed
    pub fn into_parts(self) -> (S, bool) {
        match self {
            Outcome::Changed(s) => (s, true),
            Outcome::Unchanged(s) => (s, false),
        }
    }
}

/// Mark state as changed. Use in reducer if/else branches:
/// ```rust,ignore
/// if condition { changed(new_state) } else { unchanged(state) }
/// ```
pub fn changed<S>(state: S) -> Outcome<S> {
    Outcome::Changed(state)
}

/// Mark state as unchanged. Use in reducer if/else branches:
/// ```rust,ignore
/// if condition { unchanged(state) } else { changed(new_state) }
/// ```
pub fn unchanged<S>(state: S) -> Outcome<S> {
    Outcome::Unchanged(state)
}


/// Trait for automatic conversion to Outcome.
///
/// This enables the reducer macro to accept either:
/// - A bare state value (auto-wrapped as `Outcome::Changed`)
/// - An explicit `Outcome` (passed through)
pub trait IntoOutcome<S> {
    fn into_outcome(self) -> Outcome<S>;
}

// Bare state -> Changed
impl<S> IntoOutcome<S> for S {
    fn into_outcome(self) -> Outcome<S> {
        Outcome::Changed(self)
    }
}

// Outcome<S> -> pass through unchanged
impl<S> IntoOutcome<S> for Outcome<S> {
    fn into_outcome(self) -> Outcome<S> {
        self
    }
}

/// Pure state transformation: (State, Action) -> Outcome<State>
///
/// Implement this trait to define how actions transform state.
/// Rust's exhaustive `match` on the Action enum ensures all variants are handled.
///
/// Return `Outcome::changed(new_state)` when state is modified, or
/// `Outcome::unchanged(state)` to short-circuit no-op actions.
pub trait Reducer {
    /// The state type this reducer operates on
    type State;
    /// The action type this reducer handles
    type Action;

    /// Transform state based on an action.
    ///
    /// This must be a pure function - no side effects allowed.
    /// The same inputs must always produce the same output.
    ///
    /// Return `Outcome::changed()` for most actions, `Outcome::unchanged()`
    /// to skip the on_state_change callback.
    fn reduce(state: Self::State, action: Self::Action) -> Outcome<Self::State>;
}

/// Store holds application state and an action queue.
///
/// The Store is responsible for:
/// - Holding the current state
/// - Queueing actions (typically from ISRs)
/// - Dispatching queued actions through the reducer
/// - Reporting whether state changed
///
/// # Queue Design
///
/// ISRs and other event sources call `enqueue()` to add actions.
/// The main loop calls `process_queue_with_callback()` to drain all
/// queued actions and handle state changes.
///
/// # Example
///
/// ```rust,ignore
/// // In ISR:
/// store.enqueue(Action::ButtonPressed).ok();
///
/// // In main loop:
/// store.process_queue_with_callback::<MyReducer, _>(|old, new| {
///     display.render(new);
/// });
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
    /// Returns `true` if the reducer returned `Outcome::changed()`.
    /// Prefer `process_queue_with_callback()` for normal operation.
    ///
    /// Note: State is moved to reducer (zero-copy). Rust's ownership
    /// system guarantees immutability - no cloning needed.
    pub fn dispatch<R>(&mut self, action: A) -> bool
    where
        R: Reducer<State = S, Action = A>,
        S: Default,
    {
        let (new_state, changed) = R::reduce(core::mem::take(&mut self.state), action).into_parts();
        self.state = new_state;
        changed
    }

    /// Process all queued actions, returning the number of state changes.
    ///
    /// This drains the queue and dispatches each action through the reducer.
    /// Use `process_queue_with_callback()` if you need to react to each change.
    pub fn process_queue<R>(&mut self) -> usize
    where
        R: Reducer<State = S, Action = A>,
        S: Default,
    {
        let mut changes = 0;
        while let Some(action) = self.queue.pop_front() {
            let (new_state, changed) = R::reduce(core::mem::take(&mut self.state), action).into_parts();
            self.state = new_state;
            if changed {
                changes += 1;
            }
        }
        changes
    }

    /// Process all queued actions, calling `on_change` for each state change.
    ///
    /// This is the primary method for the main loop. It drains the queue,
    /// dispatches each action, and calls the callback when reducer returns
    /// `Outcome::changed()`.
    ///
    /// Zero-copy: state is moved to reducer, not cloned. Rust's ownership
    /// guarantees no external mutation.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// store.process_queue_with_callback::<MyReducer, _>(|new| {
    ///     display.render(new);
    /// });
    /// ```
    pub fn process_queue_with_callback<R, F>(&mut self, mut on_change: F)
    where
        R: Reducer<State = S, Action = A>,
        S: Default,
        F: FnMut(&S),
    {
        while let Some(action) = self.queue.pop_front() {
            let (new_state, changed) = R::reduce(core::mem::take(&mut self.state), action).into_parts();
            self.state = new_state;
            if changed {
                on_change(&self.state);
            }
        }
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

/// Application trait for framework-owned main loop.
///
/// Implement this trait to define your application's behavior. The framework
/// provides the main loop structure, ensuring correct ordering:
/// 1. `tick()` is called every iteration
/// 2. All queued actions are processed through the reducer
/// 3. For each state change, `view().render(state)` is called automatically
///
/// Actions are queued via `Store::enqueue()` from ISRs or event handlers.
/// The framework drains the queue automatically.
///
/// # Example
///
/// ```rust,ignore
/// struct MyApp {
///     view: AppView,
/// }
///
/// impl Application for MyApp {
///     type State = AppState;
///     type Action = Action;
///     type Reducer = AppReducer;
///     type View = AppView;
///
///     fn view(&mut self) -> &mut Self::View {
///         &mut self.view
///     }
/// }
///
/// // In button ISR:
/// store.enqueue(Action::ButtonPressed).ok();
/// ```
pub trait Application {
    /// The state type for this application
    type State: Default;
    /// The action type for this application
    type Action;
    /// The reducer that handles state transitions
    type Reducer: Reducer<State = Self::State, Action = Self::Action>;
    /// The root view that renders state
    type View: View<State = Self::State>;

    /// Return the root view for rendering.
    ///
    /// The framework calls `view().render(state)` when state changes.
    fn view(&mut self) -> &mut Self::View;

    /// Called every loop iteration regardless of state changes.
    ///
    /// Override this for periodic tasks like animation updates
    /// or watchdog feeds. Default implementation does nothing.
    fn tick(&mut self) {}
}

/// Result of a dispatch operation with access to old and new state.
///
/// This enables side effects that need to detect state transitions
/// by comparing old and new state.
pub struct Dispatch<'a, S> {
    /// State before the action was dispatched
    pub old: &'a S,
    /// State after the action was dispatched
    pub new: &'a S,
    /// Whether the reducer reported a state change
    pub changed: bool,
}

/// An application that owns its store and view.
///
/// This is the recommended way to structure an application. The `App` owns
/// the state store and view, providing a clean `dispatch()` API.
///
/// Uses double-buffering for state, allowing side effects to compare
/// old and new state without cloning.
///
/// # Example
///
/// ```rust,ignore
/// let mut app = App::<AppState, Action, AppReducer, _>::new(view, AppState::new());
/// app.dispatch(Action::Boot);
///
/// loop {
///     let action = channel.receive().await;
///     let result = app.dispatch(action);
///     if result.changed {
///         // Can compare result.old and result.new for side effects
///     }
/// }
/// ```
pub struct App<S, A, R, V>
where
    S: Default + Clone,
    R: Reducer<State = S, Action = A>,
    V: View<State = S>,
{
    /// Double-buffered state: [0] = current, [1] = previous
    states: [S; 2],
    /// Index of current state (0 or 1)
    current: usize,
    view: V,
    _reducer: PhantomData<R>,
    _action: PhantomData<A>,
}

impl<S, A, R, V> App<S, A, R, V>
where
    S: Default + Clone,
    R: Reducer<State = S, Action = A>,
    V: View<State = S>,
{
    /// Create a new application with the given view and initial state.
    pub fn new(view: V, initial_state: S) -> Self {
        Self {
            states: [initial_state, S::default()],
            current: 0,
            view,
            _reducer: PhantomData,
            _action: PhantomData,
        }
    }

    /// Dispatch an action through the reducer, rendering if state changed.
    ///
    /// Returns a `Dispatch` struct with references to old and new state,
    /// enabling side effects that need to detect state transitions.
    ///
    /// The view is automatically rendered if state changed.
    pub fn dispatch(&mut self, action: A) -> Dispatch<'_, S> {
        let old_idx = self.current;
        let new_idx = 1 - old_idx;

        // Clone current state to new buffer, keep original for comparison
        self.states[new_idx] = self.states[old_idx].clone();

        // Take from new buffer for reducer (old buffer preserved)
        let state_for_reducer = core::mem::take(&mut self.states[new_idx]);
        let (new_state, changed) = R::reduce(state_for_reducer, action).into_parts();
        self.states[new_idx] = new_state;

        // Swap current pointer
        self.current = new_idx;

        if changed {
            self.view.render(&self.states[new_idx]);
        }

        Dispatch {
            old: &self.states[old_idx],
            new: &self.states[new_idx],
            changed,
        }
    }

    /// Get a reference to the current state.
    pub fn state(&self) -> &S {
        &self.states[self.current]
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
///     ReducerName for State, Action {
///         Action::Variant1 => |state| new_state_expr,
///         Action::Variant2(val) => |state| new_state_expr_using_val,
///     }
/// }
/// ```
///
/// Each arm's body is automatically wrapped in `Outcome::changed()`.
/// For no-op short-circuits, return `Outcome::unchanged(state)` explicitly.
///
/// # Example
///
/// ```rust
/// use reducto::{Outcome, Reducer, Store};
///
/// #[derive(Clone, PartialEq, Default)]
/// struct Counter { count: i32 }
///
/// #[derive(Clone)]
/// enum CounterAction {
///     Increment,
///     Decrement,
///     Set(i32),
/// }
///
/// reducto::reducer! {
///     CounterReducer for Counter, CounterAction {
///         CounterAction::Increment => |state| Counter { count: state.count + 1 },
///         CounterAction::Decrement => |state| Counter { count: state.count - 1 },
///         CounterAction::Set(n) => |_state| Counter { count: n },
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
        $reducer_name:ident for $state_type:ty, $action_type:ty {
            $( $pattern:pat => |$var:ident| $body:expr ),* $(,)?
        }
    ) => {
        struct $reducer_name;

        impl $crate::Reducer for $reducer_name {
            type State = $state_type;
            type Action = $action_type;

            #[allow(unused_variables, unused_mut)]
            fn reduce(state: Self::State, action: Self::Action) -> $crate::Outcome<Self::State> {
                match action {
                    $(
                        $pattern => {
                            let mut $var = state;
                            $crate::IntoOutcome::into_outcome($body)
                        }
                    ),*
                }
            }
        }
    };
}

/// Process one iteration of the application loop.
///
/// This is the core logic used by `run_loop`. Call this in tests to
/// simulate loop iterations without blocking forever.
///
/// Each call:
/// 1. Calls `app.tick()`
/// 2. Drains all queued actions through the reducer
/// 3. Calls `view().render()` for each state change
///
/// Returns the number of renders (state changes) that occurred.
pub fn process_iteration<A: Application, const Q: usize>(
    app: &mut A,
    store: &mut Store<A::State, A::Action, Q>,
) -> usize {
    app.tick();
    let mut renders = 0;
    while let Some(action) = store.pop_action() {
        if store.dispatch::<A::Reducer>(action) {
            app.view().render(store.state());
            renders += 1;
        }
    }
    renders
}

/// Run the application main loop.
///
/// This function never returns (indicated by `-> !`). It calls
/// `process_iteration` in an infinite loop.
///
/// For testing, use `process_iteration` directly instead.
pub fn run_loop<A: Application, const Q: usize>(
    app: &mut A,
    store: &mut Store<A::State, A::Action, Q>,
) -> ! {
    loop {
        process_iteration(app, store);
    }
}
