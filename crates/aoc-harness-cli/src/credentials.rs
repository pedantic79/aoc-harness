use std::path::{Path, PathBuf};

// Deliberately no Debug implementation: tokens must never appear in diagnostics.
pub(crate) struct Session {
    pub token: String,
    pub source: String,
}

pub(crate) fn resolve(explicit: Option<&Path>) -> Result<Session, String> {
    let environment = std::env::var_os("ADVENT_OF_CODE_SESSION").map(|v| {
        v.into_string()
            .map_err(|_| "ADVENT_OF_CODE_SESSION must be UTF-8".to_owned())
    });
    resolve_with(explicit, environment, || {
        #[cfg(unix)]
        {
            std::env::var_os("HOME")
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
        }
        #[cfg(not(unix))]
        {
            dirs::home_dir()
        }
    })
}

fn resolve_with(
    explicit: Option<&Path>,
    environment: Option<Result<String, String>>,
    home: impl FnOnce() -> Option<PathBuf>,
) -> Result<Session, String> {
    if let Some(path) = explicit {
        return read(path);
    }
    if let Some(value) = environment {
        let value = value?;
        if !value.trim().is_empty() {
            return validate(&value, "ADVENT_OF_CODE_SESSION".into());
        }
    }

    let home = home()
        .filter(|p| p.is_absolute())
        .ok_or("home directory unavailable; supply --session-file or ADVENT_OF_CODE_SESSION")?;
    read(&home.join(".adventofcode.session"))
}

fn read(path: &Path) -> Result<Session, String> {
    let text = std::fs::read_to_string(path).map_err(|_| format!(
        "cannot read session file {}; place the raw hexadecimal token there, or use --session-file or ADVENT_OF_CODE_SESSION", path.display()))?;
    validate(&text, path.display().to_string())
}

fn validate(text: &str, source: String) -> Result<Session, String> {
    let token = text.trim();
    if token.is_empty() || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!(
            "invalid session source {source}: expected nonempty raw ASCII hexadecimal text"
        ));
    }
    Ok(Session {
        token: token.into(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precedence_validation_and_read_only_shared_file() {
        let temp = tempfile::tempdir().unwrap();
        let default = temp.path().join(".adventofcode.session");
        let explicit = temp.path().join("override");
        std::fs::write(&default, " ab12\r\n").unwrap();
        std::fs::write(&explicit, "CD34\n").unwrap();
        let bytes = std::fs::read(&default).unwrap();
        let permissions = std::fs::metadata(&default).unwrap().permissions();
        let load = |file: Option<&Path>, env: &str| {
            resolve_with(file, Some(Ok(env.into())), || Some(temp.path().into()))
        };

        assert_eq!(load(None, " \n").unwrap().token, "ab12");
        assert_eq!(load(None, " ef56 ").unwrap().token, "ef56");
        assert_eq!(load(Some(&explicit), "bad-secret").unwrap().token, "CD34");
        assert!(load(None, "bad-secret")
            .err()
            .unwrap()
            .contains("ADVENT_OF_CODE_SESSION"));
        assert!(load(Some(&temp.path().join("absent")), "abcd").is_err());
        assert_eq!(std::fs::read(&default).unwrap(), bytes);
        assert_eq!(
            std::fs::metadata(&default).unwrap().permissions(),
            permissions
        );
        assert!(resolve_with(None, None, || None).is_err());
        assert!(resolve_with(None, Some(Ok("abcd".into())), || panic!(
            "unneeded home lookup"
        ))
        .is_ok());
        assert!(
            resolve_with(Some(&explicit), Some(Err("invalid unicode".into())), || {
                None
            })
            .is_ok()
        );

        for token in [
            "",
            "session=abcd",
            "ab cd",
            "ab\ncd",
            "\"abcd\"",
            "é",
            "[session]\ntoken='abcd'",
        ] {
            let error = validate(token, "synthetic file".into()).err().unwrap();

            assert!(!error.contains(token) || token.is_empty());
        }
    }
}
