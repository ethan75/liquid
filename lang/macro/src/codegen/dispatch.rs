// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::{
    codegen::{utils, GenerateCode},
    ir::{Contract, FnArg, Function, FunctionKind},
};

use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, quote_spanned};
use syn::{punctuated::Punctuated, spanned::Spanned, Token};

pub struct Dispatch<'a> {
    contract: &'a Contract,
}

impl<'a> From<&'a Contract> for Dispatch<'a> {
    fn from(contract: &'a Contract) -> Self {
        Self { contract }
    }
}

impl<'a> GenerateCode for Dispatch<'a> {
    fn generate_code(&self) -> TokenStream2 {
        let marker = self.generate_external_fn_marker();
        let traits = self.generate_external_fn_traits();
        let dispatch = self.generate_dispatch();
        let entry_point = self.generate_entry_point();

        quote! {
            #[cfg(not(test))]
            const _: () = {
                #marker
                #traits
                #dispatch
                #entry_point
            };
        }
    }
}

fn generate_input_idents(
    args: &Punctuated<FnArg, Token![,]>,
) -> (Vec<&proc_macro2::Ident>, TokenStream2) {
    let input_idents = args
        .iter()
        .skip(1)
        .filter_map(|arg| match arg {
            FnArg::Typed(ident_type) => Some(&ident_type.ident),
            _ => None,
        })
        .collect::<Vec<_>>();

    let pat_idents = if input_idents.is_empty() {
        quote! { _ }
    } else {
        quote! { (#(#input_idents,)*) }
    };

    (input_idents, pat_idents)
}

impl<'a> Dispatch<'a> {
    fn generate_external_fn_marker(&self) -> TokenStream2 {
        quote! {
            pub struct FnMarker<S> {
                marker: core::marker::PhantomData<fn() -> S>,
            }

            pub struct TyMappingHelper<S, T> {
                marker_s: core::marker::PhantomData<fn() -> S>,
                marker_t: core::marker::PhantomData<fn() -> T>,
            }
        }
    }

    fn generate_external_fn_traits(&self) -> TokenStream2 {
        let traits = self
            .contract
            .functions
            .iter()
            .filter(|func| matches!(&func.kind, FunctionKind::External(_)))
            .map(|func| self.generate_external_fn_trait(func));

        quote! {
            #(#traits)*
        }
    }

    fn generate_external_fn_trait(&self, func: &Function) -> TokenStream2 {
        let fn_id = match &func.kind {
            FunctionKind::External(fn_id) => fn_id,
            _ => unreachable!(),
        };

        let span = func.span();
        let fn_marker = quote! { FnMarker<[(); #fn_id]> };
        let sig = &func.sig;

        let input_tys = utils::generate_input_tys(sig, true);
        let input_ty_checker = utils::generate_ty_checker(input_tys.as_slice());
        let fn_input = quote_spanned! { sig.inputs.span() =>
            impl liquid_lang::FnInput for #fn_marker  {
                type Input = #input_ty_checker;
            }
        };

        let output = &sig.output;
        let output_ty_checker = match output {
            syn::ReturnType::Default => quote_spanned! { output.span() => ()},
            syn::ReturnType::Type(_, ty) => {
                let return_ty = &*ty;
                quote_spanned! { output.span() =>
                    <#return_ty as liquid_lang::You_Should_Use_An_Valid_Return_Type>::T
                }
            }
        };
        let fn_output = quote_spanned! { output.span() =>
            impl liquid_lang::FnOutput for #fn_marker {
                type Output = #output_ty_checker;
            }
        };

        let selectors = utils::generate_ty_mapping(*fn_id, &sig.ident, &input_tys);
        let is_mut = sig.is_mut();
        let mutability = quote_spanned! { span =>
            impl liquid_lang::FnMutability for #fn_marker {
                const IS_MUT: bool = #is_mut;
            }
        };

        quote_spanned! { span =>
            #fn_input
            #fn_output
            #selectors
            #mutability
            impl liquid_lang::ExternalFn for #fn_marker {}
        }
    }

    fn generate_dispatch_fragment(
        &self,
        func: &Function,
        is_getter: bool,
    ) -> TokenStream2 {
        let fn_id = match &func.kind {
            FunctionKind::External(fn_id) => fn_id,
            _ => return quote! {},
        };
        let namespace = quote! { FnMarker<[(); #fn_id]> };

        let sig = &func.sig;
        let fn_name = &sig.ident;
        let (input_idents, pat_idents) = generate_input_idents(&sig.inputs);
        let attr = if is_getter {
            quote! { #[allow(deprecated)] }
        } else {
            quote! {}
        };

        quote! {
            if selector == <#namespace as liquid_lang::FnSelector>::SELECTOR {
                let #pat_idents = <<#namespace as liquid_lang::FnInput>::Input as liquid_abi_codec::Decode>::decode(&mut data.as_slice())
                    .map_err(|_| liquid_lang::DispatchError::InvalidParams)?;
                #attr
                let result = storage.#fn_name(#(#input_idents,)*);

                if <#namespace as liquid_lang::FnMutability>::IS_MUT {
                    <Storage as liquid_core::storage::Flush>::flush(&mut storage);
                }

                if core::any::TypeId::of::<<#namespace as liquid_lang::FnOutput>::Output>() != core::any::TypeId::of::<()>() {
                    liquid_core::env::finish(&result);
                }

                return Ok(());
            }
        }
    }

    fn generate_constr_input_ty_checker(&self) -> TokenStream2 {
        let constr = &self.contract.constructor;
        let sig = &constr.sig;
        let inputs = &sig.inputs;
        let input_tys = utils::generate_input_tys(sig, true);
        let marker = quote! { FnMarker<[(); 0]> };
        let input_ty_checker = utils::generate_ty_checker(input_tys.as_slice());
        quote_spanned! { inputs.span() =>
            impl liquid_lang::FnInput for #marker  {
                type Input = #input_ty_checker;
            }
        }
    }

    fn generate_dispatch(&self) -> TokenStream2 {
        let fragments = self.contract.functions.iter().enumerate().map(|(i, func)| {
            let is_getter = self.contract.functions.len() - i
                <= self.contract.storage.public_fields.len();
            self.generate_dispatch_fragment(func, is_getter)
        });

        let constr_input_ty_checker = self.generate_constr_input_ty_checker();

        quote! {
            #constr_input_ty_checker

            impl Storage {
                pub fn dispatch() -> liquid_lang::DispatchResult {
                    let mut storage = <Storage as liquid_core::storage::New>::new();
                    let call_data = liquid_core::env::get_call_data(liquid_core::env::CallMode::Call)
                        .map_err(|_| liquid_lang::DispatchError::CouldNotReadInput)?;
                    let selector = call_data.selector;
                    let data = call_data.data;

                    #(
                        #fragments
                    )*

                    Err(liquid_lang::DispatchError::UnknownSelector)
                }
            }
        }
    }

    #[cfg(feature = "std")]
    fn generate_entry_point(&self) -> TokenStream2 {
        quote!()
    }

    #[cfg(not(feature = "std"))]
    fn generate_entry_point(&self) -> TokenStream2 {
        let constr = &self.contract.constructor;
        let sig = &constr.sig;
        let input_tys = utils::generate_input_tys(sig, true);
        let ident = &sig.ident;
        let (input_idents, pat_idents) = generate_input_idents(&sig.inputs);

        quote! {
            #[no_mangle]
            fn hash_type() -> u32 {
                if cfg!(feature = "gm") {
                    1
                } else {
                    0
                }
            }

            #[no_mangle]
            fn deploy() {
                let mut storage = <Storage as liquid_core::storage::New>::new();
                let result = liquid_core::env::get_call_data(liquid_core::env::CallMode::Deploy);
                if let Ok(call_data) = result {
                    let data = call_data.data;
                    let result = <(#(#input_tys,)*) as liquid_abi_codec::Decode>::decode(&mut data.as_slice());
                    if let Ok(data) = result {
                        let #pat_idents = data;
                        storage.#ident(#(#input_idents,)*);
                        <Storage as liquid_core::storage::Flush>::flush(&mut storage);
                    } else {
                        liquid_core::env::revert(&String::from("invalid params"));
                    }
                } else {
                    liquid_core::env::revert(&String::from("could not read input"));
                }
            }

            #[no_mangle]
            fn main() {
                let ret_info = liquid_lang::DispatchRetInfo::from(Storage::dispatch());
                if !ret_info.is_success() {
                    liquid_core::env::revert(&ret_info.get_info_string());
                }
            }
        }
    }
}
