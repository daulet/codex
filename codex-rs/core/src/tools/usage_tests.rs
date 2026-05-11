use super::*;
use codex_protocol::protocol::FileChange;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::path::PathBuf;

#[test]
fn model_read_usage_counts_text_lines() {
    assert_eq!(
        model_read_usage_for_text("one\ntwo\n"),
        ToolUsage {
            model_read_line_count: Some(2),
            ..Default::default()
        }
    );
}

#[test]
fn file_edit_usage_counts_added_deleted_and_files() {
    let changes = HashMap::from([
        (
            PathBuf::from("added.txt"),
            FileChange::Add {
                content: "a\nb\n".to_string(),
            },
        ),
        (
            PathBuf::from("deleted.txt"),
            FileChange::Delete {
                content: "old\n".to_string(),
            },
        ),
        (
            PathBuf::from("updated.txt"),
            FileChange::Update {
                unified_diff: "@@ -1,2 +1,2 @@\n same\n-old\n+new\n".to_string(),
                move_path: None,
            },
        ),
    ]);

    assert_eq!(
        file_edit_usage_for_changes(&changes),
        ToolUsage {
            file_edit_line_count: Some(5),
            file_edit_added_line_count: Some(3),
            file_edit_deleted_line_count: Some(2),
            file_edit_file_count: Some(3),
            ..Default::default()
        }
    );
}
