use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use syn::{spanned::Spanned, Item};

struct NewFile {
    path: PathBuf,
    contents: String,
}

pub(crate) struct ScaffoldPlan {
    package: PathBuf,
    /// The source file that declares the day modules: the library root, or
    /// the selected year's module file.
    root: PathBuf,
    original: String,
    replacement: String,
    permissions: fs::Permissions,
    files: Vec<NewFile>,
}

fn safe_source(package: &Path, path: &Path) -> Result<(), String> {
    let relative = path
        .strip_prefix(package)
        .map_err(|_| format!("source {} is outside the selected package", path.display()))?;
    let mut current = package.to_owned();
    for part in relative.components() {
        if !matches!(part, std::path::Component::Normal(_)) {
            return Err("source path must stay within the selected package".into());
        }
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(format!("refusing source symlink {}", current.display()))
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("inspect source {}: {e}", current.display())),
        }
    }
    Ok(())
}

/// Reports whether `ident` names a day 1–9 module, as `day04` when `padded`
/// is set or as `day4` otherwise.
fn is_single_digit_day(ident: &str, padded: bool) -> bool {
    let Some(digits) = ident.strip_prefix("day") else {
        return false;
    };
    let digits = if padded {
        match digits.strip_prefix('0') {
            Some(rest) => rest,
            None => return false,
        }
    } else {
        digits
    };

    matches!(digits.as_bytes(), [b'1'..=b'9'])
}

/// Find the file and child-module directory of the year module at `module`,
/// below the library root. Each segment must be one plain `mod name;`
/// declaration backed by exactly one of `name.rs` or `name/mod.rs`.
fn module_source(
    package: &Path,
    library: &Path,
    module: &[String],
) -> Result<(PathBuf, PathBuf), String> {
    let mut file = library.to_owned();
    let mut directory = library.parent().unwrap().to_owned();

    for (depth, segment) in module.iter().enumerate() {
        let path = module[..=depth].join("::");
        let ambiguous = |reason: &str| {
            format!("year module `{path}` {reason}; generate days there manually or declare it as a plain `mod {segment};` backed by {segment}.rs or {segment}/mod.rs")
        };
        safe_source(package, &file)?;
        let text = fs::read_to_string(&file)
            .map_err(|e| format!("read source {}: {e}", file.display()))?;
        let parsed =
            syn::parse_file(&text).map_err(|e| format!("parse source {}: {e}", file.display()))?;

        let declarations: Vec<_> = parsed
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Mod(m) if m.ident == segment => Some(m),
                _ => None,
            })
            .collect();
        let [declaration] = declarations[..] else {
            return Err(ambiguous(&format!(
                "must be declared exactly once in {}",
                file.display()
            )));
        };
        if declaration.content.is_some() {
            return Err(ambiguous("is an inline module"));
        }
        if declaration.attrs.iter().any(|a| {
            a.path().is_ident("cfg") || a.path().is_ident("cfg_attr") || a.path().is_ident("path")
        }) {
            return Err(ambiguous("has cfg or path attributes"));
        }

        let candidates = [
            directory.join(format!("{segment}.rs")),
            directory.join(segment).join("mod.rs"),
        ];
        let mut existing = Vec::new();
        for candidate in candidates {
            safe_source(package, &candidate)?;
            match fs::symlink_metadata(&candidate) {
                Ok(meta) if meta.is_file() => existing.push(candidate),
                Ok(_) => return Err(format!("conflicting source path {}", candidate.display())),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(format!("inspect {}: {e}", candidate.display())),
            }
        }
        let [found] = &existing[..] else {
            return Err(ambiguous(if existing.is_empty() {
                "has no source file"
            } else {
                "has both a flat and a mod.rs source file"
            }));
        };

        file = found.clone();
        directory = directory.join(segment);
    }

    Ok((file, directory))
}

/// The byte offset of the start of the line after `line` (1-based), and
/// whether the text before it lacks a trailing newline.
fn after_line(text: &str, line: usize) -> (usize, bool) {
    let offset: usize = text.split_inclusive('\n').take(line).map(str::len).sum();

    (offset, offset > 0 && !text[..offset].ends_with('\n'))
}

