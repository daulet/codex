use std::collections::HashMap;
use std::collections::HashSet;

use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadTree {
    pub turns: Vec<ThreadTreeTurn>,
    pub roots: Vec<usize>,
    pub active_leaf_turn_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadTreeTurn {
    pub turn_id: String,
    pub parent_turn_id: Option<String>,
    pub children: Vec<usize>,
    pub depth: usize,
    pub user_message: Option<String>,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub is_active_path: bool,
    pub rollout_start_index: usize,
    pub rollout_end_index: usize,
}

#[derive(Debug)]
struct PendingTurn {
    turn_id: String,
    parent_turn_id: Option<String>,
    user_message: Option<String>,
    started_at: Option<i64>,
    completed_at: Option<i64>,
    started_explicitly: bool,
    started_from_response_item: bool,
    rollout_start_index: usize,
    rollout_end_index: usize,
}

pub fn build_thread_tree(items: &[RolloutItem]) -> ThreadTree {
    let mut turns = Vec::new();
    let mut turn_index_by_id = HashMap::new();
    let mut current_leaf_turn_id: Option<String> = None;
    let mut pending_turn: Option<PendingTurn> = None;

    for (index, item) in items.iter().enumerate() {
        match item {
            RolloutItem::EventMsg(EventMsg::TurnStarted(event)) => {
                finish_pending_turn(
                    &mut pending_turn,
                    &mut turns,
                    &mut turn_index_by_id,
                    &mut current_leaf_turn_id,
                );
                pending_turn = Some(PendingTurn {
                    turn_id: event.turn_id.clone(),
                    parent_turn_id: current_leaf_turn_id.clone(),
                    user_message: None,
                    started_at: event.started_at,
                    completed_at: None,
                    started_explicitly: true,
                    started_from_response_item: false,
                    rollout_start_index: index,
                    rollout_end_index: index + 1,
                });
            }
            RolloutItem::EventMsg(EventMsg::UserMessage(event)) => {
                if let Some(turn) = pending_turn.as_ref()
                    && turn.user_message.is_some()
                    && !turn.started_explicitly
                    && !(turn.started_from_response_item
                        && turn.user_message.as_deref() == Some(event.message.as_str()))
                {
                    finish_pending_turn(
                        &mut pending_turn,
                        &mut turns,
                        &mut turn_index_by_id,
                        &mut current_leaf_turn_id,
                    );
                }
                let turn = pending_turn.get_or_insert_with(|| PendingTurn {
                    turn_id: fallback_turn_id(index),
                    parent_turn_id: current_leaf_turn_id.clone(),
                    user_message: None,
                    started_at: None,
                    completed_at: None,
                    started_explicitly: false,
                    started_from_response_item: false,
                    rollout_start_index: index,
                    rollout_end_index: index + 1,
                });
                if turn.user_message.is_none() {
                    turn.user_message = Some(event.message.clone());
                }
                turn.rollout_end_index = index + 1;
            }
            RolloutItem::EventMsg(EventMsg::TurnComplete(event)) => {
                if let Some(turn) = pending_turn.as_mut() {
                    if turn.turn_id == event.turn_id {
                        turn.completed_at = event.completed_at;
                    }
                    turn.rollout_end_index = index + 1;
                }
                finish_pending_turn(
                    &mut pending_turn,
                    &mut turns,
                    &mut turn_index_by_id,
                    &mut current_leaf_turn_id,
                );
            }
            RolloutItem::EventMsg(EventMsg::TurnAborted(_)) => {
                if let Some(turn) = pending_turn.as_mut() {
                    turn.rollout_end_index = index + 1;
                }
                finish_pending_turn(
                    &mut pending_turn,
                    &mut turns,
                    &mut turn_index_by_id,
                    &mut current_leaf_turn_id,
                );
            }
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(event)) => {
                finish_pending_turn(
                    &mut pending_turn,
                    &mut turns,
                    &mut turn_index_by_id,
                    &mut current_leaf_turn_id,
                );
                current_leaf_turn_id = rollback_leaf(
                    &turns,
                    &turn_index_by_id,
                    current_leaf_turn_id,
                    event.num_turns,
                );
            }
            RolloutItem::EventMsg(EventMsg::ThreadNavigated(event)) => {
                finish_pending_turn(
                    &mut pending_turn,
                    &mut turns,
                    &mut turn_index_by_id,
                    &mut current_leaf_turn_id,
                );
                current_leaf_turn_id = event.target_turn_id.clone();
            }
            RolloutItem::ResponseItem(response_item) => {
                if let Some(message) = user_message_from_response_item(response_item) {
                    if let Some(turn) = pending_turn.as_ref()
                        && turn.user_message.is_some()
                        && !turn.started_explicitly
                        && !(turn.started_from_response_item
                            && turn.user_message.as_deref() == Some(message.as_str()))
                    {
                        finish_pending_turn(
                            &mut pending_turn,
                            &mut turns,
                            &mut turn_index_by_id,
                            &mut current_leaf_turn_id,
                        );
                    }
                    let turn = pending_turn.get_or_insert_with(|| PendingTurn {
                        turn_id: fallback_turn_id(index),
                        parent_turn_id: current_leaf_turn_id.clone(),
                        user_message: None,
                        started_at: None,
                        completed_at: None,
                        started_explicitly: false,
                        started_from_response_item: true,
                        rollout_start_index: index,
                        rollout_end_index: index + 1,
                    });
                    if turn.user_message.is_none() {
                        turn.user_message = Some(message);
                    }
                }
                if let Some(turn) = pending_turn.as_mut() {
                    turn.rollout_end_index = index + 1;
                }
            }
            RolloutItem::Compacted(_) | RolloutItem::TurnContext(_) => {
                if let Some(turn) = pending_turn.as_mut() {
                    turn.rollout_end_index = index + 1;
                } else {
                    extend_current_leaf_rollout_end(
                        &mut turns,
                        &turn_index_by_id,
                        current_leaf_turn_id.as_deref(),
                        index + 1,
                    );
                }
            }
            RolloutItem::EventMsg(_) => {
                if let Some(turn) = pending_turn.as_mut() {
                    turn.rollout_end_index = index + 1;
                } else {
                    extend_current_leaf_rollout_end(
                        &mut turns,
                        &turn_index_by_id,
                        current_leaf_turn_id.as_deref(),
                        index + 1,
                    );
                }
            }
            RolloutItem::SessionMeta(_) => {
                extend_current_leaf_rollout_end(
                    &mut turns,
                    &turn_index_by_id,
                    current_leaf_turn_id.as_deref(),
                    index + 1,
                );
            }
        }
    }

    finish_pending_turn(
        &mut pending_turn,
        &mut turns,
        &mut turn_index_by_id,
        &mut current_leaf_turn_id,
    );

    let mut roots = Vec::new();
    for index in 0..turns.len() {
        if let Some(parent_turn_id) = turns[index].parent_turn_id.as_ref() {
            if let Some(parent_index) = turn_index_by_id.get(parent_turn_id).copied() {
                turns[parent_index].children.push(index);
            } else {
                roots.push(index);
            }
        } else {
            roots.push(index);
        }
    }

    assign_depths(&mut turns, &roots);
    mark_active_path(
        &mut turns,
        &turn_index_by_id,
        current_leaf_turn_id.as_deref(),
    );

    ThreadTree {
        turns,
        roots,
        active_leaf_turn_id: current_leaf_turn_id,
    }
}

