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

impl ScaffoldPlan {
    pub fn prepare(package: &Path, root: &Path, days: &[u8]) -> Result<Self, String> {
        safe_source(package, root)?;
        let original =
            fs::read_to_string(root).map_err(|e| format!("read source {}: {e}", root.display()))?;
        let parsed = syn::parse_file(&original)
            .map_err(|e| format!("parse source {}: {e}", root.display()))?;
        let permissions = fs::metadata(root)
            .map_err(|e| format!("source permissions: {e}"))?
            .permissions();
        let entries: Vec<_> = parsed.items.iter().filter(|item| matches!(item, Item::Macro(m) if m.mac.path.segments.last().is_some_and(|s| s.ident == "aoc_lib"))).collect();
        if entries.len() != 1 {
            return Err(
                "scaffolding requires exactly one direct aoc_lib! invocation in the library root"
                    .into(),
            );
        }
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
        for day in days {
            let mut names = vec![format!("day{day}"), format!("day{day:02}")];
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
                        root.parent().unwrap().join(format!("{n}.rs")),
                        root.parent().unwrap().join(n).join("mod.rs"),
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
            let directory_modules = names
                .iter()
                .any(|n| root.parent().unwrap().join(n).exists());
            if modules.len() == 1 && existing.len() == 1 {
                let module = modules[0];
                let expected = root.parent().unwrap().join(format!("{}.rs", module.ident));
                let directory_source = root
                    .parent()
                    .unwrap()
                    .join(module.ident.to_string())
                    .join("mod.rs");
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
                return Err(format!("day {day} has ambiguous/conflicting modules or files; resolve inline, cfg/path, padded declarations, and file/declaration mismatches manually"));
            }
            let path = paths[0].clone();
            let contents = include_str!("../templates/day.rs").replace("@DAY@", &day.to_string());
            files.push(NewFile {
                path,
                contents: contents.replace('\n', newline),
            });
            declarations.push_str(&format!("mod day{day};{newline}"));
        }
        let mut replacement = original.clone();
        if !files.is_empty() {
            let start = entries[0].span().start();
            let offset = original
                .split_inclusive('\n')
                .take(start.line - 1)
                .map(str::len)
                .sum::<usize>()
                + start.column;
            replacement.insert_str(offset, &format!("{declarations}{newline}"));
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
        self.apply_to(&mut std::io::stdout())
    }

    pub fn apply_to(self, progress: &mut dyn Write) -> Result<(), String> {
        self.apply_with(
            |temporary, root| {
                temporary
                    .persist(root)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            },
            progress,
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
        let plan = ScaffoldPlan::prepare(temp.path(), &root, &[4, 5]).unwrap();

        assert_eq!(fs::read_to_string(&root).unwrap(), original);
        assert!(plan
            .replacement
            .contains("mod day4;\r\nmod day5;\r\n\r\naoc_lib!"));
        plan.apply().unwrap();
        let after = fs::read(&root).unwrap();
        ScaffoldPlan::prepare(temp.path(), &root, &[4, 5])
            .unwrap()
            .apply()
            .unwrap();

        assert_eq!(fs::read(&root).unwrap(), after);
        fs::write(temp.path().join("day7.rs"), "conflict").unwrap();
        assert!(ScaffoldPlan::prepare(temp.path(), &root, &[6, 7]).is_err());
        assert!(!temp.path().join("day6.rs").exists());
        let plan = ScaffoldPlan::prepare(temp.path(), &root, &[6]).unwrap();
        fs::write(&root, "// user edit").unwrap();

        assert!(plan.apply().is_err());
        assert!(!temp.path().join("day6.rs").exists());
    }

    #[test]
    fn root_failure_rolls_back_and_padded_modules_are_preserved() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("lib.rs");
        fs::write(&root, "mod day04;\naoc_harness::aoc_lib! { year = 2025 }\n").unwrap();
        fs::write(temp.path().join("day04.rs"), "// existing").unwrap();
        let plan = ScaffoldPlan::prepare(temp.path(), &root, &[4, 5]).unwrap();

        assert_eq!(plan.files.len(), 1);
        assert!(plan
            .apply_with(
                |_, _| Err("injected write failure".into()),
                &mut std::io::stdout()
            )
            .is_err());
        assert!(!temp.path().join("day5.rs").exists());
        assert_eq!(
            fs::read_to_string(temp.path().join("day04.rs")).unwrap(),
            "// existing"
        );
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
                ScaffoldPlan::prepare(temp.path(), &root, &[3, 4]).is_err(),
                "{declaration}"
            );
            assert_eq!(fs::read(&root).unwrap(), original);
            assert!(!temp.path().join("day3.rs").exists());
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

        ScaffoldPlan::prepare(temp.path(), &root, &[4])
            .unwrap()
            .apply()
            .unwrap();

        assert_eq!(fs::read_to_string(root).unwrap(), original);
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

        assert!(ScaffoldPlan::prepare(temp.path(), &linked, &[1]).is_err());
        ScaffoldPlan::prepare(temp.path(), &root, &[1])
            .unwrap()
            .apply()
            .unwrap();

        assert_eq!(
            fs::metadata(root).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }
}
