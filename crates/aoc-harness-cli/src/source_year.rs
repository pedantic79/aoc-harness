use crate::{project::ConsumerProject, BuildOptions};
use proc_macro2::{TokenStream, TokenTree};
use quote::{quote, ToTokens};
use std::{collections::BTreeSet, path::Path, process::Command};
use syn::{Item, Meta, Token};

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

/// Registered years and their module segments below the crate root.
type Years = Vec<(u16, Vec<String>)>;

/// The event years an `aoc_lib!` declaration registers, read from its source.
enum Declared {
    /// `year = EXPR`, which maps one year to the crate root.
    Single(Box<syn::Expr>),
    /// `years = { YEAR => module, ... }`.
    Multi { years: Years, default: Option<u16> },
}

/// The arguments of `aoc_lib!` that select years. The macro validates the
/// rest of the declaration when the crate compiles.
struct Arguments {
    year: Option<syn::Expr>,
    years: Option<Vec<(syn::Expr, syn::Path)>>,
    default_year: Option<syn::Expr>,
}

impl syn::parse::Parse for Arguments {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        let mut arguments = Self {
            year: None,
            years: None,
            default_year: None,
        };
        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let duplicate =
                || syn::Error::new_spanned(&key, format!("{key} is given more than once"));

            match key.to_string().as_str() {
                "years" => {
                    let content;
                    syn::braced!(content in input);
                    let mut pairs = Vec::new();
                    while !content.is_empty() {
                        let year: syn::Expr = content.parse()?;
                        content.parse::<Token![=>]>()?;
                        pairs.push((year, content.call(syn::Path::parse_mod_style)?));
                        if !content.is_empty() {
                            content.parse::<Token![,]>()?;
                        }
                    }
                    if arguments.years.replace(pairs).is_some() {
                        return Err(duplicate());
                    }
                }
                "year" => {
                    if arguments.year.replace(input.parse()?).is_some() {
                        return Err(duplicate());
                    }
                }
                "default_year" => {
                    if arguments.default_year.replace(input.parse()?).is_some() {
                        return Err(duplicate());
                    }
                }
                _ => {
                    input.parse::<syn::Expr>()?;
                }
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(arguments)
    }
}

fn literal_year(expr: &syn::Expr, role: &str) -> Result<u16, String> {
    if let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Int(int),
        ..
    }) = expr
    {
        if let Ok(year) = int.base10_parse::<u16>() {
            if year >= 2015 {
                return Ok(year);
            }
        }
    }
    Err(format!(
        "{role} must be an integer literal event year, 2015 or later"
    ))
}

fn declared(source: &str) -> Result<(syn::File, Declared), String> {
    let parsed = syn::parse_file(source).map_err(|e| e.to_string())?;
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

    let arguments: Arguments = entries[0].mac.parse_body().map_err(|e| e.to_string())?;
    let declared = match (arguments.year, arguments.years) {
        (Some(year), None) => Declared::Single(Box::new(year)),
        (None, Some(pairs)) => {
            let mut years = Vec::new();
            for (year, path) in &pairs {
                let year = literal_year(year, "each years key")?;
                let mut module: Vec<_> =
                    path.segments.iter().map(|s| s.ident.to_string()).collect();
                if module.first().is_some_and(|s| s == "crate") {
                    module.remove(0);
                }
                years.push((year, module));
            }
            let default = arguments
                .default_year
                .map(|year| literal_year(&year, "default_year"))
                .transpose()?;
            Declared::Multi { years, default }
        }
        (Some(_), Some(_)) => {
            return Err("use either year = ... or years = { ... }, not both".into())
        }
        (None, None) => {
            return Err("expected year = EXPRESSION or years = { YEAR => module, ... }".into())
        }
    };
    Ok((parsed, declared))
}

fn unregistered(year: u16, registered: impl IntoIterator<Item = u16>) -> String {
    let mut registered: Vec<_> = registered.into_iter().collect();
    registered.sort_unstable();
    let registered: Vec<_> = registered.iter().map(u16::to_string).collect();

    format!(
        "event year {year} is not registered; registered years: {}",
        registered.join(", ")
    )
}

/// Choose the requested year, the default year, or the only year.
fn choose(
    years: &[(u16, Vec<String>)],
    default: Option<u16>,
    requested: Option<u16>,
) -> Result<(u16, Vec<String>), String> {
    let registered = || years.iter().map(|(year, _)| *year);
    let only = (years.len() == 1).then(|| years[0].0);

    let Some(year) = requested.or(default).or(only) else {
        let mut available: Vec<_> = registered().collect();
        available.sort_unstable();
        let available: Vec<_> = available.iter().map(u16::to_string).collect();
        return Err(format!(
            "several event years are registered ({}) and aoc_lib! sets no default_year; select one with --year",
            available.join(", ")
        ));
    };
    years
        .iter()
        .find(|(registered, _)| *registered == year)
        .cloned()
        .ok_or_else(|| unregistered(year, registered()))
}

/// The selected event and the module, below the crate root, that holds its
/// solutions. The year is `None` when it comes from an unevaluated
/// `year = EXPR`, so callers that never need it skip compiling a probe.
pub(crate) struct Selection {
    pub year: Option<u16>,
    pub module: Vec<String>,
}

