use darling::{ast::Data, FromDeriveInput, FromField, FromMeta};
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};

pub fn derive_proc_macro_impl(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input);
    StructConfig::from_derive_input(&input)
        .expect("Invalid Input")
        .to_token_stream()
        .into()
}

#[derive(FromDeriveInput)]
#[darling(attributes(search))]
#[darling(supports(struct_named))]
pub struct StructConfig {
    ident: syn::Ident,
    generics: syn::Generics,
    data: Data<(), FieldConfig>,

    #[darling(default, rename = "data")]
    doc_data: bool,
    data_fn: Option<syn::Path>,
    lang: Option<String>,
    #[darling(default)]
    index: bool,
    index_fn: Option<syn::Path>,
}

#[derive(Debug, Default, FromMeta, PartialEq)]
pub enum DocData {
    #[default]
    Display,
    Custom(syn::Path),
}

#[derive(FromMeta)]
pub enum DocId {
    Custom,
    Hash,
}

#[derive(Debug, FromField)]
#[darling(attributes(search))]
pub struct FieldConfig {
    ident: Option<syn::Ident>,

    alias: Option<String>,
    #[darling(default)]
    facet: bool,
    facet_fn: Option<syn::Path>,
    #[darling(default)]
    index: bool,
    index_fn: Option<syn::Path>,
    prefix: Option<String>,
}

impl ToTokens for StructConfig {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let StructConfig {
            ref ident,
            ref generics,
            ref data,
            ref doc_data,
            ref data_fn,
            ref lang,
            ref index,
            ref index_fn,
        } = *self;

        let to_string = syn::Path::from_string("::std::string::ToString::to_string").unwrap();
        let to_value = syn::Path::from_string("::search::xapian::ToValue::serialize").unwrap();
        let (impl_generics, ty_generics, conds) = generics.split_for_impl();

        let index_full = match (index, index_fn) {
            (_, Some(f)) => quote!(indexer.index_text::<&str>(#f(&self), None, None);),
            (true, None) => {
                quote!(indexer.index_text::<&str>(#to_string(&self), None, None);)
            }
            (_, None) => quote!(),
        };

        let (indexer_lang, parser_lang) = if let Some(lang) = lang {
            (
                // Language settings for Indexer
                quote! {
                    indexer.set_stemmer(::search::xapian::Stem::for_language(#lang));
                    indexer.set_stopper(::search::StopList::for_language(#lang).expect(&format!("Unsupported stopper language: {}", #lang)));
                },
                // Language settings for QueryParser
                quote! {
                    parser.set_stemmer(::search::xapian::Stem::for_language(#lang));
                    parser.set_stopper(::search::StopList::for_language(#lang).expect(&format!("Unsupported stopper language: {}", #lang)));
                },
            )
        } else {
            (quote!(), quote!())
        };

        let fields = &data
            .as_ref()
            .take_struct()
            .expect("#[derive(Search)] only supports structs")
            .fields;

        let facet_fields = fields.iter().filter(|f| f.facet || f.facet_fn.is_some());
        let index_fields = fields.iter().filter(|f| f.index || f.index_fn.is_some());

        let indexer_indexes = index_fields.clone().enumerate().map(|(idx, f)| {
            let index = {
                let ident = f.ident.as_ref().map_or_else(
                    || {
                        let ident = syn::Index::from(idx);
                        quote!(#ident)
                    },
                    |ident| quote!(#ident),
                );

                let func = match f.index_fn.as_ref() {
                    Some(f) => f,
                    None => &to_string,
                };

                match f.prefix.as_ref() {
                    Some(pfx) => quote! {
                        indexer.index_text(#func(&self.#ident), None, #pfx);
                    },
                    None => quote! {
                        indexer.index_text::<&str>(#func(&self.#ident), None, None);
                    },
                }
            };
            quote! {
                #index
                indexer.increase_termpos(None);
            }
        });

        let indexer_facets = facet_fields.enumerate().map(|(idx, f)| {
            let ident = f.ident.as_ref().map_or_else(
                || {
                    let ident = syn::Index::from(idx);
                    quote!(#ident)
                },
                |ident| quote!(#ident),
            );

            let f = match f.facet_fn.as_ref() {
                Some(f) => f,
                None => &to_value,
            };

            let idx = idx as u32;
            quote! {
                doc.set_value(#idx, #f(&self.#ident));
            }
        });

        let parser_indexes = index_fields.map(|f| {
            f.ident.as_ref().map(|ident| {
                let ident = ident.to_string();
                let prefix = f.prefix.clone().unwrap_or_default();
                let alias = if let Some(a) = f.alias.as_ref() {
                    quote! {
                        parser.add_prefix(#a, #prefix);
                    }
                } else {
                    quote!()
                };

                quote! {
                    parser.add_prefix(#ident, #prefix);
                    #alias
                }
            })
        });

        let doc_data = match (doc_data, data_fn) {
            (_, Some(f)) => quote!(doc.set_data(#f(&self));),
            (true, None) => quote!(doc.set_data(#to_string(&self));),
            (_, None) => quote!(),
        };

        tokens.extend(quote! {
            impl #impl_generics ::search::Schema for #ident #ty_generics #conds {
                fn query_parser() -> ::search::xapian::QueryParser {
                    let mut parser = ::search::xapian::QueryParser::default();
                    #parser_lang
                    #(#parser_indexes)*

                    parser
                }

                fn index(&self, indexer: &mut ::search::xapian::TermGenerator) -> ::search::xapian::Document {
                    #indexer_lang

                    let mut doc = ::search::xapian::Document::default();
                    indexer.set_document(&doc);
                    #(#indexer_facets)*
                    #(#indexer_indexes)*
                    #index_full
                    #doc_data

                    doc
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use darling::FromDeriveInput;

    #[test]
    fn test_index_struct() {
        {
            let input = syn::parse_str(
                r#"
            #[derive(Schema)]
            #[search(index)]
            struct Foo;
            "#,
            )
            .unwrap();

            let StructConfig { index, .. } = StructConfig::from_derive_input(&input).unwrap();
            assert!(index);
        }

        {
            let input = syn::parse_str(
                r#"
            #[derive(Schema)]
            #[search(index = true)]
            struct Foo;
            "#,
            )
            .unwrap();

            let StructConfig { index, .. } = StructConfig::from_derive_input(&input).unwrap();
            assert!(index);
        }

        {
            let input = syn::parse_str(
                r#"
            #[derive(Schema)]
            #[search(index = false)]
            struct Foo;
            "#,
            )
            .unwrap();

            let StructConfig { index, .. } = StructConfig::from_derive_input(&input).unwrap();
            assert!(!index);
        }
    }

    #[test]
    fn test_index() {
        {}
    }
}
