use std::fs;
use std::path::Path;

use super::{copy_file, MigrationError, MigrationReport};

pub(super) fn copy_log_files(
    source: &Path,
    destination: &Path,
    log_file_name: &str,
    report_path: &str,
    report: &mut MigrationReport,
) -> Result<bool, MigrationError> {
    let mut copied = false;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let Some(file_name_str) = file_name.to_str() else {
            continue;
        };
        if !is_channel_log_file(file_name_str, log_file_name) {
            continue;
        }

        let source_path = entry.path();
        let destination_path = destination.join(&file_name);
        let metadata = fs::symlink_metadata(&source_path)?;
        if metadata.file_type().is_symlink() {
            report
                .skipped_paths
                .push(format!("{report_path}/{file_name_str}"));
        } else if metadata.is_file() {
            copy_file(&source_path, &destination_path)?;
            copied = true;
        }
    }
    Ok(copied)
}

fn is_channel_log_file(file_name: &str, log_file_name: &str) -> bool {
    if file_name == log_file_name {
        return true;
    }
    let Some(suffix) = file_name.strip_prefix(log_file_name) else {
        return false;
    };
    if matches!(suffix, ".recovery" | ".old.temp") {
        return true;
    }
    if let Some(index) = suffix.strip_prefix(".in_session.") {
        return index.parse::<usize>().is_ok();
    }
    let Some(old_suffix) = suffix.strip_prefix(".old.") else {
        return false;
    };
    if old_suffix.parse::<usize>().is_ok() {
        return true;
    }
    old_suffix
        .split_once(".in_session.")
        .is_some_and(|(slot, chunk)| {
            slot.parse::<usize>().is_ok() && chunk.parse::<usize>().is_ok()
        })
}
