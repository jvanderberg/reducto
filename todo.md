# Reducto TODO

## Completed

### Type Parameter Simplification ✓
`App<S, A, R, V, Q>` simplified to `App<R, V, Q>`. State and Action types derived from `R::State` and `R::Action`.

### reducer! Macro Removal ✓
Removed the `reducer!` macro and `IntoEffect` trait. Manual `Reducer` impl is cleaner and supports match guards.

### View Composition DSL ✓
Created `reducto-view` crate with `view!` macro:

```rust
use reducto_view::view;

view! {
    TodoBody<D: Write> for TodoState {
        <Header />
        @if state.todos.is_empty() { <EmptyMessage /> } @else { <TodoList /> }
        @match state.filter {
            Filter::All => <AllView />,
            Filter::Active => <ActiveView />,
        }
        <Footer />
    }
}
```

Generates struct + View impl. User wraps for setup/teardown (clear/flush).

---

## Remaining

### Middleware via Reducer Composition

The `Reducer` trait supports middleware through composition:

```rust
struct WithLogging<R>(PhantomData<R>);

impl<R: Reducer> Reducer for WithLogging<R>
where
    R::Action: Debug,
    R::Effect: Debug,
{
    type State = R::State;
    type Action = R::Action;
    type Effect = R::Effect;

    fn reduce(state: &mut Self::State, action: Self::Action) -> Self::Effect {
        log::debug!("action: {:?}", action);
        let effect = R::reduce(state, action);
        log::debug!("effect: {:?}", effect);
        effect
    }
}

// Usage:
type MyStack = WithLogging<WithAnalytics<MyReducer>>;
let app: App<MyStack, MyView> = App::new(...);
```

- [ ] Document middleware pattern in README/lib.rs
- [ ] Add example middleware (logging, timing)

### Effect::changed() Naming

The method `Effect::changed()` returns "the default effect when state changed" but reads like a query. Consider:
- `render()` - indicates "just render, no other side effects"
- `none()` - matches common `Effect::None` variant naming

### Unchanged Semantic Overload

`Effect::Unchanged` currently means both:
- "No-op, state already in desired state" (e.g., `Set(5)` when count is already 5)
- "Invalid action, nothing to do" (e.g., `Toggle(999)` for non-existent ID)

Options:
- [ ] Document this as intentional (keep it simple)
- [ ] Leave to user to define their own Effect variants for distinction

### Action Batching

Each `dispatch()` triggers a render. No way to batch multiple actions:

```rust
// Renders 3 times:
app.dispatch(Action::A);
app.dispatch(Action::B);
app.dispatch(Action::C);
```

Options:
- [ ] `dispatch_batch(&[actions])` that renders once at end
- [ ] `transaction(|app| { ... })` wrapper that defers render
- [ ] Document that users can call reducer directly and render manually if needed

### View Macro Enhancements

Current implementation supports `@if`, `@if let`, `@else`, `@match`. Future:
- [ ] Better error messages with span information
- [ ] Consider `@for` if props are ever added