impl ScaffoldPlan {
    /// Plan new day modules under `module`, a path of module segments below
    /// the library root; an empty path targets the root itself.
    pub fn prepare(
        package: &Path,
        library: &Path,
        module: &[String],
        days: &[u8],
        template_path: Option<&Path>,
        resolve_year: impl FnOnce() -> Result<u16, String>,
    ) -> Result<Self, String> {
        safe_source(package, library)?;
        let library_text = fs::read_to_string(library)
            .map_err(|e| format!("read source {}: {e}", library.display()))?;
        let library_parsed = syn::parse_file(&library_text)
            .map_err(|e| format!("parse source {}: {e}", library.display()))?;
        let entries: Vec<_> = library_parsed.items.iter().filter(|item| matches!(item, Item::Macro(m) if m.mac.path.segments.last().is_some_and(|s| s.ident == "aoc_lib"))).collect();
        if entries.len() != 1 {
            return Err(
                "scaffolding requires exactly one direct aoc_lib! invocation in the library root"
                    .into(),
            );
        }

        let (root, directory) = module_source(package, library, module)?;
        let root = root.as_path();
        let directory = directory.as_path();
        let original =
            fs::read_to_string(root).map_err(|e| format!("read source {}: {e}", root.display()))?;
        let parsed = syn::parse_file(&original)
            .map_err(|e| format!("parse source {}: {e}", root.display()))?;
        let permissions = fs::metadata(root)
            .map_err(|e| format!("source permissions: {e}"))?
            .permissions();
        let mut files = Vec::new();
        let mut declarations = String::new();
        let newline = if original.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let mut days = days.to_vec();
        days.sort_unstable();
        days.dedup();
        if days.is_empty() {
            return Err("generation requires an explicit --day selection".into());
        }

        let template = match template_path {
            Some(path) => fs::read_to_string(path)
                .map_err(|e| format!("read day template {}: {e}", path.display()))?,
            None => include_str!("../templates/day.rs").to_owned(),
        };
        let template = template.replace("\r\n", "\n").replace('\n', newline);
        let template = if template.contains("@YEAR@") {
            template.replace("@YEAR@", &resolve_year()?.to_string())
        } else {
            template
        };

        // Projects that already declare only unpadded single-digit days keep that style.
        let declared: Vec<_> = parsed
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Mod(m) => Some(m.ident.to_string()),
                _ => None,
            })
            .collect();
        let pad = !declared.iter().any(|n| is_single_digit_day(n, false))
            || declared.iter().any(|n| is_single_digit_day(n, true));

        for day in days {
            let padded = format!("day{day:02}");
            let unpadded = format!("day{day}");
            let (name, other) = if pad {
                (padded, unpadded)
            } else {
                (unpadded, padded)
            };
            let mut names = vec![name.clone(), other];
            names.dedup();
            let modules: Vec<_> = parsed
                .items
                .iter()
                .filter_map(|i| match i {
                    Item::Mod(m) if names.iter().any(|n| m.ident == n) => Some(m),
                    _ => None,
                })
                .collect();
            let paths: Vec<_> = names
                .iter()
                .flat_map(|n| {
                    [
                        directory.join(format!("{n}.rs")),
                        directory.join(n).join("mod.rs"),
                    ]
                })
                .collect();
            for path in &paths {
                safe_source(package, path)?;
            }
            let mut existing = Vec::new();
            for path in &paths {
                match fs::symlink_metadata(path) {
                    Ok(meta) if meta.is_file() => existing.push(path),
                    Ok(_) => return Err(format!("conflicting source path {}", path.display())),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(format!("inspect {}: {e}", path.display())),
                }
            }
            let directory_modules = names.iter().any(|n| directory.join(n).exists());
            if modules.len() == 1 && existing.len() == 1 {
                let module = modules[0];
                let expected = directory.join(format!("{}.rs", module.ident));
                let directory_source = directory.join(module.ident.to_string()).join("mod.rs");
                if module.content.is_none()
                    && !module.attrs.iter().any(|a| {
                        a.path().is_ident("cfg")
                            || a.path().is_ident("cfg_attr")
                            || a.path().is_ident("path")
                    })
                    && (expected == *existing[0] || directory_source == *existing[0])
                {
                    continue;
                }
            }
            if !modules.is_empty() || !existing.is_empty() || directory_modules {
                return Err(format!("day {day} has ambiguous/conflicting modules or files; resolve inline modules, cfg/path attributes, coexisting padded and unpadded declarations, and undeclared or mismatched files manually"));
            }
            files.push(NewFile {
                path: paths[0].clone(),
                contents: template.replace("@DAY@", &day.to_string()),
            });
            declarations.push_str(&format!("mod {name};{newline}"));
        }
        let mut replacement = original.clone();
        if !files.is_empty() && module.is_empty() {
            // The library root is the target, so its aoc_lib! anchors the declarations.
            let start = entries[0].span().start();
            let offset = original
                .split_inclusive('\n')
                .take(start.line - 1)
                .map(str::len)
                .sum::<usize>()
                + start.column;
            replacement.insert_str(offset, &format!("{declarations}{newline}"));
        } else if !files.is_empty() {
            // Year modules have no aoc_lib! anchor: follow the last file
            // module declaration, else the inner attributes, else the top.
            let last_module = parsed
                .items
                .iter()
                .rev()
                .find(|i| matches!(i, Item::Mod(m) if m.content.is_none()));
            let last_inner = parsed
                .attrs
                .iter()
                .rfind(|a| matches!(a.style, syn::AttrStyle::Inner(_)));
            let (line, separate) = match (last_module, last_inner) {
                (Some(item), _) => (item.span().end().line, false),
                (None, Some(attribute)) => (attribute.span().end().line, true),
                (None, None) => (0, true),
            };
            let (offset, unterminated) = after_line(&original, line);
            let separator = if separate && offset < original.len() {
                newline
            } else {
                ""
            };
            let lead = if unterminated { newline } else { "" };
            replacement.insert_str(offset, &format!("{lead}{declarations}{separator}"));
        }
        Ok(Self {
            package: package.to_owned(),
            root: root.to_owned(),
            original,
            replacement,
            permissions,
            files,
        })
    }

    pub fn print(&self) {
        if self.files.is_empty() {
            println!("No source changes required");
            return;
        }
        for file in &self.files {
            println!(
                "--- /dev/null\n+++ {}\n@@ -0,0 +1,{} @@",
                file.path.display(),
                file.contents.lines().count()
            );
            for line in file.contents.lines() {
                println!("+{line}");
            }
        }
        println!(
            "--- {}\n+++ {}\n@@ -1,{} +1,{} @@",
            self.root.display(),
            self.root.display(),
            self.original.lines().count(),
            self.replacement.lines().count()
        );
        for line in self.original.lines() {
            println!("-{line}");
        }
        for line in self.replacement.lines() {
            println!("+{line}");
        }
    }

    pub fn apply(self) -> Result<(), String> {
        self.apply_with(
            |temporary, root| {
                temporary
                    .persist(root)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            },
            &mut std::io::stdout(),
        )
    }

    fn apply_with(
        self,
        replace: impl FnOnce(tempfile::NamedTempFile, &Path) -> Result<(), String>,
        progress: &mut dyn Write,
    ) -> Result<(), String> {
        safe_source(&self.package, &self.root)?;
        if fs::read_to_string(&self.root).map_err(|e| e.to_string())? != self.original {
            return Err("source changed since scaffolding preflight; no files written".into());
        }
        if self.files.is_empty() {
            writeln!(progress, "No source changes required").map_err(|e| e.to_string())?;
            return Ok(());
        }
        for file in &self.files {
            safe_source(&self.package, &file.path)?;
            if fs::symlink_metadata(&file.path).is_ok() {
                return Err(format!(
                    "source appeared since preflight: {}",
                    file.path.display()
                ));
            }
        }
        let mut created: Vec<&NewFile> = Vec::new();
        let result = (|| {
            for file in &self.files {
                // A flat year module, y2019.rs, keeps its days in y2019/.
                let directory = file.path.parent().unwrap();
                fs::create_dir_all(directory)
                    .map_err(|e| format!("create {}: {e}", directory.display()))?;
                let mut temporary = tempfile::NamedTempFile::new_in(file.path.parent().unwrap())
                    .map_err(|e| format!("create temporary source: {e}"))?;
                temporary
                    .write_all(file.contents.as_bytes())
                    .map_err(|e| format!("write {}: {e}", file.path.display()))?;
                temporary
                    .as_file()
                    .sync_all()
                    .map_err(|e| format!("sync {}: {e}", file.path.display()))?;
                temporary
                    .persist_noclobber(&file.path)
                    .map_err(|e| format!("publish source {}: {}", file.path.display(), e.error))?;
                created.push(file);
            }
            let mut temporary = tempfile::NamedTempFile::new_in(self.root.parent().unwrap())
                .map_err(|e| e.to_string())?;
            temporary
                .write_all(self.replacement.as_bytes())
                .map_err(|e| e.to_string())?;
            temporary
                .as_file()
                .set_permissions(self.permissions.clone())
                .map_err(|e| e.to_string())?;
            temporary.as_file().sync_all().map_err(|e| e.to_string())?;
            safe_source(&self.package, &self.root)?;
            if fs::read_to_string(&self.root).map_err(|e| e.to_string())? != self.original {
                return Err("source changed during scaffolding; root left untouched".into());
            }
            replace(temporary, &self.root).map_err(|e| format!("replace root source: {e}"))
        })();
        if let Err(error) = result {
            let mut rollback = Vec::new();
            for file in created {
                if safe_source(&self.package, &file.path).is_ok()
                    && fs::read(&file.path).ok().as_deref() == Some(file.contents.as_bytes())
                {
                    if let Err(e) = fs::remove_file(&file.path) {
                        rollback.push(format!("{}: {e}", file.path.display()));
                    }
                } else {
                    rollback.push(format!(
                        "{} changed; retained for manual recovery",
                        file.path.display()
                    ));
                }
            }
            return Err(if rollback.is_empty() {
                error
            } else {
                format!("{error}; rollback incomplete: {}", rollback.join("; "))
            });
        }
        for file in &self.files {
            writeln!(progress, "Generated: {}", file.path.display()).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_preflight_idempotence_line_endings_and_concurrent_edits() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("custom.rs");
        let original = "//! Inner docs\r\n#![allow(dead_code)]\r\nuse aoc_harness::aoc_lib;\r\naoc_lib! { year = 2025 }\r\n";
        fs::write(&root, original).unwrap();
        let plan =
            ScaffoldPlan::prepare(temp.path(), &root, &[], &[4, 5], None, || Ok(2025)).unwrap();

        assert_eq!(fs::read_to_string(&root).unwrap(), original);
        assert!(plan
            .replacement
            .contains("mod day04;\r\nmod day05;\r\n\r\naoc_lib!"));
        plan.apply().unwrap();
        let after = fs::read(&root).unwrap();
        ScaffoldPlan::prepare(temp.path(), &root, &[], &[4, 5], None, || Ok(2025))
            .unwrap()
            .apply()
            .unwrap();

        assert_eq!(fs::read(&root).unwrap(), after);
        fs::write(temp.path().join("day7.rs"), "conflict").unwrap();
        assert!(
            ScaffoldPlan::prepare(temp.path(), &root, &[], &[6, 7], None, || Ok(2025)).is_err()
        );
        assert!(!temp.path().join("day06.rs").exists());
        let plan = ScaffoldPlan::prepare(temp.path(), &root, &[], &[6], None, || Ok(2025)).unwrap();
        fs::write(&root, "// user edit").unwrap();

        assert!(plan.apply().is_err());
        assert!(!temp.path().join("day06.rs").exists());
    }

    #[test]
    fn root_failure_rolls_back_and_existing_modules_are_preserved() {
        for (name, generated) in [("day4", "day5.rs"), ("day04", "day05.rs")] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("lib.rs");
            fs::write(
                &root,
                format!("mod {name};\naoc_harness::aoc_lib! {{ year = 2025 }}\n"),
            )
            .unwrap();
            let existing = temp.path().join(format!("{name}.rs"));
            fs::write(&existing, "// existing").unwrap();

            let plan =
                ScaffoldPlan::prepare(temp.path(), &root, &[], &[4, 5], None, || Ok(2025)).unwrap();

            assert_eq!(plan.files.len(), 1);
            assert_eq!(plan.files[0].path, temp.path().join(generated));
            assert!(plan
                .apply_with(
                    |_, _| Err("injected write failure".into()),
                    &mut std::io::stdout()
                )
                .is_err());
            assert!(!temp.path().join(generated).exists());
            assert_eq!(fs::read_to_string(existing).unwrap(), "// existing");
        }
    }

    #[test]
    fn new_modules_follow_the_declared_naming_style() {
        for (declarations, expected) in [
            ("", "day05"),
            ("mod day1;\nmod day4;\nmod day10;\n", "day5"),
            ("mod day01;\nmod day04;\n", "day05"),
            ("mod day1;\nmod day04;\n", "day05"),
            ("mod day10;\nmod day11;\n", "day05"),
        ] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("lib.rs");
            fs::write(
                &root,
                format!("{declarations}aoc_harness::aoc_lib! {{ year = 2025 }}\n"),
            )
            .unwrap();
            for module in declarations.lines() {
                let module = module.trim_start_matches("mod ").trim_end_matches(';');
                fs::write(temp.path().join(format!("{module}.rs")), "").unwrap();
            }

            let plan =
                ScaffoldPlan::prepare(temp.path(), &root, &[], &[5], None, || Ok(2025)).unwrap();

            assert_eq!(
                plan.files[0].path,
                temp.path().join(format!("{expected}.rs")),
                "{declarations:?}"
            );
            assert!(
                plan.replacement.contains(&format!("mod {expected};\n")),
                "{declarations:?}"
            );
        }
    }

    #[test]
    fn two_digit_days_are_generated_once_and_then_skipped() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("lib.rs");
        fs::write(&root, "aoc_harness::aoc_lib! { year = 2025 }\n").unwrap();

        let plan =
            ScaffoldPlan::prepare(temp.path(), &root, &[], &[10], None, || Ok(2025)).unwrap();

        assert_eq!(plan.files.len(), 1);
        assert_eq!(plan.files[0].path, temp.path().join("day10.rs"));

        plan.apply().unwrap();
        let after = fs::read(&root).unwrap();
        let rerun =
            ScaffoldPlan::prepare(temp.path(), &root, &[], &[10], None, || Ok(2025)).unwrap();

        assert!(rerun.files.is_empty());
        rerun.apply().unwrap();
        assert_eq!(fs::read(&root).unwrap(), after);
    }
}

