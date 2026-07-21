use std::path::{Path, PathBuf};

const RESERVED_CHARS: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
const MAX_FILENAME_UTF16_UNITS: usize = 180;

fn is_windows_device_name(name: &str) -> bool {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(name)
        .trim()
        .to_ascii_uppercase();

    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|index| {
                matches!(index, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            })
}

fn truncate_filename(name: &str) -> String {
    if name.encode_utf16().count() <= MAX_FILENAME_UTF16_UNITS {
        return name.to_string();
    }

    let path = Path::new(name);
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty() && value.encode_utf16().count() <= 16);
    let suffix = extension
        .map(|value| format!(".{value}"))
        .unwrap_or_default();
    let stem = extension
        .and_then(|_| path.file_stem())
        .and_then(|value| value.to_str())
        .unwrap_or(name);
    let stem_limit = MAX_FILENAME_UTF16_UNITS.saturating_sub(suffix.encode_utf16().count());
    let mut used = 0;
    let shortened = stem
        .chars()
        .take_while(|character| {
            let next = used + character.len_utf16();
            if next > stem_limit {
                return false;
            }
            used = next;
            true
        })
        .collect::<String>();
    format!("{shortened}{suffix}")
}

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

    let mut output = output.trim().trim_matches('.').to_string();
    if output.is_empty() {
        "download".to_string()
    } else {
        if is_windows_device_name(&output) {
            output.insert(0, '_');
        }
        truncate_filename(&output)
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