pub fn active_branch_items(items: &[RolloutItem]) -> Vec<RolloutItem> {
    let tree = build_thread_tree(items);
    if tree.turns.is_empty() {
        return items
            .iter()
            .filter(|item| !is_branch_navigation_item(item))
            .cloned()
            .collect();
    }

    let active_ids: HashSet<&str> = tree
        .turns
        .iter()
        .filter(|turn| turn.is_active_path)
        .map(|turn| turn.turn_id.as_str())
        .collect();
    let mut active_turns: Vec<&ThreadTreeTurn> = tree
        .turns
        .iter()
        .filter(|turn| active_ids.contains(turn.turn_id.as_str()))
        .collect();
    active_turns.sort_by_key(|turn| turn.depth);

    let mut selected_items = Vec::new();
    selected_items.extend(
        items
            .iter()
            .take_while(|item| !turn_starts(item))
            .filter(|item| !is_branch_navigation_item(item))
            .cloned(),
    );
    for turn in active_turns {
        selected_items.extend(
            items[turn.rollout_start_index..turn.rollout_end_index]
                .iter()
                .filter(|item| !is_branch_navigation_item(item))
                .cloned(),
        );
    }
    selected_items
}

pub fn thread_tree_contains_turn_id(items: &[RolloutItem], turn_id: &str) -> bool {
    build_thread_tree(items)
        .turns
        .iter()
        .any(|turn| turn.turn_id == turn_id)
}

