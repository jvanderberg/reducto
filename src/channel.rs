//! Async action channel for embassy-based applications.
//!
//! This module provides [`ActionChannel`], an ISR-safe async channel for
//! sending actions from interrupt handlers to your main loop.
//!
//! # Overview
//!
//! In embedded systems, interrupt handlers need to be fast. The typical pattern
//! is to enqueue work in the ISR and process it later in the main loop.
//! `ActionChannel` provides this with async/await ergonomics.
//!
//! # Example
//!
//! ```rust,ignore
//! use reducto::{App, ActionChannel};
//!
//! // Static channel - lives for the entire program
//! static ACTIONS: ActionChannel<Action, 8> = ActionChannel::new();
//!
//! // Interrupt handler - fast, just enqueue
//! #[interrupt]
//! fn BUTTON_IRQ() {
//!     // try_send is non-blocking and ISR-safe
//!     ACTIONS.try_send(Action::ButtonPressed).ok();
//! }
//!
//! #[interrupt]
//! fn TIMER_IRQ() {
//!     ACTIONS.try_send(Action::Tick).ok();
//! }
//!
//! // Main loop - process actions with async/await
//! #[embassy_executor::main]
//! async fn main(_spawner: Spawner) {
//!     let mut app = App::new(MyView::new(), MyState::default());
//!
//!     loop {
//!         // Awaits until an action is available
//!         let action = ACTIONS.receive().await;
//!         let effect = app.dispatch(action);
//!
//!         match effect {
//!             AppEffect::Save => storage::save(app.state()),
//!             AppEffect::Beep => buzzer::beep(),
//!             _ => {}
//!         }
//!     }
//! }
//! ```
//!
//! # API Summary
//!
//! | Method | Blocking | Use Case |
//! |--------|----------|----------|
//! | `try_send()` | No | ISRs - fast, never blocks |
//! | `send().await` | Async | Tasks - waits if channel full |
//! | `try_receive()` | No | Polling - returns `None` if empty |
//! | `receive().await` | Async | Main loop - waits for next action |
//!
//! # Channel Capacity
//!
//! The second type parameter `N` sets the channel capacity. If ISRs produce
//! actions faster than the main loop consumes them, `try_send()` will fail
//! when the channel is full. Size accordingly:
//!
//! ```rust,ignore
//! // Small - for low-frequency events
//! static ACTIONS: ActionChannel<Action, 4> = ActionChannel::new();
//!
//! // Larger - for burst-heavy workloads
//! static ACTIONS: ActionChannel<Action, 32> = ActionChannel::new();
//! ```
//!
//! # Dependencies
//!
//! This module requires the `embassy` feature:
//!
//! ```toml
//! reducto = { version = "0.1", features = ["embassy"] }
//! ```
//!
//! You'll also need an embassy executor for your platform:
//!
//! ```toml
//! embassy-executor = { version = "0.7", features = ["arch-cortex-m"] }
//! ```

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;

/// An ISR-safe async channel for dispatching actions.
///
/// This is a type alias for [`embassy_sync::channel::Channel`] with the mutex
/// type fixed to [`CriticalSectionRawMutex`], which is safe to use from
/// interrupt handlers on any platform that supports `critical-section`.
///
/// # Type Parameters
///
/// - `A` - The action type (your action enum)
/// - `N` - Channel capacity (max queued actions)
///
/// # Example
///
/// ```rust,ignore
/// use reducto::ActionChannel;
///
/// #[derive(Clone)]
/// enum Action {
///     Increment,
///     Decrement,
///     SetValue(i32),
/// }
///
/// static ACTIONS: ActionChannel<Action, 8> = ActionChannel::new();
///
/// // From ISR:
/// ACTIONS.try_send(Action::Increment).ok();
///
/// // From async main:
/// let action = ACTIONS.receive().await;
/// ```
pub type ActionChannel<A, const N: usize> = Channel<CriticalSectionRawMutex, A, N>;
