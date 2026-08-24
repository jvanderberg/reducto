# Reducto roadmap

This file tracks work against the current pure-reducer API. Historical mutable
reducer and reducer-returned-effect designs were removed before the 0.1 release.

## Before 0.1

- [ ] Benchmark clone and full-state equality costs on representative Cortex-M0
  application states.
- [ ] Add CI for formatting, Clippy, default/all-feature tests, doctests,
  `thumbv6m-none-eabi`, and package verification.
- [ ] Document release ordering for the optional view macro crates.

## Later

- [ ] Evaluate a derive helper for state equality/projection boilerplate.
- [ ] Improve `view!` diagnostics and add optional loop syntax.
- [ ] Add more platform examples for bounded ISR-to-foreground queues.
