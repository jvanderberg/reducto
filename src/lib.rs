#![doc = include_str!("../README.md")]
#![no_std]

use core::marker::PhantomData;
use heapless::String;

#[cfg(feature = "embassy")]
mod channel;

#[cfg(feature = "embassy")]
pub use channel::ActionChannel;

/// A pure state transformation: `(old state, action) -> new state`.
pub trait Reducer {
    type State: Clone + PartialEq;
    type Action;

    fn reduce(state: &Self::State, action: Self::Action) -> Self::State;
}

/// Derives a typed external effect from a completed state transition.
///
/// Planning must be pure and bounded. Dispatch returns the planned value so
/// callers execute I/O only after dispatch and rendering have completed.
pub trait TransitionEffect<S> {
    type Effect;

    fn plan(old: &S, new: &S) -> Option<Self::Effect>;
}

/// Result of dispatching an action with typed effect planning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use]
pub struct DispatchOutcome<E> {
    changed: bool,
    effect: Option<E>,
}

impl<E> DispatchOutcome<E> {
    pub const fn changed(&self) -> bool {
        self.changed
    }

    pub fn effect(self) -> Option<E> {
        self.effect
    }

    pub fn into_parts(self) -> (bool, Option<E>) {
        (self.changed, self.effect)
    }
}

/// A stateless rendering function over application state.
///
/// `render` draws the complete initial view. `render_transition` receives both
/// states and may compare the values its widgets display, then repaint only the
/// affected rectangles. The default redraws the complete view.
pub trait View {
    type State;

    fn render(&mut self, state: &Self::State);

    fn render_transition(&mut self, old: &Self::State, new: &Self::State) {
        let _ = old;
        self.render(new);
    }
}

/// A fixed-capacity text buffer useful for views and tests.
pub struct TextView<const N: usize> {
    buffer: String<N>,
}

impl<const N: usize> TextView<N> {
    pub const fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    pub fn as_str(&self) -> &str {
        self.buffer.as_str()
    }

    pub fn contains(&self, value: &str) -> bool {
        self.buffer.as_str().contains(value)
    }

    pub fn buffer_mut(&mut self) -> &mut String<N> {
        &mut self.buffer
    }
}

impl<const N: usize> Default for TextView<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// Owns the current state, view, and a bounded action queue.
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
    pub fn new(view: V, initial_state: R::State) -> Self {
        Self {
            state: initial_state,
            view,
            queue: heapless::Deque::new(),
            _reducer: PhantomData,
        }
    }

    /// Draw the complete current state, normally once during initialization.
    pub fn render_full(&mut self) {
        self.view.render(&self.state);
    }

    /// Reduce one action and render its transition if the state changed.
    ///
    /// Returns `true` exactly when the reducer produced a changed state.
    pub fn dispatch(&mut self, action: R::Action) -> bool {
        let next = R::reduce(&self.state, action);
        let changed = next != self.state;
        if changed {
            self.view.render_transition(&self.state, &next);
        }
        self.state = next;
        changed
    }

    fn dispatch_planned<P>(&mut self, action: R::Action) -> DispatchOutcome<P::Effect>
    where
        P: TransitionEffect<R::State>,
    {
        let next = R::reduce(&self.state, action);
        let changed = next != self.state;
        let effect = if changed {
            P::plan(&self.state, &next)
        } else {
            None
        };
        if changed {
            self.view.render_transition(&self.state, &next);
        }
        self.state = next;
        DispatchOutcome { changed, effect }
    }

    /// Enqueue an action from foreground code.
    ///
    /// This method requires `&mut self` and is not itself ISR-safe. Use
    /// `ActionChannel` with the `embassy` feature, or move ISR-originated
    /// actions through a platform-specific critical-section queue.
    pub fn enqueue(&mut self, action: R::Action) -> Result<(), R::Action> {
        self.queue.push_back(action)
    }

    /// Process queued actions individually and return the number that changed state.
    pub fn process_queue(&mut self) -> usize {
        let mut changed = 0;
        while let Some(action) = self.queue.pop_front() {
            changed += usize::from(self.dispatch(action));
        }
        changed
    }

    /// Reduce the entire queue and expose one old/new transition to the view.
    ///
    /// This is for render coalescing in effect-free applications. Intermediate
    /// transitions can cancel, so safety-relevant effects must process each
    /// action through [`EffectApp::dispatch`] instead.
    pub fn process_queue_coalesced(&mut self) -> usize {
        let old = self.state.clone();
        let mut changed = 0;
        while let Some(action) = self.queue.pop_front() {
            let next = R::reduce(&self.state, action);
            changed += usize::from(next != self.state);
            self.state = next;
        }
        if self.state != old {
            self.view.render_transition(&old, &self.state);
        }
        changed
    }

    pub fn is_queue_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn state(&self) -> &R::State {
        &self.state
    }

    pub fn view(&self) -> &V {
        &self.view
    }

    /// Mutably access the view for scheduler-driven work such as draining a
    /// deferred render queue after dispatch returns.
    pub fn view_mut(&mut self) -> &mut V {
        &mut self.view
    }
}

/// An application whose every dispatch plans a typed external effect.
///
/// Unlike [`App`], this type has no effect-bypassing dispatch path. It is the
/// appropriate container when state transitions control hardware, storage, or
/// another safety-relevant external system.
pub struct EffectApp<R, V, P, const Q: usize = 8>
where
    R: Reducer,
    V: View<State = R::State>,
    P: TransitionEffect<R::State>,
{
    app: App<R, V, Q>,
    _planner: PhantomData<P>,
}

impl<R, V, P, const Q: usize> EffectApp<R, V, P, Q>
where
    R: Reducer,
    V: View<State = R::State>,
    P: TransitionEffect<R::State>,
{
    pub fn new(view: V, initial_state: R::State) -> Self {
        Self {
            app: App::new(view, initial_state),
            _planner: PhantomData,
        }
    }

    pub fn render_full(&mut self) {
        self.app.render_full();
    }

    /// Reduce, plan, and render one transition.
    ///
    /// Consume the returned effect only after this method returns.
    pub fn dispatch(&mut self, action: R::Action) -> DispatchOutcome<P::Effect> {
        self.app.dispatch_planned::<P>(action)
    }

    pub fn state(&self) -> &R::State {
        self.app.state()
    }

    pub fn view(&self) -> &V {
        self.app.view()
    }

    /// Mutably access the view without bypassing reducer or effect handling.
    /// This is intended for servicing work that the view deferred during
    /// `render` or `render_transition`.
    pub fn view_mut(&mut self) -> &mut V {
        self.app.view_mut()
    }
}