fn finish_pending_turn(
    pending_turn: &mut Option<PendingTurn>,
    turns: &mut Vec<ThreadTreeTurn>,
    turn_index_by_id: &mut HashMap<String, usize>,
    current_leaf_turn_id: &mut Option<String>,
) {
    let Some(turn) = pending_turn.take() else {
        return;
    };
    let turn_id = turn.turn_id;
    let index = turns.len();
    turn_index_by_id.insert(turn_id.clone(), index);
    *current_leaf_turn_id = Some(turn_id.clone());
    turns.push(ThreadTreeTurn {
        turn_id,
        parent_turn_id: turn.parent_turn_id,
        children: Vec::new(),
        depth: 0,
        user_message: turn.user_message,
        started_at: turn.started_at,
        completed_at: turn.completed_at,
        is_active_path: false,
        rollout_start_index: turn.rollout_start_index,
        rollout_end_index: turn.rollout_end_index,
    });
}

fn extend_current_leaf_rollout_end(
    turns: &mut [ThreadTreeTurn],
    turn_index_by_id: &HashMap<String, usize>,
    current_leaf_turn_id: Option<&str>,
    rollout_end_index: usize,
) {
    let Some(current_leaf_turn_id) = current_leaf_turn_id else {
        return;
    };
    let Some(index) = turn_index_by_id.get(current_leaf_turn_id).copied() else {
        return;
    };
    turns[index].rollout_end_index = turns[index].rollout_end_index.max(rollout_end_index);
}

fn rollback_leaf(
    turns: &[ThreadTreeTurn],
    turn_index_by_id: &HashMap<String, usize>,
    mut leaf_turn_id: Option<String>,
    num_turns: u32,
) -> Option<String> {
    let mut remaining = usize::try_from(num_turns).unwrap_or(usize::MAX);
    while remaining > 0 {
        let leaf = leaf_turn_id?;
        let index = turn_index_by_id.get(&leaf).copied()?;
        if turns[index].user_message.is_some() {
            remaining -= 1;
        }
        leaf_turn_id = turns[index].parent_turn_id.clone();
    }
    leaf_turn_id
}

fn assign_depths(turns: &mut [ThreadTreeTurn], roots: &[usize]) {
    let mut stack: Vec<(usize, usize)> = roots
        .iter()
        .rev()
        .copied()
        .map(|index| (index, 0))
        .collect();
    while let Some((index, depth)) = stack.pop() {
        turns[index].depth = depth;
        let children = turns[index].children.clone();
        for child_index in children.into_iter().rev() {
            stack.push((child_index, depth + 1));
        }
    }
}

fn mark_active_path(
    turns: &mut [ThreadTreeTurn],
    turn_index_by_id: &HashMap<String, usize>,
    leaf_turn_id: Option<&str>,
) {
    let mut leaf_turn_id = leaf_turn_id.map(ToString::to_string);
    while let Some(turn_id) = leaf_turn_id {
        let Some(index) = turn_index_by_id.get(turn_id.as_str()).copied() else {
            return;
        };
        turns[index].is_active_path = true;
        leaf_turn_id = turns[index].parent_turn_id.clone();
    }
}