#[cfg(test)]
mod conflict_tests {
    use super::*;

    #[test]
    fn ambiguous_module_layouts_fail_before_writing() {
        for declaration in [
            "mod day4 {}",
            "#[cfg(feature=\"day\")] mod day4;",
            "#[path=\"elsewhere.rs\"] mod day4;",
            "mod day4; mod day04;",
        ] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("lib.rs");
            fs::write(
                &root,
                format!("{declaration}\naoc_harness::aoc_lib! {{ year=2025 }}\n"),
            )
            .unwrap();
            fs::write(temp.path().join("day4.rs"), "// preserve").unwrap();
            let original = fs::read(&root).unwrap();

            assert!(
                ScaffoldPlan::prepare(temp.path(), &root, &[], &[3, 4], None, || Ok(2025)).is_err(),
                "{declaration}"
            );
            assert_eq!(fs::read(&root).unwrap(), original);
            assert!(!temp.path().join("day03.rs").exists());
        }
    }

    #[test]
    fn documented_directory_modules_are_unchanged() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("lib.rs");
        let original =
            "/// Existing puzzle module\nmod day04;\naoc_harness::aoc_lib! { year=2025 }\n";
        fs::write(&root, original).unwrap();
        fs::create_dir(temp.path().join("day04")).unwrap();
        fs::write(temp.path().join("day04/mod.rs"), "// existing code").unwrap();

        ScaffoldPlan::prepare(temp.path(), &root, &[], &[4], None, || Ok(2025))
            .unwrap()
            .apply()
            .unwrap();

        assert_eq!(fs::read_to_string(root).unwrap(), original);
        assert!(!temp.path().join("day04.rs").exists());
        assert!(!temp.path().join("day4.rs").exists());
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_refused_and_root_permissions_are_preserved() {
        use std::os::unix::{fs::symlink, fs::PermissionsExt};
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("lib.rs");
        let linked = temp.path().join("linked.rs");
        fs::write(&root, "aoc_harness::aoc_lib! { year=2025 }\n").unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o640)).unwrap();
        symlink(&root, &linked).unwrap();

        assert!(ScaffoldPlan::prepare(temp.path(), &linked, &[], &[1], None, || Ok(2025)).is_err());
        ScaffoldPlan::prepare(temp.path(), &root, &[], &[1], None, || Ok(2025))
            .unwrap()
            .apply()
            .unwrap();

        assert_eq!(
            fs::metadata(root).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }
}

