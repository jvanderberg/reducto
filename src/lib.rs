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
//! use reducto::{Reducer, Store};
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
//!     fn reduce(state: Self::State, action: Self::Action) -> Self::State {
//!         match action {
//!             Action::Increment => AppState { count: state.count + 1 },
//!             Action::Decrement => AppState { count: state.count - 1 },
//!         }
//!     }
//! }
//!
//! let mut store: Store<AppState, Action> = Store::new(AppState::default());
//! store.dispatch::<AppReducer>(Action::Increment);
//! assert_eq!(store.state().count, 1);
//! ```

#![no_std]

use heapless::String;

/// Pure state transformation: (State, Action) -> State
///
/// Implement this trait to define how actions transform state.
/// Rust's exhaustive `match` on the Action enum ensures all variants are handled.
pub trait Reducer {
    /// The state type this reducer operates on
    type State;
    /// The action type this reducer handles
    type Action;

    /// Transform state based on an action.
    ///
    /// This must be a pure function - no side effects allowed.
    /// The same inputs must always produce the same output.
    fn reduce(state: Self::State, action: Self::Action) -> Self::State;
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

impl<S, A, const N: usize> Store<S, A, N>
where
    S: Clone + PartialEq,
{
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
        self.queue.push_back(action).map_err(|a| a)
    }

    /// Check if the action queue is empty.
    pub fn is_queue_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Dispatch a single action through the reducer immediately.
    ///
    /// Returns `true` if the state changed, `false` otherwise.
    /// Prefer `process_queue_with_callback()` for normal operation.
    pub fn dispatch<R>(&mut self, action: A) -> bool
    where
        R: Reducer<State = S, Action = A>,
    {
        let old = self.state.clone();
        self.state = R::reduce(self.state.clone(), action);
        self.state != old
    }

    /// Process all queued actions, returning the number of state changes.
    ///
    /// This drains the queue and dispatches each action through the reducer.
    /// Use `process_queue_with_callback()` if you need to react to each change.
    pub fn process_queue<R>(&mut self) -> usize
    where
        R: Reducer<State = S, Action = A>,
    {
        let mut changes = 0;
        while let Some(action) = self.queue.pop_front() {
            if self.dispatch::<R>(action) {
                changes += 1;
            }
        }
        changes
    }

    /// Process all queued actions, calling `on_change` for each state change.
    ///
    /// This is the primary method for the main loop. It drains the queue,
    /// dispatches each action, and calls the callback whenever state changes.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// store.process_queue_with_callback::<MyReducer, _>(|old, new| {
    ///     // Render, update LEDs, etc.
    ///     display.render(new);
    /// });
    /// ```
    pub fn process_queue_with_callback<R, F>(&mut self, mut on_change: F)
    where
        R: Reducer<State = S, Action = A>,
        F: FnMut(&S, &S),
    {
        while let Some(action) = self.queue.pop_front() {
            let old = self.state.clone();
            self.state = R::reduce(self.state.clone(), action);
            if self.state != old {
                on_change(&old, &self.state);
            }
        }
    }
}

/// View renders state to some output.
///
/// The View trait is kept separate from Store - the application decides
/// when to render, not the framework. This keeps side effects isolated.
///
/// Views receive a `TextView` buffer for rendering. For hardware displays,
/// the View implementation translates the text buffer to display commands.
/// For testing, you can inspect the `TextView` contents directly.
pub trait View {
    /// The state type this view renders
    type State;

    /// Render the state to the text view buffer.
    ///
    /// Called by the application when state changes and a render is needed.
    fn render(&mut self, view: &mut TextView<128>, state: &Self::State);
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
/// 3. For each state change, `on_state_change()` is called
///
/// Actions are queued via `Store::enqueue()` from ISRs or event handlers.
/// The framework drains the queue automatically.
///
/// This design is bullet-proof because:
/// - You can't forget to check for state changes (framework does it)
/// - Side effects are isolated to `on_state_change`
/// - The order is enforced by the framework
/// - ISRs just enqueue, main loop processes
///
/// # Example
///
/// ```rust,ignore
/// struct MyApp { display: Display }
///
/// impl Application for MyApp {
///     type State = AppState;
///     type Action = Action;
///     type Reducer = AppReducer;
///
///     fn on_state_change(&mut self, _old: &AppState, new: &AppState) {
///         self.display.render(new);
///     }
/// }
///
/// // In button ISR:
/// store.enqueue(Action::ButtonPressed).ok();
/// ```
pub trait Application {
    /// The state type for this application
    type State: Clone + PartialEq;
    /// The action type for this application
    type Action;
    /// The reducer that handles state transitions
    type Reducer: Reducer<State = Self::State, Action = Self::Action>;

    /// Execute side effects when state changes.
    ///
    /// Called ONLY when state actually changed after dispatch.
    /// This is where you should render to displays, update LEDs, etc.
    fn on_state_change(&mut self, old: &Self::State, new: &Self::State);

    /// Called every loop iteration regardless of state changes.
    ///
    /// Override this for periodic tasks like animation updates
    /// or watchdog feeds. Default implementation does nothing.
    fn tick(&mut self) {}
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
/// # Example
///
/// ```rust
/// use reducto::{Reducer, Store};
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
            $( $pattern:pat => |$state_var:ident| $body:expr ),* $(,)?
        }
    ) => {
        struct $reducer_name;

        impl $crate::Reducer for $reducer_name {
            type State = $state_type;
            type Action = $action_type;

            fn reduce(state: Self::State, action: Self::Action) -> Self::State {
                match action {
                    $( $pattern => {
                        let $state_var = state;
                        $body
                    } ),*
                }
            }
        }
    };
}

/// Run the application main loop.
///
/// This function never returns (indicated by `-> !`). It implements
/// the bullet-proof loop pattern:
///
/// ```text
/// loop {
///     app.tick()
///     for action in store.drain_queue() {
///         old = state.clone()
///         dispatch(action)
///         if state != old {
///             app.on_state_change(old, new)
///         }
///     }
/// }
/// ```
///
/// Actions are queued via `Store::enqueue()` from ISRs or event handlers.
///
/// # Note
///
/// For testing, you can simulate the loop manually rather than calling
/// this function, since it never returns.
pub fn run_loop<A: Application, const N: usize>(
    app: &mut A,
    store: &mut Store<A::State, A::Action, N>,
) -> ! {
    loop {
        app.tick();
        store.process_queue_with_callback::<A::Reducer, _>(|old, new| {
            app.on_state_change(old, new);
        });
    }
}
