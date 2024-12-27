extern crate proc_macro;
use proc_macro::TokenStream;

mod schema;

#[proc_macro_derive(Schema, attributes(search))]
pub fn derive_schema(input: TokenStream) -> TokenStream {
    schema::derive_proc_macro_impl(input)
}
