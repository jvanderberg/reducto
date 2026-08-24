use proc_macro2::Span;
use syn::{
    braced,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    Expr, Ident, Pat, Token, Type, TypeParamBound,
};

/// Root AST node for the view! macro
pub struct ViewMacro {
    pub name: Ident,
    pub generic: Ident,
    pub bounds: Option<Punctuated<TypeParamBound, Token![+]>>,
    pub state_type: Type,
    pub body: Vec<ViewNode>,
}

/// A node in the view tree
#[allow(dead_code)]
pub enum ViewNode {
    /// `<ComponentName />`
    Component { name: Ident, span: Span },

    /// `@if condition { ... }` with optional `@else { ... }`
    If {
        condition: Expr,
        then_branch: Vec<ViewNode>,
        else_branch: Option<Vec<ViewNode>>,
        span: Span,
    },

    /// `@if let pattern = expr { ... }`
    IfLet {
        pattern: Pat,
        expr: Expr,
        then_branch: Vec<ViewNode>,
        span: Span,
    },

    /// `@match expr { arms... }`
    Match {
        expr: Expr,
        arms: Vec<MatchArm>,
        span: Span,
    },
}

/// A single arm in a @match expression
pub struct MatchArm {
    pub pattern: Pat,
    pub body: Vec<ViewNode>,
}

impl Parse for ViewMacro {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Parse: Name<D> or Name<D: Bound + Bound> for StateType { body }
        let name: Ident = input.parse()?;
        input.parse::<Token![<]>()?;
        let generic: Ident = input.parse()?;

        // Check for optional bounds: D: Bound + Bound
        let bounds = if input.peek(Token![:]) {
            input.parse::<Token![:]>()?;
            Some(Punctuated::parse_separated_nonempty(input)?)
        } else {
            None
        };

        input.parse::<Token![>]>()?;
        input.parse::<Token![for]>()?;
        let state_type: Type = input.parse()?;

        let content;
        braced!(content in input);

        let body = parse_view_body(&content)?;

        Ok(ViewMacro {
            name,
            generic,
            bounds,
            state_type,
            body,
        })
    }
}

fn parse_view_body(input: ParseStream) -> syn::Result<Vec<ViewNode>> {
    let mut nodes = Vec::new();
    while !input.is_empty() {
        nodes.push(parse_view_node(input)?);
    }
    Ok(nodes)
}

fn parse_view_node(input: ParseStream) -> syn::Result<ViewNode> {
    if input.peek(Token![@]) {
        // Control flow: @if, @match
        input.parse::<Token![@]>()?;

        // Check for keywords (if, match) which can't be parsed as Ident
        if input.peek(Token![if]) {
            input.parse::<Token![if]>()?;
            parse_if_node(input)
        } else if input.peek(Token![match]) {
            input.parse::<Token![match]>()?;
            parse_match_node(input)
        } else {
            // Try parsing as ident for @else
            let keyword: Ident = input.parse()?;
            Err(syn::Error::new(
                keyword.span(),
                format!("expected 'if' or 'match' after @, found '{}'", keyword),
            ))
        }
    } else if input.peek(Token![<]) {
        // Component: <Name />
        parse_component(input)
    } else {
        Err(syn::Error::new(
            input.span(),
            "expected <Component /> or @if/@match",
        ))
    }
}

fn parse_component(input: ParseStream) -> syn::Result<ViewNode> {
    let span = input.span();
    input.parse::<Token![<]>()?;
    let name: Ident = input.parse()?;
    input.parse::<Token![/]>()?;
    input.parse::<Token![>]>()?;

    Ok(ViewNode::Component { name, span })
}

fn parse_if_node(input: ParseStream) -> syn::Result<ViewNode> {
    let span = input.span();

    // Check for "let" (if let pattern)
    if input.peek(Token![let]) {
        input.parse::<Token![let]>()?;
        let pattern = Pat::parse_single(input)?;
        input.parse::<Token![=]>()?;
        let expr: Expr = input.parse()?;

        let content;
        braced!(content in input);
        let then_branch = parse_view_body(&content)?;

        Ok(ViewNode::IfLet {
            pattern,
            expr,
            then_branch,
            span,
        })
    } else {
        // Regular if - parse condition expression
        // We need to parse until we hit a brace
        let condition: Expr = input.parse()?;

        let content;
        braced!(content in input);
        let then_branch = parse_view_body(&content)?;

        // Check for @else
        let else_branch = if input.peek(Token![@]) {
            let fork = input.fork();
            fork.parse::<Token![@]>()?;
            if fork.peek(Token![else]) {
                // Consume the @else
                input.parse::<Token![@]>()?;
                input.parse::<Token![else]>()?;
                let else_content;
                braced!(else_content in input);
                Some(parse_view_body(&else_content)?)
            } else {
                None
            }
        } else {
            None
        };

        Ok(ViewNode::If {
            condition,
            then_branch,
            else_branch,
            span,
        })
    }
}

fn parse_match_node(input: ParseStream) -> syn::Result<ViewNode> {
    let span = input.span();
    let expr: Expr = input.parse()?;

    let content;
    braced!(content in input);

    let mut arms = Vec::new();
    while !content.is_empty() {
        arms.push(parse_match_arm(&content)?);
    }

    Ok(ViewNode::Match { expr, arms, span })
}

fn parse_match_arm(input: ParseStream) -> syn::Result<MatchArm> {
    let pattern = Pat::parse_multi_with_leading_vert(input)?;
    input.parse::<Token![=>]>()?;

    // Body can be single component or braced block
    let body = if input.peek(Token![<]) {
        vec![parse_component(input)?]
    } else {
        let content;
        braced!(content in input);
        parse_view_body(&content)?
    };

    // Consume optional trailing comma
    let _ = input.parse::<Token![,]>();

    Ok(MatchArm { pattern, body })
}
