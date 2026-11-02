use crate::error::{Result, ZError};
use std::fs;
use std::path::{Path, PathBuf};

pub fn write_string_to_file(in_string: &String, file_path: &Path) -> Result<()> {
    let temp_path = temp_path_for(file_path)?;
    if let Err(e) =
        fs::write(&temp_path, in_string).and_then(|()| fs::rename(&temp_path, file_path))
    {
        let _ = fs::remove_file(&temp_path);
        return Err(e.into());
    }
    Ok(())
}

fn temp_path_for(file_path: &Path) -> Result<PathBuf> {
    let mut temp_name = file_path
        .file_name()
        .ok_or_else(|| ZError::Message(format!("{} has no file name", file_path.display())))?
        .to_os_string();
    temp_name.push(".tmp");
    Ok(file_path.with_file_name(temp_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write(contents: &str, path: &Path) -> Result<()> {
        write_string_to_file(&contents.to_string(), path)
    }

    fn file_names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn write_creates_file_with_content() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("a.json");
        write("new", &path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
    }

    #[test]
    fn write_replaces_longer_old_content() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("a.json");
        fs::write(&path, "old content that is longer").unwrap();
        write("new", &path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
    }

    #[test]
    fn write_leaves_no_temp_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("a.json");
        write("first", &path).unwrap();
        write("second", &path).unwrap();
        assert_eq!(file_names(tmp.path()), ["a.json"]);
    }

    #[test]
    fn write_replaces_file_instead_of_truncating_it() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("a.json");
        let hard_link = tmp.path().join("link.json");
        fs::write(&path, "old").unwrap();
        fs::hard_link(&path, &hard_link).unwrap();
        write("new", &path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        assert_eq!(fs::read_to_string(&hard_link).unwrap(), "old");
    }

    #[test]
    fn write_into_missing_dir_fails_without_stray_file() {
        let tmp = TempDir::new().unwrap();
        assert!(write("new", &tmp.path().join("missing").join("a.json")).is_err());
        assert!(file_names(tmp.path()).is_empty());
    }

    #[test]
    fn write_onto_directory_fails_and_removes_temp_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("a.json");
        fs::create_dir(&path).unwrap();
        assert!(write("new", &path).is_err());
        assert_eq!(file_names(tmp.path()), ["a.json"]);
    }
}
