use crate::{project::ConsumerProject, BuildOptions};
use proc_macro2::{TokenStream, TokenTree};
use quote::{quote, ToTokens};
use std::{collections::BTreeSet, path::Path, process::Command};
use syn::{punctuated::Punctuated, Item, Meta, Token};

fn identifiers(tokens: TokenStream, names: &mut BTreeSet<String>) {
    for token in tokens {
        match token {
            TokenTree::Ident(ident) => {
                names.insert(ident.to_string());
            }
            TokenTree::Group(group) => identifiers(group.stream(), names),
            _ => {}
        }
    }
}

fn item_name(item: &Item) -> Option<String> {
    match item {
        Item::Const(i) => Some(i.ident.to_string()),
        Item::Static(i) => Some(i.ident.to_string()),
        Item::Fn(i) => Some(i.sig.ident.to_string()),
        Item::Type(i) => Some(i.ident.to_string()),
        Item::Struct(i) => Some(i.ident.to_string()),
        Item::Enum(i) => Some(i.ident.to_string()),
        Item::Union(i) => Some(i.ident.to_string()),
        Item::Mod(i) => Some(i.ident.to_string()),
        Item::Macro(i) => i.ident.as_ref().map(ToString::to_string),
        _ => None,
    }
}

fn imported(tree: &syn::UseTree, names: &BTreeSet<String>) -> bool {
    match tree {
        syn::UseTree::Path(p) => imported(&p.tree, names),
        syn::UseTree::Name(n) => names.contains(&n.ident.to_string()),
        syn::UseTree::Rename(n) => names.contains(&n.rename.to_string()),
        syn::UseTree::Group(g) => g.items.iter().any(|t| imported(t, names)),
        syn::UseTree::Glob(_) => true,
    }
}

