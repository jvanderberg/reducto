# Changelog

All notable changes to Reducto are documented here.

## 0.1.0 - 2026-08-23

- Introduced a pure `Reducer` contract over immutable old state.
- Added full-state equality change detection to prevent missed transitions.
- Added `EffectApp`, typed transition-effect planning, and post-dispatch effect
  execution.
- Added explicit full and old/new transition rendering.
- Added bounded foreground action processing and optional Embassy channels.
- Established `no_std` support and MIT licensing.