fn user_message_from_response_item(item: &ResponseItem) -> Option<String> {
    let ResponseItem::Message { role, content, .. } = item else {
        return None;
    };
    if role != "user" {
        return None;
    }
    let text = content
        .iter()
        .filter_map(|item| match item {
            ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                Some(text.as_str())
            }
            ContentItem::InputImage { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    (!text.is_empty()).then_some(text)
}

fn fallback_turn_id(index: usize) -> String {
    format!("rollout-{index}")
}

fn turn_starts(item: &RolloutItem) -> bool {
    matches!(
        item,
        RolloutItem::EventMsg(EventMsg::TurnStarted(_))
            | RolloutItem::EventMsg(EventMsg::UserMessage(_))
            | RolloutItem::ResponseItem(_)
    )
}

fn is_branch_navigation_item(item: &RolloutItem) -> bool {
    matches!(
        item,
        RolloutItem::EventMsg(EventMsg::ThreadRolledBack(_))
            | RolloutItem::EventMsg(EventMsg::ThreadNavigated(_))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::protocol::CompactedItem;
    use codex_protocol::protocol::ThreadNavigatedEvent;
    use codex_protocol::protocol::ThreadRolledBackEvent;
    use codex_protocol::protocol::TurnAbortReason;
    use codex_protocol::protocol::TurnAbortedEvent;
    use codex_protocol::protocol::TurnCompleteEvent;
    use codex_protocol::protocol::TurnStartedEvent;
    use codex_protocol::protocol::UserMessageEvent;
    use pretty_assertions::assert_eq;

    fn turn(turn_id: &str, message: &str) -> Vec<RolloutItem> {
        vec![
            RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: turn_id.to_string(),
                started_at: None,
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            })),
            RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
                message: message.to_string(),
                images: None,
                image_details: Vec::new(),
                local_images: Vec::new(),
                local_image_details: Vec::new(),
                text_elements: Vec::new(),
            })),
            RolloutItem::EventMsg(EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id: turn_id.to_string(),
                last_agent_message: None,
                completed_at: None,
                duration_ms: None,
                time_to_first_token_ms: None,
            })),
        ]
    }

    fn empty_turn(turn_id: &str) -> Vec<RolloutItem> {
        vec![
            RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: turn_id.to_string(),
                started_at: None,
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            })),
            RolloutItem::EventMsg(EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id: turn_id.to_string(),
                last_agent_message: None,
                completed_at: None,
                duration_ms: None,
                time_to_first_token_ms: None,
            })),
        ]
    }

    fn response_user_message(text: &str) -> RolloutItem {
        RolloutItem::ResponseItem(ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: text.to_string(),
            }],
            phase: None,
        })
    }

    fn response_assistant_message(text: &str) -> RolloutItem {
        RolloutItem::ResponseItem(ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: text.to_string(),
            }],
            phase: None,
        })
    }

    fn compacted_item(message: &str) -> RolloutItem {
        RolloutItem::Compacted(CompactedItem {
            message: message.to_string(),
            replacement_history: None,
        })
    }

    fn active_branch_user_messages(items: &[RolloutItem]) -> Vec<String> {
        active_branch_items(items)
            .iter()
            .filter_map(|item| match item {
                RolloutItem::EventMsg(EventMsg::UserMessage(event)) => Some(event.message.clone()),
                RolloutItem::ResponseItem(ResponseItem::Message { role, content, .. })
                    if role == "user" =>
                {
                    Some(
                        content
                            .iter()
                            .filter_map(|item| match item {
                                ContentItem::InputText { text }
                                | ContentItem::OutputText { text } => Some(text.as_str()),
                                ContentItem::InputImage { .. } => None,
                            })
                            .collect::<Vec<_>>()
                            .join("\n"),
                    )
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn rollback_creates_branch_for_later_turn() {
        let mut items = Vec::new();
        items.extend(turn("turn-1", "first"));
        items.extend(turn("turn-2a", "second A"));
        items.push(RolloutItem::EventMsg(EventMsg::ThreadRolledBack(
            ThreadRolledBackEvent { num_turns: 1 },
        )));
        items.extend(turn("turn-2b", "second B"));

        let tree = build_thread_tree(&items);

        assert_eq!(tree.active_leaf_turn_id.as_deref(), Some("turn-2b"));
        assert_eq!(tree.roots.len(), 1);
        let root = &tree.turns[tree.roots[0]];
        assert_eq!(root.turn_id, "turn-1");
        assert_eq!(root.children.len(), 2);
        assert_eq!(
            active_branch_items(&items)
                .iter()
                .filter_map(|item| match item {
                    RolloutItem::EventMsg(EventMsg::UserMessage(event)) => {
                        Some(event.message.as_str())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>(),
            vec!["first", "second B"]
        );
    }

    #[test]
    fn navigation_selects_existing_inactive_branch() {
        let mut items = Vec::new();
        items.extend(turn("turn-1", "first"));
        items.extend(turn("turn-2a", "second A"));
        items.push(RolloutItem::EventMsg(EventMsg::ThreadRolledBack(
            ThreadRolledBackEvent { num_turns: 1 },
        )));
        items.extend(turn("turn-2b", "second B"));
        items.push(RolloutItem::EventMsg(EventMsg::ThreadNavigated(
            ThreadNavigatedEvent {
                previous_leaf_turn_id: Some("turn-2b".to_string()),
                target_turn_id: Some("turn-2a".to_string()),
            },
        )));

        let tree = build_thread_tree(&items);

        assert_eq!(tree.active_leaf_turn_id.as_deref(), Some("turn-2a"));
        assert_eq!(
            active_branch_items(&items)
                .iter()
                .filter_map(|item| match item {
                    RolloutItem::EventMsg(EventMsg::UserMessage(event)) => {
                        Some(event.message.as_str())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>(),
            vec!["first", "second A"]
        );
    }

    #[test]
    fn active_branch_keeps_turn_aborted_marker() {
        let items = vec![
            RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: "turn-1".to_string(),
                started_at: None,
                model_context_window: None,
                collaboration_mode_kind: Default::default(),
            })),
            RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
                message: "first".to_string(),
                images: None,
                image_details: Vec::new(),
                local_images: Vec::new(),
                local_image_details: Vec::new(),
                text_elements: Vec::new(),
            })),
            RolloutItem::EventMsg(EventMsg::TurnAborted(TurnAbortedEvent {
                turn_id: Some("turn-1".to_string()),
                reason: TurnAbortReason::Interrupted,
                completed_at: None,
                duration_ms: None,
            })),
        ];

        assert!(
            active_branch_items(&items)
                .iter()
                .any(|item| matches!(item, RolloutItem::EventMsg(EventMsg::TurnAborted(_))))
        );
    }

    #[test]
    fn legacy_user_messages_are_separate_implicit_turns() {
        let mut items = vec![
            RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
                message: "first".to_string(),
                images: None,
                image_details: Vec::new(),
                local_images: Vec::new(),
                local_image_details: Vec::new(),
                text_elements: Vec::new(),
            })),
            RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
                message: "second".to_string(),
                images: None,
                image_details: Vec::new(),
                local_images: Vec::new(),
                local_image_details: Vec::new(),
                text_elements: Vec::new(),
            })),
            RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
                message: "third".to_string(),
                images: None,
                image_details: Vec::new(),
                local_images: Vec::new(),
                local_image_details: Vec::new(),
                text_elements: Vec::new(),
            })),
        ];
        items.push(RolloutItem::EventMsg(EventMsg::ThreadRolledBack(
            ThreadRolledBackEvent { num_turns: 1 },
        )));

        assert_eq!(
            active_branch_items(&items)
                .iter()
                .filter_map(|item| match item {
                    RolloutItem::EventMsg(EventMsg::UserMessage(event)) => {
                        Some(event.message.as_str())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>(),
            vec!["first", "second"]
        );
    }

    #[test]
    fn response_item_user_messages_are_separate_implicit_turns() {
        let mut items = vec![
            response_user_message("first"),
            response_assistant_message("first response"),
            response_user_message("second"),
            response_assistant_message("second response"),
        ];
        items.push(RolloutItem::EventMsg(EventMsg::ThreadRolledBack(
            ThreadRolledBackEvent { num_turns: 1 },
        )));

        let tree = build_thread_tree(&items);

        assert_eq!(tree.turns.len(), 2);
        assert_eq!(tree.active_leaf_turn_id.as_deref(), Some("rollout-0"));
        assert_eq!(active_branch_user_messages(&items), vec!["first"]);
    }

    #[test]
    fn active_branch_keeps_inter_turn_compacted_item() {
        let mut items = Vec::new();
        items.extend(turn("turn-1", "first"));
        items.push(compacted_item("summary between turns"));
        items.extend(turn("turn-2", "second"));

        assert!(active_branch_items(&items).iter().any(|item| matches!(
            item,
            RolloutItem::Compacted(compacted)
                if compacted.message == "summary between turns"
        )));
    }

    #[test]
    fn rollback_from_no_user_leaf_counts_user_turns_only() {
        let mut items = Vec::new();
        items.extend(turn("turn-1", "first"));
        items.extend(empty_turn("turn-2"));
        items.push(RolloutItem::EventMsg(EventMsg::ThreadRolledBack(
            ThreadRolledBackEvent { num_turns: 1 },
        )));

        let tree = build_thread_tree(&items);

        assert_eq!(tree.active_leaf_turn_id, None);
        assert_eq!(active_branch_user_messages(&items), Vec::<String>::new());
    }

    #[test]
    fn legacy_user_turn_keeps_following_agent_message() {
        let items = vec![
            RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
                message: "before rollback".to_string(),
                images: None,
                image_details: Vec::new(),
                local_images: Vec::new(),
                local_image_details: Vec::new(),
                text_elements: Vec::new(),
            })),
            RolloutItem::EventMsg(EventMsg::AgentMessage(
                codex_protocol::protocol::AgentMessageEvent {
                    message: "after rollback".to_string(),
                    phase: None,
                    memory_citation: None,
                },
            )),
        ];

        let active_messages = active_branch_items(&items)
            .into_iter()
            .filter_map(|item| match item {
                RolloutItem::EventMsg(EventMsg::UserMessage(event)) => Some(event.message),
                RolloutItem::EventMsg(EventMsg::AgentMessage(event)) => Some(event.message),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            active_messages,
            vec!["before rollback".to_string(), "after rollback".to_string(),]
        );
    }
}