// Keep the expression's supporting declarations, leaving unrelated solver
// modules unopened and uncompiled. Rust, rather than a partial expression
// evaluator, resolves constants, arithmetic, blocks, and helper functions.
fn support(
    items: &[Item],
    directory: &Path,
    needed: &mut BTreeSet<String>,
) -> Result<TokenStream, String> {
    let mut selected = BTreeSet::new();
    loop {
        let before = needed.len();
        for (index, item) in items.iter().enumerate() {
            let implementation = if let Item::Impl(i) = item {
                let mut names = BTreeSet::new();
                identifiers(i.self_ty.to_token_stream(), &mut names);
                names.iter().any(|name| needed.contains(name))
            } else {
                false
            };
            let include = item_name(item).is_some_and(|name| needed.contains(&name))
                || implementation
                || matches!(item, Item::Use(u) if imported(&u.tree, needed) && !matches!(&u.tree, syn::UseTree::Path(p) if p.ident == "aoc_harness"));
            if include {
                selected.insert(index);
                // Module bodies are resolved below; scanning them here would
                // pull in unrelated day modules via their identifiers.
                if !matches!(item, Item::Mod(_)) {
                    identifiers(item.to_token_stream(), needed);
                }
            }
        }
        if needed.len() == before {
            break;
        }
    }

    let mut output = TokenStream::new();
    for index in selected {
        let item = &items[index];
        if let Item::Mod(module) = item {
            let mut module = module.clone();
            let child_directory = directory.join(module.ident.to_string());
            let child_items = if let Some((_, items)) = &module.content {
                items.clone()
            } else {
                let explicit = module.attrs.iter().find_map(|a| {
                    if a.path().is_ident("path") {
                        if let Meta::NameValue(v) = &a.meta {
                            if let syn::Expr::Lit(lit) = &v.value {
                                if let syn::Lit::Str(path) = &lit.lit {
                                    return Some(directory.join(path.value()));
                                }
                            }
                        }
                    }
                    None
                });
                let flat = directory.join(format!("{}.rs", module.ident));
                let path = explicit.unwrap_or_else(|| {
                    if flat.is_file() {
                        flat
                    } else {
                        child_directory.join("mod.rs")
                    }
                });
                let text = std::fs::read_to_string(&path)
                    .map_err(|e| format!("read {}: {e}", path.display()))?;
                syn::parse_file(&text)
                    .map_err(|e| format!("parse {}: {e}", path.display()))?
                    .items
            };
            let body = support(&child_items, &child_directory, needed)?;
            module.attrs.retain(|a| !a.path().is_ident("path"));
            let attrs = &module.attrs;
            let vis = &module.vis;
            let ident = &module.ident;
            output.extend(quote!(#(#attrs)* #vis mod #ident { #body }));
        } else {
            output.extend(item.to_token_stream());
        }
    }

    Ok(output)
}

pub(crate) fn resolve(project: &ConsumerProject, options: &BuildOptions) -> Result<u16, String> {
    resolve_inner(project, options).map_err(|error| {
        format!(
            "cannot determine the aoc_lib! event year for package {:?} in {}: {error}",
            project.package_name,
            project.library_source.display()
        )
    })
}

fn resolve_inner(project: &ConsumerProject, options: &BuildOptions) -> Result<u16, String> {
    let source = std::fs::read_to_string(&project.library_source).map_err(|e| e.to_string())?;
    let parsed = syn::parse_file(&source).map_err(|e| e.to_string())?;
    let entries: Vec<_> = parsed
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Macro(m)
                if m.mac
                    .path
                    .segments
                    .last()
                    .is_some_and(|s| s.ident == "aoc_lib") =>
            {
                Some(m)
            }
            _ => None,
        })
        .collect();
    if entries.len() != 1 {
        return Err("expected exactly one direct aoc_lib! declaration".into());
    }

    let args = entries[0]
        .mac
        .parse_body_with(Punctuated::<Meta, Token![,]>::parse_terminated)
        .map_err(|e| e.to_string())?;
    let years: Vec<_> = args
        .iter()
        .filter_map(|arg| match arg {
            Meta::NameValue(v) if v.path.is_ident("year") => Some(&v.value),
            _ => None,
        })
        .collect();
    if years.len() != 1 {
        return Err("expected year = EXPRESSION".into());
    }
    let year = years[0];
    if let syn::Expr::Lit(lit) = year {
        if let syn::Lit::Int(number) = &lit.lit {
            return number.base10_parse().map_err(|e| e.to_string());
        }
    }

    let mut needed = BTreeSet::new();
    identifiers(year.to_token_stream(), &mut needed);
    let directory = project
        .library_source
        .parent()
        .ok_or("library has no source directory")?;
    let declarations = loop {
        let before = needed.len();
        let declarations = support(&parsed.items, directory, &mut needed)?;
        if needed.len() == before {
            break declarations;
        }
    };
    let probe = tempfile::tempdir().map_err(|e| e.to_string())?;
    let path = probe.path().join("event_year.rs");
    let executable = probe
        .path()
        .join(format!("event-year{}", std::env::consts::EXE_SUFFIX));
    let code = quote! {
        #![allow(dead_code, unused_imports)]
        #declarations
        fn main() { let year: u16 = #year; println!("{}", year); }
    };
    std::fs::write(&path, code.to_string()).map_err(|e| e.to_string())?;

    let mut compiler = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
    compiler
        .current_dir(&project.manifest_dir)
        .arg(&path)
        .arg("--crate-name=event_year")
        .arg(format!(
            "--edition={}",
            project.package["edition"].as_str().unwrap_or("2021")
        ))
        .arg("-o")
        .arg(&executable)
        .env("CARGO_MANIFEST_DIR", &project.manifest_dir)
        .env("CARGO_PKG_NAME", &project.package_name);

    let feature_map = project.package["features"]
        .as_object()
        .ok_or("metadata has no features")?;
    let mut features = BTreeSet::new();
    if !options.no_default_features {
        features.insert("default".into());
    }
    for list in &options.features {
        features.extend(list.split([',', ' ']).filter(|s| !s.is_empty()).map(|s| {
            s.strip_prefix(&format!("{}/", project.package_name))
                .unwrap_or(s)
                .to_owned()
        }));
    }
    if options.all_features {
        features.extend(feature_map.keys().cloned());
    }
    loop {
        let before = features.len();
        for feature in features.clone() {
            if let Some(values) = feature_map.get(&feature).and_then(|v| v.as_array()) {
                features.extend(
                    values
                        .iter()
                        .filter_map(|v| v.as_str())
                        .filter(|s| feature_map.contains_key(*s))
                        .map(str::to_owned),
                );
            }
        }
        if before == features.len() {
            break;
        }
    }
    for feature in features {
        compiler.arg("--cfg").arg(format!("feature={feature:?}"));
    }

    let output = compiler
        .output()
        .map_err(|e| format!("evaluate year expression with rustc: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "year expression could not be resolved independently of solvers:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let output = Command::new(executable)
        .current_dir(&project.manifest_dir)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "year expression failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u16>()
        .map_err(|e| format!("year expression did not return a u16: {e}"))
}
