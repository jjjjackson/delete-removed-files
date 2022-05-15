use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::fs::DirEntry;
use std::path::Path;
use std::path::PathBuf;
use std::process::{Command, Output};

const JPG_FOLDER: &str = "JPG";
const RAW_ALLOWED_FILE_EXTENSIONS: &[&str] = &["arw"];
const JPG_ALLOWED_FILE_EXTENSIONS: &[&str] = &["jpg", "jpeg"];

struct Filename {
    filename: String,
    name: String,
    extension: String,
}

impl From<DirEntry> for Filename {
    fn from(dir_entry: DirEntry) -> Self {
        let path = dir_entry.path();
        let filename = path
            .file_name()
            .map(|s| s.to_str().or(Some("")).unwrap())
            .or(Some(""))
            .unwrap()
            .to_string();
        let name = path
            .file_stem()
            .map(|s| s.to_str().or(Some("")).unwrap())
            .or(Some(""))
            .unwrap()
            .to_string();
        let extension = path
            .extension()
            .map(|s| s.to_str().or(Some("")).unwrap())
            .or(Some(""))
            .unwrap()
            .to_string()
            .to_lowercase();
        Self {
            filename,
            name,
            extension,
        }
    }
}

fn get_pwd() -> Result<String> {
    // Didn't use `env::current_dir()` because it doesn't work well with external hard drives
    let path = env::current_exe()
        .context("❌ Could not get current executable path")?;
    let path = path.parent().context("❌ Could not get parent directory")?;
    let path = path.to_str().context("❌ Could not get string from path")?;
    Ok(path.to_string())
}

fn get_filenames_of_folder(path: PathBuf) -> Result<Vec<Filename>> {
    fs::read_dir(path.clone())
        .with_context(|| format!("❌ Could not read directory {:?}", path))
        .map(|dir| {
            dir.into_iter()
                .filter_map(|file| match file {
                    Ok(file) => Some(file.into()),
                    _ => None,
                })
                .collect::<Vec<Filename>>()
        })
}

fn get_filenames_of_folder_with_valid_extension(
    path: PathBuf,
    allowed_extensions: Vec<&str>,
) -> Result<Vec<Filename>> {
    get_filenames_of_folder(path).map(|files| {
        files
            .into_iter()
            .filter(|file| {
                allowed_extensions.contains(&file.extension.as_str())
            })
            .collect::<Vec<Filename>>()
    })
}

fn find_duplicate_file(
    compare_files: Vec<Filename>,
    target_files: Vec<Filename>,
) -> Vec<Filename> {
    let compare_names = compare_files
        .into_iter()
        .map(|filename| filename.name)
        .collect::<Vec<String>>();

    target_files
        .into_iter()
        .filter(|filename| !compare_names.contains(&filename.name))
        .collect::<Vec<Filename>>()
}

