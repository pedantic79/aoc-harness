//! Typed adapters. Registration discovery reads Rust modules, never proc-macro global state.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use quote::{format_ident, quote};
use std::path::{Path, PathBuf};
use syn::{
    parse_macro_input, punctuated::Punctuated, FnArg, Item, ItemFn, Meta, ReturnType, Token, Type,
};

#[derive(Clone)]
struct Registration {
    day: u8,
    part: Option<u8>,
    name: Option<String>,
    default: bool,
    fresh: bool,
}

impl Registration {
    fn parse(tokens: Tokens, generator: bool) -> syn::Result<Self> {
        use syn::parse::Parser;
        let args = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(tokens)?;
        let mut reg = Self {
            day: 0,
            part: None,
            name: None,
            default: false,
            fresh: false,
        };

        for (index, arg) in args.iter().enumerate() {
            match arg {
                Meta::Path(path) => {
                    let ident = path
                        .get_ident()
                        .ok_or_else(|| syn::Error::new_spanned(path, "expected an identifier"))?;
                    let value = ident.to_string();
                    if index == 0 {
                        reg.day = value
                            .strip_prefix("day")
                            .and_then(|v| v.parse().ok())
                            .filter(|v| (1..=25).contains(v))
                            .ok_or_else(|| {
                                syn::Error::new_spanned(path, "expected day1 through day25")
                            })?;
                    } else if index == 1 && value.starts_with("part") {
                        reg.part = Some(
                            value
                                .strip_prefix("part")
                                .and_then(|v| v.parse().ok())
                                .filter(|v| (1..=2).contains(v))
                                .ok_or_else(|| {
                                    syn::Error::new_spanned(path, "expected part1 or part2")
                                })?,
                        );
                    } else if value == "default" {
                        reg.default = true;
                    } else if reg.part.is_some() && reg.name.is_none() {
                        reg.name = Some(value);
                    } else {
                        return Err(syn::Error::new_spanned(
                            path,
                            "unexpected registration argument",
                        ));
                    }
                }
                Meta::NameValue(value) if value.path.is_ident("bench_input") && !generator => {
                    if value.value != syn::parse_quote!("fresh") {
                        return Err(syn::Error::new_spanned(
                            value,
                            "bench_input must be \"fresh\"",
                        ));
                    }
                    reg.fresh = true;
                }
                _ => {
                    return Err(syn::Error::new_spanned(
                        arg,
                        "invalid registration argument",
                    ))
                }
            }
        }

        if reg.day == 0 || (!generator && reg.part.is_none()) || (generator && reg.default) {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "expected day, part (for solvers), optional name, optional default",
            ));
        }
        Ok(reg)
    }

    fn alias(&self, function: &syn::Ident) -> syn::Ident {
        format_ident!(
            "__aoc_input_{}_{}_{}_{}_{}",
            self.day,
            self.part.unwrap_or(0),
            if self.name.is_some() { "named" } else { "base" },
            self.name.as_deref().unwrap_or("unnamed"),
            function
        )
    }

    fn label(&self) -> String {
        format!(
            "day{}/part{}/{}",
            self.day,
            self.part.unwrap_or(0),
            self.name.as_deref().unwrap_or("default")
        )
    }

    fn adapter(&self, generator: bool, function: &syn::Ident) -> syn::Ident {
        let kind = if generator { "generator" } else { "solution" };
        format_ident!(
            "__aoc_{}_{}_{}_{}_{}_{}",
            kind,
            self.day,
            self.part.unwrap_or(0),
            if self.name.is_some() { "named" } else { "base" },
            self.name.as_deref().unwrap_or("unnamed"),
            function
        )
    }
}

fn argument(function: &ItemFn) -> syn::Result<&Type> {
    if function.sig.asyncness.is_some()
        || function.sig.unsafety.is_some()
        || function.sig.inputs.len() != 1
        || function.sig.generics.type_params().next().is_some()
        || function.sig.generics.const_params().next().is_some()
    {
        return Err(syn::Error::new_spanned(&function.sig, "registered functions must be safe, synchronous functions with one argument and no type/const parameters"));
    }

    match function.sig.inputs.first().unwrap() {
        FnArg::Typed(arg) => Ok(&arg.ty),
        arg => Err(syn::Error::new_spanned(arg, "expected a function argument")),
    }
}

