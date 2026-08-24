# Reducto Reactive Architecture

## Status and intent

This document is the normative architecture contract for Reducto and its
applications. Code that conflicts with it is an architectural defect, even if
it appears to work on current hardware.

Reducto follows the PocketPD reactive architecture: one state tree, immutable
actions, a pure reducer, old/new-state side effects, and state-derived views.
For slow embedded displays, Reducto additionally passes both old and new state
to the view. This lets the view perform exact, local change detection without
introducing a second UI state tree.

## Invariants

### One source of truth

All durable application facts live in one application state. State implements
`Clone + PartialEq`, and dispatch compares the complete old/new values.

- A real state transition must compare unequal to its input.
- A no-op must compare equal to its input.
- Change detection does not depend on reducer authors maintaining a separate
  version token.
- Hardware driver objects, display buffers, queues, and transient I/O progress
  do not belong in application state.

### Actions describe facts

An action is an immutable value describing what happened, with any data needed
to reduce it.

Good: `MeasurementsSampled(sample)`, `OutputButtonPressed`, `Tick(now)`.

Bad: `RedrawTemperature`, `WriteEnablePin`, `PaintRegion(3)`.

Actions may originate in polling code or interrupt handlers. Interrupt handlers
should enqueue bounded work and return promptly; they must not render or run a
large reducer.

### Reducers are pure

The required shape is:

```rust
fn reduce(old: &AppState, action: Action) -> AppState
```

The reducer may copy the old value into a local `next`, transform that copy, and
return it. Given the same old state and action, it must produce the same result.

If the action makes no change, the reducer returns an equivalent state. If it
changes application state, it returns a value that compares unequal.

Reducers decide only application truth. They do not decide what the display
looks like or which hardware operation should run.

### Dispatch owns the transition

Dispatch computes `new = reduce(old, action)` and compares states. On a real
transition it exposes the same `(old, new)` pair to transition consumers and to
the view. On a no-op it performs no rendering or side effects.

The state flow is:

```text
event -> Action -> reduce(old, action) -> new
                                      |
                           old != new?
                         /              \
                       no               yes
                       |                 |
                    do nothing       effects(old,new)
                                     view(old,new)
```

Queue coalescing may reduce several actions and supply the view with the state
before the batch and the state after the batch. It must not alter reducer
semantics.

Coalescing must not be used for safety or other non-coalescible effects: an
intermediate transition can disappear when the batch's final state equals its
initial state. Effectful applications dispatch each queued action individually.

### Views derive pixels from state

A view has two entry points:

```rust
fn render(&mut self, state: &AppState);
fn render_transition(&mut self, old: &AppState, new: &AppState);
```

`render` performs a complete initial or recovery draw. `render_transition`
compares old and new values as they are actually projected onto the screen and
repaints only affected widgets or rectangles.

The view is the correct place for display-aware comparison because only it
knows formatting, layout, colors, and pixel boundaries. For example, if raw
temperature changes but both values format as `72.3 F`, the temperature widget
must perform zero display I/O.

This does not make the view stateful. The display driver may own transport and
framebuffer resources, but the view must not cache an independent copy of prior
application values. The old state supplied by dispatch is the comparison base.

### Side effects observe transitions

Hardware I/O, persistence, timers, protocol requests, logging, and other world
interactions are transition consumers. Applications with external effects use
`EffectApp`, configured with a `TransitionEffect` implementation that compares
old/new semantic state and returns a typed effect value. `EffectApp` has no
plain dispatch path that bypasses planning. Dispatch returns the planned value
to the caller, which performs the operation only after dispatch has returned.

Effect planning is expected to be pure; Rust cannot enforce purity of a static
method that can access globals. Reviews and tests must reject planners that do
I/O. Effect execution is structurally outside dispatch.

Effects are not reducer return values and are not action names. Reducers do not
dispatch effects. A side-effect layer must not mutate application state behind
the reducer; results come back later as actions.

## Explicit prohibitions

The following patterns are prohibited:

1. Reducers returning `Effect`, `RenderDamage`, dirty rectangles, widget IDs,
   or any other rendering instruction.
2. Render policies that translate reducer output into screen regions.
3. Reducers performing GPIO, SPI, USB, ADC, flash, EEPROM, timing, allocation,
   logging, or display I/O.
4. Views dispatching actions, changing application state, or controlling
   non-display hardware.
5. Views maintaining shadow copies of application fields for dirty checking.
6. Poll loops unconditionally redrawing because a sample or timer arrived.
7. Treating every sampled raw-value difference as a visible display change.
8. Using screen coordinates, region masks, colors, fonts, or formatted strings
   in reducers.
9. Updating the current state outside dispatch/reduction.
10. Hiding semantically relevant state from `PartialEq`, or implementing
    equality inconsistently with application truth.
11. Blocking USB interrupt service while painting the display or polling slow
    peripherals.