#[cfg(test)]
mod year_module_tests {
    use super::*;

    const LIBRARY: &str =
        "pub mod common;\nmod y2019;\naoc_harness::aoc_lib! { years = { 2019 => y2019 } }\n";

    fn module(path: &[&str]) -> Vec<String> {
        path.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn days_are_generated_beside_mod_rs_and_flat_year_modules() {
        for (source, existing) in [
            (
                "y2019/mod.rs",
                "//! The 2019 event.\nmod day1;\n\nfn helper() {}\n",
            ),
            (
                "y2019.rs",
                "//! The 2019 event.\nmod day1;\n\nfn helper() {}\n",
            ),
        ] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("lib.rs");
            let year = temp.path().join(source);
            fs::write(&root, LIBRARY).unwrap();
            fs::create_dir_all(year.parent().unwrap()).unwrap();
            fs::write(&year, existing).unwrap();
            if source == "y2019.rs" {
                fs::create_dir(temp.path().join("y2019")).unwrap();
            }
            fs::write(temp.path().join("y2019/day1.rs"), "// existing").unwrap();
            let template = temp.path().join("template.rs");
            fs::write(&template, "// @YEAR@ day @DAY@\n").unwrap();

            let plan = ScaffoldPlan::prepare(
                temp.path(),
                &root,
                &module(&["y2019"]),
                &[1, 2],
                Some(&template),
                || Ok(2019),
            )
            .unwrap();

            assert_eq!(fs::read_to_string(&root).unwrap(), LIBRARY);
            assert_eq!(plan.root, year, "{source}");
            assert_eq!(plan.files.len(), 1, "{source}");
            assert_eq!(plan.files[0].path, temp.path().join("y2019/day2.rs"));
            assert_eq!(plan.files[0].contents, "// 2019 day 2\n");
            assert_eq!(
                plan.replacement,
                "//! The 2019 event.\nmod day1;\nmod day2;\n\nfn helper() {}\n"
            );

            plan.apply().unwrap();

            assert_eq!(fs::read_to_string(&root).unwrap(), LIBRARY);
            assert!(fs::read_to_string(&year).unwrap().contains("mod day2;"));
            assert!(temp.path().join("y2019/day2.rs").is_file());
        }
    }

