use crate::Error;
use std::path::Path;

/// A portable directory component that cannot overlap Criterion's working data.
pub fn name(value: &str) -> Result<String, String> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.contains(['/', '\\'])
        || value
            .chars()
            .any(|c| c.is_control() || "<>:\"|?*".contains(c))
        || value.ends_with(['.', ' '])
        || ["base", "new", "change", "report", "profile"]
            .contains(&value.to_ascii_lowercase().as_str())
    {
        return Err("baseline NAME must be a single directory component other than base, new, change, report, or profile".into());
    }

    Ok(value.into())
}

pub(crate) fn require(report: &Path, name: &str, labels: &[String]) -> Result<(), Error> {
    // Criterion sanitizes and truncates directory names. Its metadata retains the
    // original workload identity, including when two names need numeric suffixes.
    let mut saved = Vec::new();
    match std::fs::read_dir(report) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry.map_err(|e| Error::message(format!("read baselines: {e}")))?;
                let path = entry.path().join(name);
                if !path.join("sample.json").is_file() || !path.join("estimates.json").is_file() {
                    continue;
                }
                let Ok(bytes) = std::fs::read(path.join("benchmark.json")) else {
                    continue;
                };
                let Ok(metadata) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
                    continue;
                };
                if let Some(label) = metadata["group_id"].as_str() {
                    saved.push(label.to_owned());
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(Error::message(format!("read {}: {e}", report.display()))),
    }

    let missing: Vec<_> = labels
        .iter()
        .filter(|label| !saved.contains(label))
        .collect();
    if !missing.is_empty() {
        return Err(Error::message(format!(
            "baseline {name:?} is missing for {}; save the selected workloads with --save-baseline {name}",
            missing.into_iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        )));
    }

    Ok(())
}
