#![crate_type = "proc-macro"]
extern crate proc_macro;
use proc_macro::TokenStream;
use quote::TokenStreamExt;
use syn::parse::{Parse, ParseStream, Result};
use syn::punctuated::Punctuated;

struct EntryDef(holochain_zome_types::entry_def::EntryDef);
struct EntryDefId(holochain_zome_types::entry_def::EntryDefId);
struct EntryVisibility(holochain_zome_types::entry_def::EntryVisibility);
struct CrdtType(holochain_zome_types::crdt::CrdtType);
struct RequiredValidations(holochain_zome_types::entry_def::RequiredValidations);

impl Parse for EntryDef {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut id = holochain_zome_types::entry_def::EntryDefId::default();
        let mut required_validations =
            holochain_zome_types::entry_def::RequiredValidations::default();
        let mut visibility = holochain_zome_types::entry_def::EntryVisibility::default();
        let crdt_type = holochain_zome_types::crdt::CrdtType::default();

        let vars = Punctuated::<syn::MetaNameValue, syn::Token![,]>::parse_terminated(input)?;
        for var in vars {
            match var.path.segments.first() {
                Some(segment) => {
                    match segment.ident.to_string().as_str() {
                        "id" => match var.lit {
                            syn::Lit::Str(s) => {
                                id = holochain_zome_types::entry_def::EntryDefId::from(s.value())
                            }
                            _ => unreachable!(),
                        },
                        "required_validations" => match var.lit {
                            syn::Lit::Int(i) => {
                                required_validations =
                                    holochain_zome_types::entry_def::RequiredValidations::from(
                                        i.base10_parse::<u8>()?,
                                    )
                            }
                            _ => unreachable!(),
                        },
                        "visibility" => {
                            match var.lit {
                                syn::Lit::Str(s) => visibility = match s.value().as_str() {
                                    "public" => {
                                        holochain_zome_types::entry_def::EntryVisibility::Public
                                    }
                                    "private" => {
                                        holochain_zome_types::entry_def::EntryVisibility::Private
                                    }
                                    _ => unreachable!(),
                                },
                                _ => unreachable!(),
                            };
                        }
                        "crdt_type" => {
                            unimplemented!();
                        }
                        _ => {}
                    }
                }
                None => {}
            }
        }
        Ok(EntryDef(holochain_zome_types::entry_def::EntryDef {
            id,
            required_validations,
            visibility,
            crdt_type,
        }))
    }
}

impl quote::ToTokens for CrdtType {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        tokens.append_all(quote::quote! {
            hdk3::prelude::CrdtType
        });
    }
}

impl quote::ToTokens for EntryDefId {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let s = String::from(&self.0);
        tokens.append_all(quote::quote! {
            hdk3::prelude::EntryDefId::from(#s)
        });
    }
}

impl quote::ToTokens for RequiredValidations {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let u = <u8>::from(self.0);
        tokens.append_all(quote::quote! {
            hdk3::prelude::RequiredValidations::from(#u)
        });
    }
}

impl quote::ToTokens for EntryVisibility {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let variant = syn::Ident::new(
            match self.0 {
                holochain_zome_types::entry_def::EntryVisibility::Public => "Public",
                holochain_zome_types::entry_def::EntryVisibility::Private => "Private",
            },
            proc_macro2::Span::call_site(),
        );
        tokens.append_all(quote::quote! {
            hdk3::prelude::EntryVisibility::#variant
        });
    }
}

impl quote::ToTokens for EntryDef {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let id = EntryDefId(self.0.id.clone());
        let visibility = EntryVisibility(self.0.visibility);
        let crdt_type = CrdtType(self.0.crdt_type);
        let required_validations = RequiredValidations(self.0.required_validations);

        tokens.append_all(quote::quote! {
            hdk3::prelude::EntryDef {
                id: #id,
                visibility: #visibility,
                crdt_type: #crdt_type,
                required_validations: #required_validations,
            }
        });
    }
}

#[proc_macro_attribute]
pub fn hdk_entry(attrs: TokenStream, code: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(code as syn::Item);

    let struct_ident = match item.clone() {
        syn::Item::Struct(item_struct) => item_struct.ident,
        syn::Item::Enum(item_enum) => item_enum.ident,
        _ => unimplemented!(),
    };
    let entry_def = syn::parse_macro_input!(attrs as EntryDef);

    (quote::quote! {
        #[derive(hdk3::prelude::Serialize, hdk3::prelude::Deserialize, hdk3::prelude::SerializedBytes)]
        #item
        hdk3::prelude::entry_def!(#struct_ident #entry_def);
    })
    .into()
}

#[proc_macro_attribute]
pub fn hdk_extern(_attrs: TokenStream, item: TokenStream) -> TokenStream {
    // extern mapping is only valid for functions
    // let mut item_fn: syn::ItemFn = syn::parse(item).unwrap();
    let mut item_fn = syn::parse_macro_input!(item as syn::ItemFn);

    // extract the ident of the fn
    // this will be exposed as the external facing extern
    let external_fn_ident = item_fn.sig.ident.clone();

    // build a new internal fn ident that is compatible with map_extern!
    // this needs to be sufficiently unlikely to have namespace collisions with other fns
    let internal_fn_ident = syn::Ident::new(
        &format!("{}_hdk_extern", external_fn_ident.to_string()),
        item_fn.sig.ident.span(),
    );

    // replace the ident in-place with the new internal ident
    item_fn.sig.ident = internal_fn_ident.clone();

    // add a map_extern! and include the modified item_fn
    (quote::quote! {
        map_extern!(#external_fn_ident, #internal_fn_ident);
        #item_fn
    })
    .into()
}