fn convert_to_hfs_path(path: PathBuf) -> Result<String> {
    let stdout = path
        .to_str()
        .with_context(|| {
            format!("❌ Could not get string from path. {:?}", path)
        })
        .and_then(|path| {
            Command::new("osascript")
                .arg("-e")
                .arg(format!(r#"POSIX file "{}" as alias as text"#, path))
                .output()
                .with_context(|| {
                    format!("❌ Cannot get HFS convert Output {}", path)
                })
                .map(|op| op.stdout)
        })?;

    String::from_utf8(stdout)
        .context("❌ Could not convert to utf8")
        .map(|s| s.trim().to_string())
}

fn print_result(output: &Output, file: &str) {
    if output.status.success() {
        println!("👍 Deleted {}", file);
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        if error.contains("29:106") {
            println!(
                "❌ Could not delete file cause couldn't find the file {:?}",
                file
            );
        } else {
            println!("❌ {:?}", &error);
        }
    }
}

fn delete_files(filename: Filename, hfs_folder_path: String) -> Result<()> {
    let filename = filename.filename;
    Command::new("osascript")
        .arg("-e")
        .arg(format!(
            r#"tell application "Finder" to delete (file "{}" of folder "{}")"#,
            filename, hfs_folder_path
        ))
        .output()
        .with_context(|| {
            format!(
                "❌ Could not delete file {:?} of folder {:?}, cause the command failed",
                filename, hfs_folder_path
            )
        }).map(|output| {
            print_result(&output, &filename);
        })
}

fn main() -> Result<()> {
    println!("🚀 Start deleting duplicated files");

    let raw_folder_path = get_pwd()?;
    let raw_folder_path = Path::new(&raw_folder_path).to_path_buf();
    let jpg_folder_path = Path::new(&raw_folder_path).join(JPG_FOLDER);
    let jpg_folder_path_hfs = convert_to_hfs_path(jpg_folder_path.clone())?;

    let raw_files = get_filenames_of_folder_with_valid_extension(
        raw_folder_path,
        RAW_ALLOWED_FILE_EXTENSIONS.into(),
    )?;
    let jpg_files = get_filenames_of_folder_with_valid_extension(
        jpg_folder_path,
        JPG_ALLOWED_FILE_EXTENSIONS.into(),
    )?;
    let unused_files_in_jpg_folder = find_duplicate_file(raw_files, jpg_files);

    unused_files_in_jpg_folder.into_iter().for_each(|file| {
        let result = delete_files(file, jpg_folder_path_hfs.clone());
        if result.is_err() {
            println!("{:?}", result.err());
        }
    });

    println!("✅ Done");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::fs::File;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn create_files(folder_path: PathBuf, number_of_files: usize, ext: &str) {
        (0..number_of_files).into_iter().for_each(|i| {
            let file = folder_path.join(format!("test{}.{}", i, ext));
            File::create(file.clone()).unwrap();
        });
    }

    #[test]
    fn test_get_filenames_of_folder() {
        let number_of_files = 10;
        let tmp_dir = tempdir().unwrap();
        let raw_folder = tmp_dir.path();

        create_files(raw_folder.into(), number_of_files, "arw");

        let filenames = get_filenames_of_folder(raw_folder.into()).unwrap();
        assert_eq!(filenames.len(), number_of_files);

        tmp_dir.close().unwrap();
    }

    #[test]
    fn test_get_filenames_of_folder_with_valid_extension() {
        let number_of_files = 10;
        let tmp_dir = tempdir().unwrap();
        let raw_folder = tmp_dir.path();

        create_files(raw_folder.into(), number_of_files, "arw");
        create_files(raw_folder.into(), number_of_files, "jpg");

        let filenames = get_filenames_of_folder_with_valid_extension(
            raw_folder.into(),
            vec!["arw"],
        )
        .unwrap();

        assert_eq!(filenames.len(), number_of_files);

        tmp_dir.close().unwrap();
    }

    #[test]
    fn test_get_filename_and_extension() {
        let number_of_raw_files = 10;
        let number_of_jpg_files = 11;
        let tmp_dir = tempdir().unwrap();
        let raw_folder = tmp_dir.path();
        let jpg_folder = raw_folder.join("JPG");
        fs::create_dir(jpg_folder.clone()).unwrap();

        create_files(raw_folder.clone().into(), number_of_raw_files, "arw");
        create_files(jpg_folder.clone().into(), number_of_jpg_files, "jpg");

        let raw_files = get_filenames_of_folder_with_valid_extension(
            raw_folder.into(),
            vec!["arw"],
        )
        .unwrap();
        let jpg_files = get_filenames_of_folder_with_valid_extension(
            jpg_folder.into(),
            vec!["jpg"],
        )
        .unwrap();

        let unused_files_in_jpg_folder = find_duplicate_file(raw_files, jpg_files);

        assert_eq!(unused_files_in_jpg_folder.len(), number_of_jpg_files - number_of_raw_files);
        tmp_dir.close().unwrap();
    }
}
