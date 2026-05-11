use codex_otel::ToolUsage;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::function_call_output_content_items_to_text;
use codex_protocol::protocol::FileChange;
use std::collections::HashMap;
use std::path::PathBuf;

pub(crate) fn model_read_usage_for_text(text: &str) -> ToolUsage {
    ToolUsage {
        model_read_line_count: Some(line_count(text)),
        ..Default::default()
    }
}

pub(crate) fn model_read_usage_for_content_items(
    items: &[FunctionCallOutputContentItem],
) -> ToolUsage {
    let line_count = function_call_output_content_items_to_text(items)
        .map(|text| line_count(&text))
        .unwrap_or(0);
    ToolUsage {
        model_read_line_count: Some(line_count),
        ..Default::default()
    }
}

pub(crate) fn model_read_usage_for_payload(payload: &FunctionCallOutputPayload) -> ToolUsage {
    match &payload.body {
        FunctionCallOutputBody::Text(text) => model_read_usage_for_text(text),
        FunctionCallOutputBody::ContentItems(items) => model_read_usage_for_content_items(items),
    }
}

pub(crate) fn merge_usage(base: ToolUsage, overlay: ToolUsage) -> ToolUsage {
    ToolUsage {
        model_read_line_count: overlay.model_read_line_count.or(base.model_read_line_count),
        file_edit_line_count: overlay.file_edit_line_count.or(base.file_edit_line_count),
        file_edit_added_line_count: overlay
            .file_edit_added_line_count
            .or(base.file_edit_added_line_count),
        file_edit_deleted_line_count: overlay
            .file_edit_deleted_line_count
            .or(base.file_edit_deleted_line_count),
        file_edit_file_count: overlay.file_edit_file_count.or(base.file_edit_file_count),
    }
}

pub(crate) fn file_edit_usage_for_changes(changes: &HashMap<PathBuf, FileChange>) -> ToolUsage {
    let mut added = 0_i64;
    let mut deleted = 0_i64;

    for change in changes.values() {
        let (change_added, change_deleted) = file_change_line_counts(change);
        added = added.saturating_add(change_added);
        deleted = deleted.saturating_add(change_deleted);
    }

    ToolUsage {
        file_edit_line_count: Some(added.saturating_add(deleted)),
        file_edit_added_line_count: Some(added),
        file_edit_deleted_line_count: Some(deleted),
        file_edit_file_count: Some(usize_to_i64(changes.len())),
        ..Default::default()
    }
}

fn file_change_line_counts(change: &FileChange) -> (i64, i64) {
    match change {
        FileChange::Add { content } => (line_count(content), 0),
        FileChange::Delete { content } => (0, line_count(content)),
        FileChange::Update { unified_diff, .. } => unified_diff_line_counts(unified_diff),
    }
}

fn unified_diff_line_counts(diff: &str) -> (i64, i64) {
    let mut added = 0_i64;
    let mut deleted = 0_i64;

    for line in diff.lines() {
        if line.starts_with("+++") || line.starts_with("---") {
            continue;
        }
        if line.starts_with('+') {
            added = added.saturating_add(1);
        } else if line.starts_with('-') {
            deleted = deleted.saturating_add(1);
        }
    }

    (added, deleted)
}

fn line_count(text: &str) -> i64 {
    usize_to_i64(text.lines().count())
}

fn usize_to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
#[path = "usage_tests.rs"]
mod tests;
