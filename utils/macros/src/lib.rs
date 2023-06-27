use proc_macro::TokenStream;
use proc_macro_error::proc_macro_error;
mod profiler;

// the `sample!{} macro`
#[proc_macro]
#[proc_macro_error]
pub fn sample(input: TokenStream) -> TokenStream {
    profiler::sample(input)
}

// the function below is an example of how a macro
// with a different name can be used to invoke profiler::sample()
// with custom arguments to augment the macro's behavior
// (for example the resulting code it generates)
#[proc_macro]
#[proc_macro_error]
pub fn sample_some_outliers_and_some_such(input: TokenStream) -> TokenStream {
    profiler::sample(input)
}