/// Select an event from the library source without compiling solvers.
///
/// `year = EXPR` is evaluated only when `--year` must be checked against it.
pub(crate) fn select(
    project: &ConsumerProject,
    options: &BuildOptions,
    requested: Option<u16>,
) -> Result<Selection, String> {
    select_with(project, options, requested, false)
}

/// Resolve the selected event year, evaluating `year = EXPR` when needed.
pub(crate) fn resolve(
    project: &ConsumerProject,
    options: &BuildOptions,
    requested: Option<u16>,
) -> Result<u16, String> {
    let selection = select_with(project, options, requested, true)?;

    Ok(selection.year.expect("evaluated selections have a year"))
}

fn select_with(
    project: &ConsumerProject,
    options: &BuildOptions,
    requested: Option<u16>,
    evaluate_single: bool,
) -> Result<Selection, String> {
    let undetermined = |error: String| {
        format!(
            "cannot determine the aoc_lib! event year for package {:?} in {}: {error}",
            project.package_name,
            project.library_source.display()
        )
    };

    let source = std::fs::read_to_string(&project.library_source)
        .map_err(|e| undetermined(e.to_string()))?;
    let (parsed, declared) = declared(&source).map_err(undetermined)?;

    match declared {
        Declared::Single(_) if requested.is_none() && !evaluate_single => Ok(Selection {
            year: None,
            module: Vec::new(),
        }),
        Declared::Single(expression) => {
            let year = evaluate(project, options, &parsed, &expression).map_err(undetermined)?;
            if let Some(requested) = requested.filter(|requested| *requested != year) {
                return Err(unregistered(requested, [year]));
            }
            Ok(Selection {
                year: Some(year),
                module: Vec::new(),
            })
        }
        Declared::Multi { years, default } => {
            let (year, module) = choose(&years, default, requested)?;
            Ok(Selection {
                year: Some(year),
                module,
            })
        }
    }
}

fn evaluate(
    project: &ConsumerProject,
    options: &BuildOptions,
    parsed: &syn::File,
    year: &syn::Expr,
) -> Result<u16, String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn years(source: &str) -> Result<(Years, Option<u16>), String> {
        match declared(source)?.1 {
            Declared::Multi { years, default } => Ok((years, default)),
            Declared::Single(_) => Err("single".into()),
        }
    }

    #[test]
    fn years_mappings_are_read_without_compiling() {
        let source = "mod y2019;\nmod nested;\naoc_harness::aoc_lib! {\n    years = { 2025 => crate::y2025, 2019 => nested::y2019, },\n    default_year = 2025,\n}\n";

        let (years, default) = years(source).unwrap();
        let single =
            declared("const EVENT_YEAR: u16 = 2025; aoc_harness::aoc_lib! { year = EVENT_YEAR }")
                .unwrap();

        assert_eq!(
            years,
            [
                (2025, vec!["y2025".to_owned()]),
                (2019, vec!["nested".to_owned(), "y2019".to_owned()]),
            ]
        );
        assert_eq!(default, Some(2025));
        assert!(matches!(single.1, Declared::Single(_)));
    }

    #[test]
    fn invalid_year_declarations_are_reported() {
        for (source, expected) in [
            ("aoc_lib! { years = { YEAR => y2025 } }", "each years key"),
            ("aoc_lib! { years = { 2014 => y2014 } }", "each years key"),
            (
                "aoc_lib! { years = { 2025 => y2025 }, default_year = DEFAULT }",
                "default_year",
            ),
            (
                "aoc_lib! { year = 2025, years = { 2025 => y2025 } }",
                "not both",
            ),
            ("aoc_lib! { year = 2025, year = 2024 }", "more than once"),
            ("aoc_lib! { default_year = 2025 }", "expected year"),
            ("pub fn unrelated() {}", "exactly one direct aoc_lib!"),
        ] {
            let error = declared(source).err().unwrap();

            assert!(error.contains(expected), "{source}: {error}");
        }
    }

    #[test]
    fn selection_prefers_requested_then_default_then_only_year() {
        let mapped = |year: u16| (year, vec![format!("y{year}")]);
        let both = [mapped(2019), mapped(2025)];
        let only = [mapped(2019)];

        let requested = choose(&both, Some(2025), Some(2019)).unwrap();
        let default = choose(&both, Some(2025), None).unwrap();
        let sole = choose(&only, None, None).unwrap();
        let ambiguous = choose(&both, None, None).unwrap_err();
        let unknown = choose(&both, Some(2025), Some(2018)).unwrap_err();
        let unknown_single = choose(&only, None, Some(2025)).unwrap_err();

        assert_eq!(requested, mapped(2019));
        assert_eq!(default, mapped(2025));
        assert_eq!(sole, mapped(2019));
        assert!(ambiguous.contains("several event years are registered (2019, 2025)"));
        assert!(ambiguous.contains("--year"));
        assert_eq!(
            unknown,
            "event year 2018 is not registered; registered years: 2019, 2025"
        );
        assert_eq!(
            unknown_single,
            "event year 2025 is not registered; registered years: 2019"
        );
    }
}
