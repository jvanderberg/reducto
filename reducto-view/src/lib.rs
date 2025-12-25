//! View composition DSL for reducto.
//!
//! This crate provides the `view!` macro for declarative view composition.
//!
//! # Example
//!
//! ```ignore
//! use reducto_view::view;
//!
//! view! {
//!     AppView<D> for AppState {
//!         <Header />
//!         @if state.loading { <Loading /> }
//!         <Footer />
//!     }
//! }
//! ```

pub use reducto_view_macros::view;
