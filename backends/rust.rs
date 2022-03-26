use std::{env, io::Write, path::PathBuf, process::Command};

use super::super::{
    grammar::ItemPath,
    semantic_analysis::{
        self, MetadataValue, Region, SemanticState, TypeRef, TypeState, TypeStateResolved,
    },
};

use quote::quote;

fn str_to_ident(s: &str) -> syn::Ident {
    quote::format_ident!("{}", s)
}

fn fully_qualified_type_ref_impl(
    out: &mut String,
    type_ref: &TypeRef,
) -> Result<(), std::fmt::Error> {
    use std::fmt::Write;
    match type_ref {
        TypeRef::Raw(path) => {
            if path.len() == 1 && path.last() == Some(&"void".into()) {
                write!(out, "::std::ffi::c_void")
            } else {
                // todo: re-evaluate this hack
                if path.len() > 1 {
                    write!(out, "crate::")?;
                }
                write!(out, "{}", path)
            }
        }
        TypeRef::ConstPointer(tr) => {
            write!(out, "*const ")?;
            fully_qualified_type_ref_impl(out, tr.as_ref())
        }
        TypeRef::MutPointer(tr) => {
            write!(out, "*mut ")?;
            fully_qualified_type_ref_impl(out, tr.as_ref())
        }
        TypeRef::Array(tr, size) => {
            write!(out, "[")?;
            fully_qualified_type_ref_impl(out, tr.as_ref())?;
            write!(out, "; {}]", size)
        }
        TypeRef::Function(args, return_type) => {
            // todo: revisit the thiscall here when we have non-thiscall functions
            write!(out, r#"unsafe extern "thiscall" fn ("#)?;
            for (field, type_ref) in args.iter() {
                write!(out, "{field}: ")?;
                fully_qualified_type_ref_impl(out, type_ref)?;
                write!(out, ", ")?;
            }
            write!(out, ")")?;
            if let Some(type_ref) = return_type {
                write!(out, " -> ")?;
                fully_qualified_type_ref_impl(out, type_ref)?;
            }
            Ok(())
        }
    }
}

fn fully_qualified_type_ref(type_ref: &TypeRef) -> Result<String, std::fmt::Error> {
    let mut out = String::new();
    fully_qualified_type_ref_impl(&mut out, type_ref)?;
    Ok(out)
}

fn type_ref_to_syn_type(type_ref: &TypeRef) -> anyhow::Result<syn::Type> {
    Ok(syn::parse_str(&fully_qualified_type_ref(type_ref)?)?)
}

fn build_function(
    function: &semantic_analysis::Function,
    is_vftable: bool,
) -> Result<proc_macro2::TokenStream, anyhow::Error> {
    use semantic_analysis::{Argument, Attribute};

    let name = str_to_ident(&function.name);

    let arguments = function
        .arguments
        .iter()
        .map(|a| {
            Ok(match a {
                Argument::ConstSelf => quote! { &self },
                Argument::MutSelf => quote! { &mut self },
                Argument::Field(name, type_ref) => {
                    let name = str_to_ident(name);
                    let syn_type = type_ref_to_syn_type(type_ref)?;
                    quote! {
                        #name: #syn_type
                    }
                }
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let lambda_arguments = function
        .arguments
        .iter()
        .map(|a| {
            Ok(match a {
                Argument::ConstSelf => quote! { this: *const Self },
                Argument::MutSelf => quote! { this: *mut Self },
                Argument::Field(name, type_ref) => {
                    let name = str_to_ident(name);
                    let syn_type = type_ref_to_syn_type(type_ref)?;
                    quote! {
                        #name: #syn_type
                    }
                }
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let call_arguments = function
        .arguments
        .iter()
        .map(|a| {
            Ok(match a {
                Argument::ConstSelf => quote! { self as *const Self},
                Argument::MutSelf => quote! { self as *mut Self },
                Argument::Field(name, _) => {
                    let name = str_to_ident(name);
                    quote! { #name }
                }
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let return_type = function
        .return_type
        .as_ref()
        .map(|type_ref| -> anyhow::Result<proc_macro2::TokenStream> {
            let syn_type = type_ref_to_syn_type(type_ref)?;
            Ok(quote! { -> #syn_type })
        })
        .transpose()?;

    #[derive(Debug)]
    enum FunctionGetter {
        Address(usize),
        Vftable,
    }
    let mut function_getter = None;
    if is_vftable {
        function_getter = Some(FunctionGetter::Vftable);
    } else {
        for attribute in &function.attributes {
            let Attribute::Address(address) = attribute;
            if function_getter.is_some() {
                return Err(anyhow::anyhow!(
                    "function getter already set: {:?}",
                    function_getter
                ));
            }

            function_getter = Some(FunctionGetter::Address(*address));
        }
    }
    if function_getter.is_none() {
        return Err(anyhow::anyhow!("no function getter set"));
    }
    let function_getter_impl = function_getter.map(|fg| match fg {
        FunctionGetter::Address(address) => quote! {
            ::std::mem::transmute(#address)
        },
        FunctionGetter::Vftable => quote! {
            std::ptr::addr_of!((*self.vftable).#name).read()
        },
    });

    Ok(quote! {
        #[allow(dead_code)]
        pub unsafe fn #name(#(#arguments),*) #return_type {
            let f: unsafe extern "thiscall" fn(#(#lambda_arguments),*) #return_type = #function_getter_impl;
            f(#(#call_arguments),*)
        }
    })
}

fn write_type(
    semantic_state: &SemanticState,
    item_path: &ItemPath,
    out: &mut impl Write,
) -> Result<(), anyhow::Error> {
    let type_definition = semantic_state.type_registry().get(item_path).unwrap();
    if let (
        Some(name),
        TypeState::Resolved(TypeStateResolved {
            size,
            regions,
            functions,
            metadata,
        }),
    ) = (item_path.last(), &type_definition.state)
    {
        let fields = regions
            .iter()
            .enumerate()
            .map(|(i, r)| {
                Ok(match r {
                    Region::Field(field, type_ref) => {
                        let field_ident = str_to_ident(field);
                        let syn_type = type_ref_to_syn_type(type_ref)?;
                        quote! {
                            pub #field_ident: #syn_type
                        }
                    }
                    Region::Padding(size) => {
                        let field_ident = quote::format_ident!("padding_{}", i);
                        quote! {
                            #field_ident: [u8; #size]
                        }
                    }
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let name_ident = str_to_ident(name.as_str());
        let size_check_ident = quote::format_ident!("_{}_size_check", name.as_str());
        let size_check_impl = (*size > 0).then(|| {
            quote! {
                #[allow(non_snake_case)]
                fn #size_check_ident() {
                    unsafe {
                        ::std::mem::transmute::<_, #name_ident>([0u8; #size]);
                    }
                    unreachable!()
                }
            }
        });

        let singleton_impl = metadata
            .iter()
            .find(|(k, _)| k.as_str() == "singleton")
            .map(|(_, address)| match address {
                MetadataValue::Integer(ref address) => {
                    let address = *address as usize;
                    quote! {
                        impl #name_ident {
                            pub fn get() -> &'static mut Self {
                                unsafe {
                                    &mut **::std::mem::transmute::<usize, *mut *mut Self>(#address)
                                }
                            }
                        }
                    }
                }
            });

        let free_functions_impl = functions
            .get("free")
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|f| build_function(f, false))
            .collect::<anyhow::Result<Vec<_>>>()?;

        let vftable_function_impl = functions
            .get("vftable")
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|f| build_function(f, true))
            .collect::<anyhow::Result<Vec<_>>>()?;

        let body = quote! {
            #[repr(C)]
            pub struct #name_ident {
                #(#fields),*
            }
            #size_check_impl
            #singleton_impl
            impl #name_ident {
                #(#free_functions_impl)*
                #(#vftable_function_impl)*
            }
        };

        writeln!(out, "{}", body)?;
    };
    Ok(())
}

pub fn write_types<'a>(
    output: &mut impl Write,
    types: impl Iterator<Item = &'a ItemPath>,
    semantic_state: &SemanticState,
) -> Result<(), anyhow::Error> {
    for item_path in types {
        write_type(semantic_state, item_path, output)?;
    }

    Ok(())
}

pub fn write_module<'a>(
    key: ItemPath,
    types: impl Iterator<Item = &'a ItemPath>,
    semantic_state: &SemanticState,
) -> Result<(), anyhow::Error> {
    const FORMAT_OUTPUT: bool = true;

    let path = std::iter::once(env::var("OUT_DIR")?)
        .chain(key.iter().map(|s| s.as_str().to_string()))
        .collect::<PathBuf>()
        .with_extension("rs");
    let directory_path = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    std::fs::create_dir_all(&directory_path)?;

    let mut file = std::fs::File::create(&path)?;
    write_types(&mut file, types, semantic_state)?;

    if FORMAT_OUTPUT {
        Command::new("rustfmt").args([&path]).output()?;
    }

    Ok(())
}