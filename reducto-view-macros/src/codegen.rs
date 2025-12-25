use proc_macro2::TokenStream;
use quote::quote;

use crate::parse::{MatchArm, ViewMacro, ViewNode};

pub fn generate(view: &ViewMacro) -> TokenStream {
    let name = &view.name;
    let generic = &view.generic;
    let state_type = &view.state_type;
    let body = generate_body(&view.body);

    // Generate bounds clause if present
    let bounds_clause = if let Some(bounds) = &view.bounds {
        quote! { : #bounds }
    } else {
        quote! {}
    };

    quote! {
        struct #name<#generic> {
            display: #generic,
        }

        impl<#generic #bounds_clause> #name<#generic> {
            fn new(display: #generic) -> Self {
                Self { display }
            }

            fn display(&self) -> &#generic {
                &self.display
            }

            fn display_mut(&mut self) -> &mut #generic {
                &mut self.display
            }
        }

        impl<#generic #bounds_clause> reducto::View for #name<#generic> {
            type State = #state_type;

            fn render(&mut self, state: &Self::State) {
                #body
            }
        }
    }
}

fn generate_body(nodes: &[ViewNode]) -> TokenStream {
    let stmts: Vec<TokenStream> = nodes.iter().map(generate_node).collect();
    quote! { #(#stmts)* }
}

fn generate_node(node: &ViewNode) -> TokenStream {
    match node {
        ViewNode::Component { name, .. } => {
            quote! {
                #name::render(&mut self.display, state);
            }
        }

        ViewNode::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            let then_body = generate_body(then_branch);
            if let Some(else_nodes) = else_branch {
                let else_body = generate_body(else_nodes);
                quote! {
                    if #condition {
                        #then_body
                    } else {
                        #else_body
                    }
                }
            } else {
                quote! {
                    if #condition {
                        #then_body
                    }
                }
            }
        }

        ViewNode::IfLet {
            pattern,
            expr,
            then_branch,
            ..
        } => {
            let body = generate_body(then_branch);
            quote! {
                if let #pattern = #expr {
                    #body
                }
            }
        }

        ViewNode::Match { expr, arms, .. } => {
            let match_arms: Vec<TokenStream> = arms.iter().map(generate_match_arm).collect();
            quote! {
                match #expr {
                    #(#match_arms)*
                }
            }
        }
    }
}

fn generate_match_arm(arm: &MatchArm) -> TokenStream {
    let pattern = &arm.pattern;
    let body = generate_body(&arm.body);
    quote! {
        #pattern => { #body }
    }
}