fn output(function: &ItemFn) -> syn::Result<(Type, bool)> {
    let ReturnType::Type(_, ty) = &function.sig.output else {
        return Err(syn::Error::new_spanned(
            &function.sig,
            "registered functions must return a value",
        ));
    };
    if let Type::Path(path) = &**ty {
        if let Some(segment) = path.path.segments.last() {
            if segment.ident == "Result" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(value)) = args.args.first() {
                        return Ok((value.clone(), true));
                    }
                }
            }
        }
    }
    Ok(((**ty).clone(), false))
}

fn checked(call: Tokens, fallible: bool, reg: &Registration, phase: &str) -> Tokens {
    if !fallible {
        return call;
    }
    let day = reg.day;
    let part = reg.part;
    let part = match part {
        Some(p) => quote!(Some(#p)),
        None => quote!(None),
    };
    let name = reg.name.as_deref().unwrap_or("default");
    quote!(#call.map_err(|error| crate::__aoc_runtime::Error::phase(#day, #part, #name, #phase, error))?)
}

#[proc_macro_attribute]
pub fn aoc_generator(args: TokenStream, item: TokenStream) -> TokenStream {
    let function = parse_macro_input!(item as ItemFn);
    let result = (|| {
        let reg = Registration::parse(args.into(), true)?;
        let conditions: Vec<_> = function
            .attrs
            .iter()
            .filter(|a| a.path().is_ident("cfg") || a.path().is_ident("cfg_attr"))
            .collect();
        let arg = argument(&function)?;
        let (out, fallible) = output(&function)?;
        let name = &function.sig.ident;
        let adapter = reg.adapter(true, name);
        let parse = format_ident!("parse");
        let mut signature = function.sig.clone();
        signature.ident = parse.clone();

        let Type::Reference(reference) = arg else {
            return Err(syn::Error::new_spanned(
                arg,
                "generator input must be &str or &[u8]",
            ));
        };
        let valid_input = match &*reference.elem {
            Type::Path(p) => p.path.is_ident("str"),
            Type::Slice(s) => matches!(&*s.elem, Type::Path(p) if p.path.is_ident("u8")),
            _ => false,
        };
        if reference.mutability.is_some() || !valid_input {
            return Err(syn::Error::new_spanned(
                arg,
                "generator input must be &str or &[u8]",
            ));
        }

        let lifetime = &reference.lifetime;
        signature.inputs = syn::parse_quote!(raw: &#lifetime str);
        signature.output =
            syn::parse_quote!(-> ::std::result::Result<#out, crate::__aoc_runtime::Error>);
        let input = if matches!(&*reference.elem, Type::Slice(_)) {
            quote!(raw.as_bytes())
        } else {
            quote!(raw)
        };
        let call = checked(quote!(#name(#input)), fallible, &reg, "generator");
        let label = format!("generator/{}", reg.label());

        Ok(quote! {
            #function

            #[doc(hidden)]
            #(#conditions)*
            #[allow(private_interfaces)]
            impl crate::#adapter {
                pub(crate) #signature { ::std::result::Result::Ok(#call) }

                #[doc(hidden)]
                pub(crate) fn check(raw: &str) -> ::std::result::Result<(), crate::__aoc_runtime::Error> {
                    ::std::hint::black_box(Self::parse(raw)?);
                    ::std::result::Result::Ok(())
                }

                #[doc(hidden)]
                pub(crate) fn bench(c: &mut crate::__aoc_runtime::criterion::Criterion, raw: &str) {
                    c.bench_function(#label, |b| b.iter_batched(|| (), |_| {
                        ::std::hint::black_box(Self::parse(::std::hint::black_box(raw)).unwrap_or_else(|e| panic!("{e}")))
                    }, crate::__aoc_runtime::criterion::BatchSize::PerIteration));
                }
            }
        })
    })();
    result.unwrap_or_else(syn::Error::into_compile_error).into()
}

#[proc_macro_attribute]
pub fn aoc(args: TokenStream, item: TokenStream) -> TokenStream {
    let function = parse_macro_input!(item as ItemFn);
    let result = (|| {
        let reg = Registration::parse(args.into(), false)?;
        let conditions: Vec<_> = function
            .attrs
            .iter()
            .filter(|a| a.path().is_ident("cfg") || a.path().is_ident("cfg_attr"))
            .collect();
        let arg = argument(&function)?;
        let (_, fallible) = output(&function)?;
        let name = &function.sig.ident;
        let adapter = reg.adapter(false, name);
        let alias = reg.alias(name);
        let has_generator = format_ident!("{}_generated", alias);
        let day = reg.day;
        let part = reg.part.unwrap();
        let variant = reg.name.as_deref().unwrap_or("default");
        let invocation = match arg {
            Type::Reference(r) if r.mutability.is_some() => quote!(&mut input),
            Type::Reference(_) => quote!(&input),
            _ => quote!(input),
        };
        let call = quote!(#name(::std::hint::black_box(#invocation)));
        let solve = checked(call.clone(), fallible, &reg, "solver");
        let timed_solve = if fallible {
            quote!(#call.map_err(|e| crate::__aoc_runtime::Error::phase(#day, Some(#part), #variant, "solver", e).timings(generator, start.elapsed()))?)
        } else {
            call
        };
        let label = reg.label();
        let solver_label = format!("solver/{label}");
        let end_label = format!("end-to-end/{label}");
        let prepare = quote!(crate::#alias::parse(::std::hint::black_box(raw)));
        let prepare_bench = quote!(#prepare.unwrap_or_else(|e| panic!("{e}")));
        let solve_bench = quote!((|| -> ::std::result::Result<_, crate::__aoc_runtime::Error> { ::std::result::Result::Ok(#solve) })().unwrap_or_else(|e| panic!("{e}")));

        let solver_loop = match arg {
            Type::Reference(r) if r.mutability.is_some() || reg.fresh => {
                let invoke = if r.mutability.is_some() {
                    quote!(&mut *input)
                } else {
                    quote!(&*input)
                };
                let solve = checked(
                    quote!(#name(::std::hint::black_box(#invoke))),
                    fallible,
                    &reg,
                    "solver",
                );
                quote! {
                    b.iter_batched_ref(|| #prepare_bench, |input| {
                        ::std::hint::black_box((|| -> ::std::result::Result<_, crate::__aoc_runtime::Error> { ::std::result::Result::Ok(#solve) })().unwrap_or_else(|e| panic!("{e}")))
                    }, crate::__aoc_runtime::criterion::BatchSize::PerIteration);
                }
            }
            Type::Reference(_) => quote! {
                let input = #prepare_bench;
                b.iter_batched(|| (), |_| ::std::hint::black_box(#solve_bench), crate::__aoc_runtime::criterion::BatchSize::PerIteration);
            },
            _ => quote! {
                b.iter_batched(|| #prepare_bench, |input| ::std::hint::black_box(#solve_bench), crate::__aoc_runtime::criterion::BatchSize::PerIteration);
            },
        };

        Ok(quote! {
            #function

            #[doc(hidden)]
            #(#conditions)*
            impl crate::#adapter {
                #[allow(unused_mut)]
                pub(crate) fn run(raw: &str) -> ::std::result::Result<crate::__aoc_runtime::Invocation, crate::__aoc_runtime::Error> {
                    let start = ::std::time::Instant::now();
                    let mut input = #prepare.map_err(|e| e.timings(start.elapsed(), ::std::time::Duration::ZERO))?;
                    let generator = if crate::#has_generator { start.elapsed() } else { ::std::time::Duration::ZERO };

                    let start = ::std::time::Instant::now();
                    let answer = #timed_solve;
                    let solver = start.elapsed();

                    ::std::result::Result::Ok(crate::__aoc_runtime::Invocation { answer: ::std::boxed::Box::new(answer), generator, solver })
                }

                #[doc(hidden)]
                #[allow(unused_mut)]
                pub(crate) fn execute(raw: &str) -> ::std::result::Result<(), crate::__aoc_runtime::Error> {
                    let mut input = #prepare?;
                    ::std::hint::black_box(#solve);
                    ::std::result::Result::Ok(())
                }

                #[doc(hidden)]
                pub(crate) fn bench(c: &mut crate::__aoc_runtime::criterion::Criterion, raw: &str) {
                    c.bench_function(#solver_label, |b| { #solver_loop });
                    c.bench_function(#end_label, |b| b.iter(|| Self::execute(::std::hint::black_box(raw)).unwrap_or_else(|e| panic!("{e}"))));
                }
            }
        })
    })();
    result.unwrap_or_else(syn::Error::into_compile_error).into()
}

struct Found {
    reg: Registration,
    function: ItemFn,
    cfg: Vec<Tokens>,
}

fn cfgs(attrs: &[syn::Attribute]) -> Vec<Tokens> {
    attrs
        .iter()
        .filter_map(|a| {
            if a.path().is_ident("cfg") {
                if let Meta::List(list) = &a.meta {
                    return Some(list.tokens.clone());
                }
            }
            None
        })
        .collect()
}

fn scan(
    path: &Path,
    items: Vec<Item>,
    inherited: &[Tokens],
    found: &mut Vec<Found>,
    sources: &mut Vec<PathBuf>,
) -> syn::Result<()> {
    for item in items {
        match item {
            Item::Fn(function) => {
                for attr in &function.attrs {
                    let kind = attr
                        .path()
                        .segments
                        .last()
                        .map(|s| s.ident.to_string())
                        .unwrap_or_default();
                    if kind == "aoc" || kind == "aoc_generator" {
                        let Meta::List(list) = &attr.meta else {
                            continue;
                        };
                        let reg =
                            Registration::parse(list.tokens.clone(), kind == "aoc_generator")?;
                        let mut cfg = inherited.to_vec();
                        cfg.extend(cfgs(&function.attrs));
                        found.push(Found {
                            reg,
                            function: function.clone(),
                            cfg,
                        });
                    }
                }
            }
            Item::Mod(m) => {
                let mut cfg = inherited.to_vec();
                cfg.extend(cfgs(&m.attrs));

                if let Some((_, items)) = m.content {
                    scan(&path.join(m.ident.to_string()), items, &cfg, found, sources)?;
                } else {
                    let override_path = m
                        .attrs
                        .iter()
                        .find(|a| a.path().is_ident("path"))
                        .and_then(|a| {
                            if let Meta::NameValue(v) = &a.meta {
                                if let syn::Expr::Lit(l) = &v.value {
                                    if let syn::Lit::Str(s) = &l.lit {
                                        return Some(s.value());
                                    }
                                }
                            }
                            None
                        });
                    let file = override_path.map(|p| path.join(p)).unwrap_or_else(|| {
                        let flat = path.join(format!("{}.rs", m.ident));
                        if flat.exists() {
                            flat
                        } else {
                            path.join(m.ident.to_string()).join("mod.rs")
                        }
                    });
                    // Absent cfg-disabled modules are legal Rust.
                    if !file.exists() && !cfg.is_empty() {
                        continue;
                    }
                    let text = std::fs::read_to_string(&file).map_err(|e| {
                        syn::Error::new_spanned(
                            &m.ident,
                            format!("cannot read {}: {e}", file.display()),
                        )
                    })?;
                    let parsed = syn::parse_file(&text)?;
                    cfg.extend(cfgs(&parsed.attrs));
                    sources.push(file.clone());
                    let directory = if file.file_name().unwrap() == "mod.rs" {
                        file.parent().unwrap().to_owned()
                    } else {
                        file.with_extension("")
                    };
                    scan(&directory, parsed.items, &cfg, found, sources)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

#[proc_macro]
pub fn aoc_lib(input: TokenStream) -> TokenStream {
    let result = (|| -> syn::Result<Tokens> {
        let declaration: syn::ExprAssign = syn::parse(input)?;
        if declaration.left != Box::new(syn::parse_quote!(year)) {
            return Err(syn::Error::new_spanned(declaration, "expected year = 2025"));
        }
        let year = declaration.right;
        let dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let manifest: toml::Value = std::fs::read_to_string(dir.join("Cargo.toml"))
            .unwrap()
            .parse()
            .map_err(|e| {
                syn::Error::new(
                    proc_macro2::Span::call_site(),
                    format!("invalid manifest: {e}"),
                )
            })?;
        let lib = manifest
            .get("lib")
            .and_then(|l| l.get("path"))
            .and_then(|p| p.as_str())
            .unwrap_or("src/lib.rs");
        let workspace = dir
            .ancestors()
            .filter_map(|path| {
                std::fs::read_to_string(path.join("Cargo.toml"))
                    .ok()?
                    .parse::<toml::Value>()
                    .ok()
            })
            .find_map(|value| value.get("workspace").cloned());
        let runtime_name = manifest
            .get("dependencies")
            .and_then(|d| d.as_table())
            .and_then(|deps| {
                deps.iter()
                    .find(|(name, value)| {
                        let inherited =
                            value.get("workspace").and_then(|v| v.as_bool()) == Some(true);
                        let value = if inherited {
                            workspace
                                .as_ref()
                                .and_then(|w| w.get("dependencies"))
                                .and_then(|d| d.get(*name))
                                .unwrap_or(value)
                        } else {
                            value
                        };
                        *name == "aoc-harness"
                            || value.get("package").and_then(|p| p.as_str()) == Some("aoc-harness")
                    })
                    .map(|(n, _)| n.replace('-', "_"))
            })
            .unwrap_or_else(|| "aoc_harness".into());
        let runtime = format_ident!("{}", runtime_name);
        let root = dir.join(lib);
        let text = std::fs::read_to_string(&root).map_err(|e| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                format!("cannot read library: {e}"),
            )
        })?;
        let mut found = Vec::new();
        let mut sources = vec![root.clone()];
        scan(
            root.parent().unwrap(),
            syn::parse_file(&text)?.items,
            &[],
            &mut found,
            &mut sources,
        )?;

        let mut aliases = Vec::new();
        let mut solutions = Vec::new();
        let mut generators = Vec::new();
        let mut adapters = Vec::new();
        let mut diagnostics = Vec::new();
        for (index, a) in found.iter().enumerate() {
            for b in found.iter().skip(index + 1) {
                let is_generator = |entry: &Found| {
                    entry.function.attrs.iter().any(|attr| {
                        attr.path()
                            .segments
                            .last()
                            .is_some_and(|s| s.ident == "aoc_generator")
                    })
                };
                if is_generator(a) == is_generator(b)
                    && a.reg.day == b.reg.day
                    && a.reg.part == b.reg.part
                    && a.reg.name == b.reg.name
                {
                    let conditions = a.cfg.iter().chain(&b.cfg);
                    let kind = if is_generator(a) {
                        "generator"
                    } else {
                        "solution"
                    };
                    let message = format!("duplicate {kind} identity: {}", a.reg.label());
                    diagnostics.push(quote! {
                        #[cfg(all(#(#conditions),*))]
                        compile_error!(#message);
                    });
                }
            }
        }
        for entry in &found {
            let reg = &entry.reg;
            let day = reg.day;
            let part = reg.part.unwrap_or(0);
            let name = reg.name.as_deref().unwrap_or("default");
            let named = reg.name.is_some();
            let explicit = reg.default;
            let cfg = &entry.cfg;
            let function = &entry.function.sig.ident;
            let is_generator = entry.function.attrs.iter().any(|a| {
                a.path()
                    .segments
                    .last()
                    .is_some_and(|s| s.ident == "aoc_generator")
            });
            let adapter = reg.adapter(is_generator, function);
            adapters.push(quote! {
                #[doc(hidden)]
                #[cfg(all(#(#cfg),*))]
                #[allow(non_camel_case_types)]
                pub(crate) struct #adapter;
            });
            if is_generator {
                generators.push(quote! {
                    #[cfg(all(#(#cfg),*))]
                    crate::__aoc_runtime::Generator { day: #day, part: #part, name: #name, bench: crate::#adapter::bench, check: crate::#adapter::check }
                });
                continue;
            }

            solutions.push(quote! {
                #[cfg(all(#(#cfg),*))]
                crate::__aoc_runtime::Solution {
                    day: #day, part: #part, name: #name, named: #named, explicit_default: #explicit,
                    run: crate::#adapter::run, execute: crate::#adapter::execute, bench: crate::#adapter::bench,
                }
            });

            let alias = reg.alias(function);
            let has_generator = format_ident!("{}_generated", alias);
            let mut candidates: Vec<_> = found
                .iter()
                .filter(|g| {
                    g.function.attrs.iter().any(|a| {
                        a.path()
                            .segments
                            .last()
                            .is_some_and(|s| s.ident == "aoc_generator")
                    }) && g.reg.day == day
                        && (g.reg.part.is_none() || g.reg.part == reg.part)
                        && (g.reg.name.is_none() || g.reg.name == reg.name)
                })
                .collect();
            candidates
                .sort_by_key(|g| std::cmp::Reverse((g.reg.part.is_some(), g.reg.name.is_some())));
            let mut higher = Vec::new();
            for candidate in candidates {
                let conditions = &candidate.cfg;
                let condition = quote!(all(#(#conditions),*));
                let generator = candidate.reg.adapter(true, &candidate.function.sig.ident);
                aliases.push(quote! {
                    #[cfg(all(#(#cfg,)* #condition, not(any(#(#higher),*))))]
                    pub(crate) use crate::#generator as #alias;
                    #[cfg(all(#(#cfg,)* #condition, not(any(#(#higher),*))))]
                    #[allow(non_upper_case_globals)]
                    pub(crate) const #has_generator: bool = true;
                });
                higher.push(condition);
            }

            let arg = argument(&entry.function)?;
            let raw = match arg {
                Type::Reference(r) if r.mutability.is_none() => match &*r.elem {
                    Type::Path(p) if p.path.is_ident("str") => Some(quote!(&str)),
                    Type::Slice(s) if matches!(&*s.elem, Type::Path(p) if p.path.is_ident("u8")) => {
                        Some(quote!(&[u8]))
                    }
                    _ => None,
                },
                _ => None,
            };
            let fallback = if let Some(out) = raw {
                let input = if out.to_string().contains("u8") {
                    quote!(raw.as_bytes())
                } else {
                    quote!(raw)
                };
                quote! {
                    #[allow(non_camel_case_types)]
                    pub(crate) struct #alias;
                    #[cfg(all(#(#cfg,)* not(any(#(#higher),*))))]
                    impl #alias {
                        pub(crate) fn parse(raw: &str) -> ::std::result::Result<#out, crate::__aoc_runtime::Error> { ::std::result::Result::Ok(#input) }
                    }
                }
            } else {
                let error = format!("no generator registered for {}", reg.label());
                quote!(compile_error!(#error);)
            };
            aliases.push(quote! {
                #[cfg(all(#(#cfg,)* not(any(#(#higher),*))))]
                #fallback
                #[cfg(all(#(#cfg,)* not(any(#(#higher),*))))]
                #[allow(non_upper_case_globals)]
                pub(crate) const #has_generator: bool = false;
            });
        }
        let sources = sources.iter().map(|p| p.to_string_lossy().into_owned());

        Ok(quote! {
            #[doc(hidden)]
            pub use ::#runtime as __aoc_runtime;

            #(#diagnostics)*
            #(#adapters)*
            #(#aliases)*
            const _: () = { #(let _ = include_str!(#sources);)* };
            const _: () = crate::__aoc_runtime::validate(&[#(#solutions),*], &[#(#generators),*]);

            #[doc(hidden)]
            pub fn __aoc_registry() -> crate::__aoc_runtime::Registry {
                crate::__aoc_runtime::Registry {
                    year: #year, manifest_dir: env!("CARGO_MANIFEST_DIR"),
                    solutions: &[#(#solutions),*], generators: &[#(#generators),*],
                }
            }

            #[doc(hidden)]
            pub fn __aoc_harness_main() -> i32 {
                crate::__aoc_runtime::entry(__aoc_registry())
            }
        })
    })();
    result.unwrap_or_else(syn::Error::into_compile_error).into()
}
