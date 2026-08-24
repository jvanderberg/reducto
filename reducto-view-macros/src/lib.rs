use proc_macro::TokenStream;

mod codegen;
mod parse;

/// View composition macro.
///
/// Generates a struct that implements `reducto::View` by composing components.
///
/// # Example
///
/// ```ignore
/// view! {
///     TodoView<D> for TodoState {
///         <Header />
///         @if state.todos.is_empty() { <EmptyMessage /> }
///         @match state.filter {
///             Filter::All => <AllTodos />,
///             Filter::Active => <ActiveTodos />,
///             Filter::Completed => <CompletedTodos />,
///         }
///         <Footer />
///     }
/// }
/// ```
#[proc_macro]
pub fn view(input: TokenStream) -> TokenStream {
    let view_macro = syn::parse_macro_input!(input as parse::ViewMacro);
    codegen::generate(&view_macro).into()
}
