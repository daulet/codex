use std::cmp::Reverse;

use codex_rollout::ThreadTree;
use ratatui::style::Stylize;
use ratatui::text::Line;

use crate::app_event::AppEvent;
use crate::bottom_pane::ColumnWidthMode;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionRowDisplay;
use crate::bottom_pane::SelectionViewParams;

pub(crate) fn build_thread_tree_params(tree: ThreadTree) -> SelectionViewParams {
    let flattened_turns = flattened_turns(&tree);
    let initial_selected_idx = tree
        .active_leaf_turn_id
        .as_deref()
        .and_then(|active_id| {
            flattened_turns
                .iter()
                .position(|turn| tree.turns[turn.index].turn_id == active_id)
        })
        .map(|index| index + 1);

    let mut items = vec![SelectionItem {
        name: "root".to_string(),
        description: Some("Before the first user turn".to_string()),
        is_current: tree.active_leaf_turn_id.is_none(),
        dismiss_on_select: true,
        actions: vec![Box::new(|tx| {
            tx.send(AppEvent::NavigateThreadTree {
                target_turn_id: None,
            });
        })],
        search_value: Some("root start beginning".to_string()),
        ..Default::default()
    }];

    for flattened in flattened_turns {
        let turn = &tree.turns[flattened.index];
        let turn_id = turn.turn_id.clone();
        let message = turn
            .user_message
            .as_deref()
            .map(shorten_message)
            .unwrap_or_else(|| "(no user message)".to_string());
        let child_count = turn.children.len();
        let active_leaf = tree.active_leaf_turn_id.as_deref() == Some(turn.turn_id.as_str());
        let mut description = format!("turn {}", short_turn_id(&turn.turn_id));
        if child_count > 1 {
            description.push_str(&format!(" - {child_count} branches"));
        }
        if turn.is_active_path {
            description.push_str(" - active path");
        }
        items.push(SelectionItem {
            name: message,
            name_prefix_spans: vec![flattened.prefix.dim()],
            description: Some(description),
            is_current: active_leaf,
            dismiss_on_select: true,
            actions: vec![Box::new(move |tx| {
                tx.send(AppEvent::NavigateThreadTree {
                    target_turn_id: Some(turn_id.clone()),
                });
            })],
            search_value: Some(format!(
                "{} {}",
                turn.turn_id,
                turn.user_message.as_deref().unwrap_or_default()
            )),
            ..Default::default()
        });
    }

    SelectionViewParams {
        title: Some("Conversation Tree".to_string()),
        subtitle: Some("Select a turn to make that branch active".to_string()),
        footer_hint: Some(Line::from(vec![
            "Enter".bold(),
            " switch branch  ".dim(),
            "Esc".bold(),
            " close".dim(),
        ])),
        items,
        initial_selected_idx,
        is_searchable: true,
        search_placeholder: Some("Search branches".to_string()),
        col_width_mode: ColumnWidthMode::Fixed,
        row_display: SelectionRowDisplay::SingleLine,
        ..Default::default()
    }
}

struct FlattenedTurn {
    index: usize,
    prefix: String,
}

fn flattened_turns(tree: &ThreadTree) -> Vec<FlattenedTurn> {
    let mut flattened = Vec::new();
    let mut stack = ordered_roots(tree)
        .into_iter()
        .rev()
        .map(|index| (index, String::new(), String::new()))
        .collect::<Vec<_>>();
    while let Some((index, row_prefix, child_prefix)) = stack.pop() {
        flattened.push(FlattenedTurn {
            index,
            prefix: row_prefix,
        });

        let children = ordered_children(tree, index);
        let child_count = children.len();
        for (child_position, child_index) in children.into_iter().enumerate().rev() {
            let is_last = child_position + 1 == child_count;
            let branch = if is_last { "`- " } else { "|- " };
            let continuation = if is_last { "   " } else { "|  " };
            stack.push((
                child_index,
                format!("{child_prefix}{branch}"),
                format!("{child_prefix}{continuation}"),
            ));
        }
    }
    flattened
}

fn ordered_roots(tree: &ThreadTree) -> Vec<usize> {
    let mut roots = tree.roots.clone();
    roots.sort_by_key(|index| active_sort_key(tree, *index));
    roots
}

fn ordered_children(tree: &ThreadTree, index: usize) -> Vec<usize> {
    let mut children = tree.turns[index].children.clone();
    children.sort_by_key(|child_index| active_sort_key(tree, *child_index));
    children
}

fn active_sort_key(tree: &ThreadTree, index: usize) -> (Reverse<bool>, Option<i64>, usize) {
    (
        Reverse(tree.turns[index].is_active_path),
        tree.turns[index].started_at,
        index,
    )
}

fn shorten_message(message: &str) -> String {
    let first_line = message.lines().next().unwrap_or_default().trim();
    const MAX_CHARS: usize = 80;
    if first_line.chars().count() <= MAX_CHARS {
        return first_line.to_string();
    }
    let mut shortened = first_line.chars().take(MAX_CHARS - 3).collect::<String>();
    shortened.push_str("...");
    shortened
}

fn short_turn_id(turn_id: &str) -> &str {
    turn_id.get(..8).unwrap_or(turn_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_rollout::ThreadTreeTurn;
    use insta::assert_snapshot;

    #[test]
    fn renders_branch_labels() {
        let tree = ThreadTree {
            active_leaf_turn_id: Some("turn-3b".to_string()),
            roots: vec![0],
            turns: vec![
                ThreadTreeTurn {
                    turn_id: "turn-1".to_string(),
                    parent_turn_id: None,
                    children: vec![1, 2],
                    depth: 0,
                    user_message: Some("first".to_string()),
                    started_at: None,
                    completed_at: None,
                    is_active_path: true,
                    rollout_start_index: 0,
                    rollout_end_index: 1,
                },
                ThreadTreeTurn {
                    turn_id: "turn-2a".to_string(),
                    parent_turn_id: Some("turn-1".to_string()),
                    children: Vec::new(),
                    depth: 1,
                    user_message: Some("second A".to_string()),
                    started_at: None,
                    completed_at: None,
                    is_active_path: false,
                    rollout_start_index: 1,
                    rollout_end_index: 2,
                },
                ThreadTreeTurn {
                    turn_id: "turn-3b".to_string(),
                    parent_turn_id: Some("turn-1".to_string()),
                    children: Vec::new(),
                    depth: 1,
                    user_message: Some("second B".to_string()),
                    started_at: None,
                    completed_at: None,
                    is_active_path: true,
                    rollout_start_index: 2,
                    rollout_end_index: 3,
                },
            ],
        };

        let params = build_thread_tree_params(tree);
        let rows = params
            .items
            .iter()
            .map(|item| {
                let prefix = item
                    .name_prefix_spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>();
                format!(
                    "{prefix}{} -- {}",
                    item.name,
                    item.description.as_deref().unwrap()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert_snapshot!(rows);
    }
}
