use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::ToTokens;
use syn::parse_macro_input;

#[proc_macro]
pub fn aoc_main(input: TokenStream) -> TokenStream {
    (parse_macro_input!(input as aoc_harness_macros_impl::AocMainInput))
        .do_macro()
        .into()
}

#[proc_macro]
pub fn aoc_all_main(input: TokenStream) -> TokenStream {
    (parse_macro_input!(input as aoc_harness_macros_impl::all::AocAllMainInput))
        .do_macro()
        .into()
}

#[proc_macro]
pub fn find_most_recent(input: TokenStream) -> TokenStream {
    (parse_macro_input!(input as aoc_harness_macros_impl::latest::FindLatestInput))
        .do_macro()
        .into()
}

#[proc_macro_attribute]
pub fn if_file_exist(args: TokenStream, input: TokenStream) -> TokenStream {
    // error if we are missing all arguments
    if args.is_empty() {
        return syn::Error::new_spanned(
            TokenStream2::from(args),
            "expected argument file is missing",
        )
        .into_compile_error()
        .into();
    }

    let file_name = parse_macro_input!(args as syn::LitStr).value();
    if std::fs::metadata(&file_name).is_ok() {
        input
    } else {
        let mut func = parse_macro_input!(input as syn::ItemFn);
        func.block.stmts = vec![];
        func.to_token_stream().into()
    }
}
