use anyhow::{anyhow, Context, Result};
use std::env;
use std::fs;
use std::fs::DirEntry;
use std::path::Path;
use std::path::PathBuf;
use std::process::{Command, Output};

const JPG_FOLDER: &str = "JPG";
const RAW_ALLOWED_FILE_EXTENSIONS: &'static [&'static str] = &["arw"];
const JPG_ALLOWED_FILE_EXTENSIONS: &'static [&'static str] = &["jpg", "jpeg"];

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
            filename: filename,
            name: name,
            extension: extension,
        }
    }
}

fn get_pwd() -> Result<String> {
    let path = env::current_exe()
        .context("❌ Could not get current executable path")?;
    let path = path.parent().context("❌ Could not get parent directory")?;
    let path = path.to_str().context("❌ Could not get string from path")?;
    Ok(path.to_string())
}

fn get_filenames_of_folder(path: PathBuf) -> Result<Vec<Filename>> {
    match path.to_str() {
        Some(path) => fs::read_dir(path)
            .with_context(|| {format!("❌ Could not read directory {:?}", path)})
            .map(|dir| {
                dir.into_iter()
                    .filter_map(|file| match file {
                        Ok(file) => Some(file.into()),
                        _ => None,
                    })
                    .collect::<Vec<Filename>>()
            }),
        None => Err(anyhow!("❌ Could not get string from path")),
    }
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
        .map(|filename| {
            filename.name
        })
        .collect::<Vec<String>>();

    target_files
        .into_iter()
        .filter_map(|filename| {
            if compare_names.contains(&filename.name){
                None
            } else {
                Some(filename)
            }
        })
        .collect::<Vec<Filename>>()
}

fn convert_to_hfs_path(path: PathBuf) -> Result<String> {
    let path = path.to_str().context("❌ Could not convert path to string")?;
    let stdout = Command::new("osascript")
        .arg("-e")
        .arg(format!(r#"POSIX file "{}" as alias as text"#, path))
        .output()
        .context("❌ Cannot get HFS convert Output")?
        .stdout;
    match String::from_utf8(stdout) {
        Ok(s) => Ok(s.trim().to_string()),
        Err(_) => Err(anyhow!("❌ Could not convert to utf8")),
    }
}

fn print_result(output: &Output, file: &str) {
    if output.status.success() {
        println!("👍 Delete {}", file);
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        if error.contains("29:106") {
            println!("❌ Cannot find the file {:?}", file);
        } else {
            println!("❌ {:?}", &error);
        }
    }
}

fn delete_files(file: String, hfs_folder_path: String) -> Result<()> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(format!(
            r#"tell application "Finder" to delete (file "{}" of folder "{}")"#,
            file, hfs_folder_path
        ))
        .output();
    match output {
        Err(_) => {
            println!("⚠️⚠️⚠️ Cannot delete file {} ⚠️⚠️⚠️", file);
        },
        Ok(output) => {
            print_result(&output, &file);
        }
    };
    Ok(())
}

fn main() -> Result<()> {
    println!("🚀 Start deleting duplicated files");

    let raw_folder_path = get_pwd()?;
    let raw_folder_path = Path::new(&raw_folder_path).to_path_buf();
    let jpg_folder_path = Path::new(&raw_folder_path)
        .join(JPG_FOLDER);
    let jpg_folder_path_hfs = convert_to_hfs_path(jpg_folder_path.clone())?;

    let raw_files =
        get_filenames_of_folder_with_valid_extension(raw_folder_path, RAW_ALLOWED_FILE_EXTENSIONS.into())?;
    let jpg_files =
        get_filenames_of_folder_with_valid_extension(jpg_folder_path, JPG_ALLOWED_FILE_EXTENSIONS.into())?;
    let unused_files_in_jpg_folder = find_duplicate_file(raw_files, jpg_files);

    unused_files_in_jpg_folder.iter().for_each(|file| {
        let _ = delete_files(file.filename.clone(), jpg_folder_path_hfs.clone());
    });

    println!("✅ Done");

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_get_filename_and_extension() {
        assert_eq!(2 + 2, 4);
    }
}