    #[test]
    fn a_flat_year_module_without_days_gets_a_directory_and_padded_names() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("lib.rs");
        fs::write(&root, LIBRARY).unwrap();
        fs::write(
            temp.path().join("y2019.rs"),
            "#![allow(dead_code)]\nuse std::fmt;\n",
        )
        .unwrap();

        let plan =
            ScaffoldPlan::prepare(temp.path(), &root, &module(&["y2019"]), &[3], None, || {
                Ok(2019)
            })
            .unwrap();

        assert_eq!(
            plan.replacement,
            "#![allow(dead_code)]\nmod day03;\n\nuse std::fmt;\n"
        );
        plan.apply().unwrap();
        assert!(temp.path().join("y2019/day03.rs").is_file());
    }

    #[test]
    fn nested_year_modules_follow_each_declaration() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("lib.rs");
        fs::write(
            &root,
            "mod events;\naoc_harness::aoc_lib! { years = { 2019 => events::y2019 } }\n",
        )
        .unwrap();
        fs::create_dir_all(temp.path().join("events")).unwrap();
        fs::write(temp.path().join("events.rs"), "pub mod y2019;\n").unwrap();
        fs::write(temp.path().join("events/y2019.rs"), "").unwrap();

        let plan = ScaffoldPlan::prepare(
            temp.path(),
            &root,
            &module(&["events", "y2019"]),
            &[4],
            None,
            || Ok(2019),
        )
        .unwrap();

        assert_eq!(plan.root, temp.path().join("events/y2019.rs"));
        assert_eq!(
            plan.files[0].path,
            temp.path().join("events/y2019/day04.rs")
        );
        assert_eq!(plan.replacement, "mod day04;\n");
    }

    #[test]
    fn ambiguous_year_module_layouts_fail_before_writing() {
        for (declaration, files) in [
            ("mod y2019 {}", &[][..]),
            ("#[path = \"old.rs\"] mod y2019;", &["old.rs"][..]),
            ("#[cfg(feature = \"old\")] mod y2019;", &["y2019.rs"][..]),
            ("mod y2019;", &["y2019.rs", "y2019/mod.rs"][..]),
            ("mod y2019;", &[][..]),
            ("", &["y2019.rs"][..]),
        ] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("lib.rs");
            let original =
                format!("{declaration}\naoc_harness::aoc_lib! {{ years = {{ 2019 => y2019 }} }}\n");
            fs::write(&root, &original).unwrap();
            fs::create_dir_all(temp.path().join("y2019")).unwrap();
            for file in files {
                fs::write(temp.path().join(file), "").unwrap();
            }

            let result =
                ScaffoldPlan::prepare(temp.path(), &root, &module(&["y2019"]), &[1], None, || {
                    Ok(2019)
                });

            assert!(
                result.err().unwrap().contains("year module `y2019`"),
                "{declaration} {files:?}"
            );
            assert_eq!(fs::read_to_string(&root).unwrap(), original);
            assert!(!temp.path().join("y2019/day01.rs").exists());
        }
    }
}
