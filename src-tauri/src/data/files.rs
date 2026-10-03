use super::{err, Result};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

fn resolved(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        fs::canonicalize(path).map_err(err)
    } else {
        Ok(fs::canonicalize(path.parent().ok_or("data_path")?)
            .map_err(err)?
            .join(path.file_name().ok_or("data_path")?))
    }
}
fn folded(path: &Path) -> String {
    let s = path.to_string_lossy().to_string();
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s
    }
}
fn aliases(a: &Path, b: &Path) -> Result<bool> {
    if folded(&resolved(a)?) == folded(&resolved(b)?) {
        return Ok(true);
    }
    if a.exists() && b.exists() {
        return same_file::is_same_file(a, b).map_err(err);
    }
    Ok(false)
}
/// Protect the whole application-data tree and hard-link aliases to its files.
pub fn allowed(target: &Path, app_data: &Path, imports: &[PathBuf]) -> Result<()> {
    let target = resolved(target)?;
    let root = resolved(app_data)?;
    if folded(&target).starts_with(&(folded(&root) + std::path::MAIN_SEPARATOR_STR))
        || aliases(&target, &root)?
    {
        return Err("data_protected_path".into());
    }
    #[cfg(windows)]
    if target
        .file_name()
        .is_some_and(|s| s.to_string_lossy().contains(':'))
    {
        return Err("data_protected_path".into());
    }
    for source in imports {
        if aliases(&target, source)? {
            return Err("data_protected_path".into());
        }
    }
    if target.exists() {
        let mut dirs = vec![root];
        while let Some(dir) = dirs.pop() {
            for item in fs::read_dir(dir).map_err(err)? {
                let item = item.map_err(err)?;
                let ty = item.file_type().map_err(err)?;
                if ty.is_dir() {
                    dirs.push(item.path());
                } else if !ty.is_symlink()
                    && same_file::is_same_file(&target, item.path()).map_err(err)?
                {
                    return Err("data_protected_path".into());
                }
            }
        }
    }
    Ok(())
}
/// Flush and close the temporary file before replacing the native-dialog-approved target.
/// TempPath::persist uses the platform's replace operation, never remove + create.
pub fn atomic_save(target: &Path, bytes: &[u8]) -> Result<()> {
    let parent = target.parent().ok_or("data_path")?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(err)?;
    temp.write_all(bytes).map_err(err)?;
    temp.as_file().sync_all().map_err(err)?;
    let path = temp.into_temp_path(); // closes the file handle (also required on Windows).
    path.persist(target).map_err(err)?;
    Ok(())
}
pub fn safe_filename(value: &str) -> String {
    let name: String = value
        .chars()
        .take(60)
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let name = name.trim().trim_end_matches(['.', ' ']);
    let stem = name.split('.').next().unwrap_or("").to_uppercase();
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if name.is_empty() {
        "all".into()
    } else if reserved.contains(&stem.as_str()) {
        format!("_{name}")
    } else {
        name.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replace_and_failure_preserve_target() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("report.csv");
        fs::write(&p, "original").unwrap();
        atomic_save(&p, b"new").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"new");
        let destination = dir.path().join("folder");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("keep"), "old").unwrap();
        assert!(atomic_save(&destination, b"bad").is_err());
        assert_eq!(fs::read(destination.join("keep")).unwrap(), b"old");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }
    #[cfg(windows)]
    #[test]
    fn windows_locked_target_is_not_destroyed() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("locked.csv");
        fs::write(&p, b"original").unwrap();
        let _lock = fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&p)
            .unwrap();
        assert!(atomic_save(&p, b"new").is_err());
        assert_eq!(fs::read(&p).unwrap(), b"original");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    #[test]
    fn paths_and_windows_names() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("app");
        fs::create_dir(&app).unwrap();
        let db = app.join("pomotroid.db");
        fs::write(&db, "db").unwrap();
        let alias = dir.path().join("alias.json");
        fs::hard_link(&db, &alias).unwrap();
        assert!(allowed(&alias, &app, &[]).is_err());
        assert!(allowed(&db, &app, &[]).is_err());
        let source = dir.path().join("import.json");
        fs::write(&source, "source").unwrap();
        assert!(allowed(&source, &app, &[source.clone()]).is_err());
        assert_eq!(safe_filename("CON"), "_CON");
        assert_eq!(safe_filename("a/b:c"), "a_b_c");
    }
}
