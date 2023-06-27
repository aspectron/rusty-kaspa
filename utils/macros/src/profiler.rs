use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::quote;
use std::collections::hash_map::DefaultHasher;
use std::convert::Into;
use std::hash::{Hash, Hasher};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    Error, Expr, ExprLit, Lit, Result, Token,
};

#[derive(Debug)]
struct Sample {
    name: String,
    name_expr: Expr,
    content: TokenStream2,
}

impl Parse for Sample {
    fn parse(input: ParseStream) -> Result<Self> {
        let parsed = Punctuated::<Expr, Token![,]>::parse_terminated(input);
        if parsed.is_err() {
            return Err(Error::new(Span::call_site(), "usage: sample!(\"<sample name>\", { <code> })".to_string()));
        }
        let parsed = parsed.unwrap();
        if parsed.len() != 2 {
            return Err(Error::new_spanned(parsed, "usage: sample!(\"<sample name>\", { <code> })".to_string()));
        }

        let mut iter = parsed.iter();

        let name_expr = iter.next().unwrap().clone();
        let name = match &name_expr {
            Expr::Lit(ExprLit { lit: Lit::Str(name), .. }) => name,
            _ => {
                return Err(Error::new_spanned(
                    name_expr,
                    "the first argument should be a static string literal (\"name\")".to_string(),
                ));
            }
        };

        let content_expr = iter.next();
        if content_expr.is_none() {
            return Err(Error::new_spanned(parsed, "usage: sample!(\"<sample name>\", { <code> })".to_string()));
        }

        let content_expr = content_expr.unwrap().clone();
        let expr_block = match &content_expr {
            Expr::Block(expr_block) => expr_block,
            _ => {
                return Err(Error::new_spanned(content_expr, "the second argument must be code block { <code> }".to_string()));
            }
        };

        let stmts = &expr_block.block.stmts;
        let content = quote! {
            #(#stmts)*
        };

        let name = quote! {#name};
        let handlers = Sample { name: name.to_string().to_ascii_lowercase(), name_expr, content };
        Ok(handlers)
    }
}

pub fn sample(input: TokenStream) -> TokenStream {
    let sample = parse_macro_input!(input as Sample);
    let content = sample.content;

    let mut hasher = DefaultHasher::new();
    sample.name.hash(&mut hasher);
    let sampler_id = hasher.finish();

    let sampler_var = Ident::new(&format!("sampler_{sampler_id}"), Span::call_site());
    let result_var = Ident::new(&format!("result_{sampler_id}"), Span::call_site());
    let name_expr = sample.name_expr.clone();

    let output = quote! {
        let #sampler_var = kaspa_utils::profiler::start_sampling();
        let #result_var = { #content };
        kaspa_utils::profiler::stop_sampling(#name_expr, #sampler_id, #sampler_var);
        #result_var
    };

    output.into()
}
