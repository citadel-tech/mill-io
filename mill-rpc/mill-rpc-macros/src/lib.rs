use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, FnArg, Ident, ItemTrait, Pat, ReturnType, TraitItem, Type};

/// Attribute macro for defining an RPC service.
///
/// Applied to a trait, it generates:
/// - `{Name}Server` trait with `&self, ctx: &RpcContext` prepended to each method
/// - `{Name}Client` struct with typed RPC call methods
/// - `{Name}Dispatcher<T>` wrapper that implements `ServiceDispatch`
/// - Per-method request/response types with serde derives
///
/// # Example
///
/// ```ignore
/// #[mill_rpc::service]
/// trait Calculator {
///     fn add(a: i32, b: i32) -> i32;
///     fn divide(a: f64, b: f64) -> Result<f64, String>;
/// }
/// ```
#[proc_macro_attribute]
pub fn service(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemTrait);
    match generate_service(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

struct MethodInfo {
    name: Ident,
    method_id: u16,
    args: Vec<(Ident, Type)>,
    return_type: Type,
}

fn generate_service(input: ItemTrait) -> syn::Result<proc_macro2::TokenStream> {
    let trait_name = &input.ident;
    let server_trait_name = format_ident!("{}Server", trait_name);
    let client_struct_name = format_ident!("{}Client", trait_name);
    let dispatcher_name = format_ident!("{}Dispatcher", trait_name);
    let methods_mod_name = format_ident!("{}_methods", to_snake_case(&trait_name.to_string()));

    // Parse methods from the trait
    let mut methods = Vec::new();
    for (idx, item) in input.items.iter().enumerate() {
        let method = match item {
            TraitItem::Fn(m) => m,
            _ => continue,
        };

        let name = method.sig.ident.clone();

        // Extract arguments (skip &self if present, though we don't expect it)
        let mut args = Vec::new();
        for arg in &method.sig.inputs {
            match arg {
                FnArg::Typed(pat_type) => {
                    let pat = &*pat_type.pat;
                    let ident = match pat {
                        Pat::Ident(pi) => pi.ident.clone(),
                        _ => {
                            return Err(syn::Error::new_spanned(
                                pat,
                                "Expected a simple identifier pattern for argument",
                            ))
                        }
                    };
                    let ty = (*pat_type.ty).clone();
                    args.push((ident, ty));
                }
                FnArg::Receiver(_) => {
                    return Err(syn::Error::new_spanned(
                        arg,
                        "#[mill_rpc::service] trait methods should not have `self` parameter",
                    ));
                }
            }
        }

        // Extract return type
        let return_type = match &method.sig.output {
            ReturnType::Default => syn::parse_quote!(()),
            ReturnType::Type(_, ty) => (**ty).clone(),
        };

        methods.push(MethodInfo {
            name,
            method_id: idx as u16,
            args,
            return_type,
        });
    }

    // Generate method ID constants
    let method_consts: Vec<_> = methods
        .iter()
        .map(|m| {
            let const_name = format_ident!("{}", m.name.to_string().to_uppercase());
            let id = m.method_id;
            quote! { pub const #const_name: u16 = #id; }
        })
        .collect();

    // Generate per-method request/response structs
    let mut type_defs = Vec::new();
    for m in &methods {
        let req_name = format_ident!("{}Request", to_pascal_case(&m.name.to_string()));
        let resp_name = format_ident!("{}Response", to_pascal_case(&m.name.to_string()));
        let ret_ty = &m.return_type;

        let field_names: Vec<_> = m.args.iter().map(|(name, _)| name).collect();
        let field_types: Vec<_> = m.args.iter().map(|(_, ty)| ty).collect();

        let req_struct = if m.args.is_empty() {
            quote! {
                #[derive(::serde::Serialize, ::serde::Deserialize, Debug)]
                pub struct #req_name;
            }
        } else {
            quote! {
                #[derive(::serde::Serialize, ::serde::Deserialize, Debug)]
                pub struct #req_name {
                    #( pub #field_names: #field_types, )*
                }
            }
        };

        type_defs.push(quote! {
            #req_struct

            #[derive(::serde::Serialize, ::serde::Deserialize, Debug)]
            pub struct #resp_name(pub #ret_ty);
        });
    }

    // Generate Server trait
    let server_methods: Vec<_> = methods
        .iter()
        .map(|m| {
            let name = &m.name;
            let ret_ty = &m.return_type;
            let arg_names: Vec<_> = m.args.iter().map(|(n, _)| n).collect();
            let arg_types: Vec<_> = m.args.iter().map(|(_, t)| t).collect();

            quote! {
                fn #name(&self, ctx: &::mill_rpc_core::RpcContext, #( #arg_names: #arg_types ),*) -> #ret_ty;
            }
        })
        .collect();

    // Generate dispatcher match arms
    let dispatch_arms: Vec<_> = methods
        .iter()
        .map(|m| {
            let name = &m.name;
            let const_name = format_ident!("{}", m.name.to_string().to_uppercase());
            let req_name = format_ident!("{}Request", to_pascal_case(&m.name.to_string()));
            let resp_name = format_ident!("{}Response", to_pascal_case(&m.name.to_string()));

            let field_names: Vec<_> = m.args.iter().map(|(n, _)| n).collect();

            let call_args = if m.args.is_empty() {
                quote! {}
            } else {
                let args: Vec<_> = field_names.iter().map(|n| quote! { req.#n }).collect();
                quote! { , #( #args ),* }
            };

            quote! {
                #methods_mod_name::#const_name => {
                    let req: #req_name = codec.deserialize(args)?;
                    let result = self.0.#name(ctx #call_args);
                    codec.serialize(&#resp_name(result))
                }
            }
        })
        .collect();

    // Generate client methods
    let client_methods: Vec<_> = methods
        .iter()
        .map(|m| {
            let name = &m.name;
            let ret_ty = &m.return_type;
            let const_name = format_ident!("{}", m.name.to_string().to_uppercase());
            let req_name = format_ident!("{}Request", to_pascal_case(&m.name.to_string()));
            let resp_name = format_ident!("{}Response", to_pascal_case(&m.name.to_string()));

            let arg_names: Vec<_> = m.args.iter().map(|(n, _)| n).collect();
            let arg_types: Vec<_> = m.args.iter().map(|(_, t)| t).collect();

            let req_construct = if m.args.is_empty() {
                quote! { #req_name }
            } else {
                quote! { #req_name { #( #arg_names: #arg_names, )* } }
            };

            quote! {
                pub fn #name(&self, #( #arg_names: #arg_types ),*) -> Result<#ret_ty, ::mill_rpc_core::RpcError> {
                    let req = #req_construct;
                    let payload = self.codec.serialize(&req)?;
                    let resp_bytes = self.transport.call(
                        self.service_id,
                        #methods_mod_name::#const_name,
                        payload,
                    )?;
                    let resp: #resp_name = self.codec.deserialize(&resp_bytes)?;
                    Ok(resp.0)
                }
            }
        })
        .collect();

    // Count methods for service registration
    let method_count = methods.len() as u16;
    let service_name_str = trait_name.to_string();

    let output = quote! {
        /// Method ID constants for this service.
        pub mod #methods_mod_name {
            #( #method_consts )*
        }

        #( #type_defs )*

        /// Server trait - implement this to handle RPC calls.
        pub trait #server_trait_name: Send + Sync + 'static {
            #( #server_methods )*
        }

        /// Service descriptor for registration.
        pub struct #trait_name;

        impl #trait_name {
            pub const SERVICE_NAME: &'static str = #service_name_str;
            pub const METHOD_COUNT: u16 = #method_count;
        }

        /// Wrapper that adapts a `{Name}Server` impl into a `ServiceDispatch`.
        ///
        /// This avoids orphan-rule violations by being a local concrete type.
        pub struct #dispatcher_name<T: #server_trait_name>(pub T);

        impl<T: #server_trait_name> ::mill_rpc_core::ServiceDispatch for #dispatcher_name<T> {
            fn dispatch(
                &self,
                ctx: &::mill_rpc_core::RpcContext,
                method_id: u16,
                args: &[u8],
                codec: &::mill_rpc_core::Codec,
            ) -> Result<Vec<u8>, ::mill_rpc_core::RpcError> {
                match method_id {
                    #( #dispatch_arms, )*
                    _ => Err(::mill_rpc_core::RpcError::method_not_found(method_id)),
                }
            }
        }

        /// Generated client for calling this service remotely.
        pub struct #client_struct_name {
            transport: ::std::sync::Arc<dyn ::mill_rpc_core::RpcTransport>,
            codec: ::mill_rpc_core::Codec,
            service_id: u16,
        }

        impl #client_struct_name {
            /// Create a new client from a transport, codec, and service ID.
            pub fn new(
                transport: ::std::sync::Arc<dyn ::mill_rpc_core::RpcTransport>,
                codec: ::mill_rpc_core::Codec,
                service_id: u16,
            ) -> Self {
                Self { transport, codec, service_id }
            }

            #( #client_methods )*
        }
    };

    Ok(output)
}

fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(ch.to_lowercase().next().unwrap());
        } else {
            result.push(ch);
        }
    }
    result
}

fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
            }
        })
        .collect()
}
