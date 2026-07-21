use std::path::{Path, PathBuf};

const RESERVED_CHARS: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

pub fn sanitize_filename(input: &str) -> String {
    let mut output = input
        .chars()
        .map(|character| {
            if RESERVED_CHARS.contains(&character) || character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();

    while output.contains("  ") {
        output = output.replace("  ", " ");
    }

    let output = output.trim().trim_matches('.').to_string();
    if output.is_empty() {
        "download".to_string()
    } else {
        output
    }
}

pub fn next_available_path<F>(path: PathBuf, exists: F) -> PathBuf
where
    F: Fn(&Path) -> bool,
{
    if !exists(&path) {
        return path;
    }

    let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("download");
    let extension = path.extension().and_then(|value| value.to_str());

    for index in 1..10_000 {
        let file_name = match extension {
            Some(extension) => format!("{stem} ({index}).{extension}"),
            None => format!("{stem} ({index})"),
        };
        let candidate = parent.join(file_name);
        if !exists(&candidate) {
            return candidate;
        }
    }

    parent.join(format!("{stem} (copy).{}", extension.unwrap_or("part")))
}

pub fn validate_output_dir(path: &str) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("Choose an output folder before downloading.".to_string());
    }

    let output_dir = Path::new(path);
    if !output_dir.exists() {
        return Err("Output folder does not exist.".to_string());
    }
    if !output_dir.is_dir() {
        return Err("Output path must be a folder.".to_string());
    }

    Ok(())
}