12. Performing unbounded work, rendering, or synchronous peripheral traffic in
    an interrupt handler.

These remain prohibited even when they reduce code size or seem convenient.

## Contraindications and boundaries

This model is not the right abstraction for every byte of firmware:

- High-rate sample streams that are never application state should use a
  dedicated bounded buffer or signal-processing pipeline, then dispatch only
  meaningful aggregate results.
- DMA descriptors, USB endpoint state machines, SPI transfer progress, and ISR
  mailboxes are transport state. Keep them in drivers, not `AppState`.
- A framebuffer is a rendering resource, not a second application state. It may
  be used when RAM and latency justify it, but correctness must not depend on
  reading business state back from pixels.
- Extremely large state values may make whole-value copying inappropriate. A
  structurally shared or arena-backed immutable representation is acceptable if
  it preserves the reducer and old/new semantics. In-place mutation that loses
  the old value before observers run is not acceptable.
- Animations and continuous graphs can intentionally repaint on time actions,
  because time changes their displayed projection. They still must be bounded,
  state-derived, and scheduled outside interrupts.

## Embedded scheduling and USB safety

Rendering is lower priority than maintaining USB transport. Application code
must preserve these properties:

- USB interrupt handling remains enabled during display and sensor work.
- Interrupt handlers move bounded data through fixed-capacity queues.
- Command processing and response buffering are bounded; overload returns an
  explicit busy/error response instead of corrupting state or blocking.
- Slow display work is split into the smallest useful changed widgets.
- A full render is explicit and reserved for initialization, view changes, or
  recovery from lost display contents.
- Firmware startup must preserve product-specific bootloader recovery. For
  BenchVolt, the application invalidates only the boot metadata page before
  other initialization so a power cycle returns to the stock bootloader.
- Navigation work may be brought up before output control, but read-only UI
  actions must remain structurally incapable of enabling hardware.

## BenchVolt projection rules

BenchVolt validates selective rendering using exactly what the screen shows:

- Temperature compares validity plus the formatted one-decimal Celsius
  value, not raw ADC counts.
- Voltage and current compare their displayed scaled integers, not hidden lower
  precision bits.
- Channel status compares the displayed status category (`FAULT`, `ON`, `WAIT`,
  or `OFF`), not every internal enum discriminant when the text/color is equal.
- Setpoint and limit fields compare their displayed scaled values.
- Each channel field is an independent widget. A voltage change must not clear
  or redraw current, setpoint, limit, channel number, status, or the rest of its
  row.
- Recovery/header status is independently repaintable.
- If none of those projections changes, `render_transition` performs no SPI
  writes even when application state changed.

Each widget comparison and its rectangle live together in the view. This keeps
formatting and layout cohesive and keeps the reducer display-agnostic.

## Testing requirements

Every application should test at least:

1. The reducer does not modify its input state.
2. A no-op compares equal and causes no transition or effect planning.
3. A real change compares unequal and supplies the exact old/new pair.
4. Multiple queued actions can coalesce into one old/final transition.
5. Raw changes that format identically cause zero widget draws.
6. Each visible projection change redraws only its widget.
7. Full render draws every required widget.
8. Side effects run only for relevant old/new semantic changes.
9. Sustained rendering does not break or starve USB CDC traffic.
10. The target's bootloader recovery mechanism still works after flashing.

## Learnings captured from BenchVolt

- A state change and a visible change are different facts. State equality
  answers the first; the view's projection comparison answers the second.
- Dirty regions derived in reducers couple domain logic to one screen and drift
  as formatting evolves.
- An `Effect` returned by a reducer conflates state transition, external work,
  and rendering. Old/new transition observers express those concerns directly.
- A cached `ViewState` duplicates application truth and creates invalidation
  bugs. Supplying old/new state makes that cache unnecessary.
- Full-screen redraws are visibly slow on the ST7789 and can delay foreground
  command handling. Partial widget redraws are both a UI quality requirement and
  a responsiveness requirement.
- USB correctness cannot rely on a fast main loop. The USB stack must retain
  interrupt service and bounded queues even while foreground rendering is slow.
- Device recovery is part of firmware correctness, not merely a flashing
  procedure. BenchVolt images must never erase or rewrite the stock bootloader,
  option bytes, protection state, or unrelated flash pages.

## Review checklist

Before merging a state/UI change, answer all of these affirmatively:

- Is every application mutation represented by an action and reducer result?
- Is the reducer deterministic and free of I/O and layout knowledge?
- Does a no-op compare equal to its input?
- Are side effects derived from old/new semantic state outside the reducer?
- Does the view compare old/new displayed projections without a shadow state?
- Does unchanged screen output result in zero display traffic?
- Is slow work outside interrupt context and unable to starve USB?
- Are full redraw and bootloader-affecting operations explicit and justified?
