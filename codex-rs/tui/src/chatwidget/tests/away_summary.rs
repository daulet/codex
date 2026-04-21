use super::*;

fn configure_away_summary_chat(chat: &mut ChatWidget) {
    chat.thread_id = Some(ThreadId::new());
    chat.set_feature_enabled(Feature::AwaySummary, /*enabled*/ true);
    chat.note_user_turn_for_away_summary();
}

#[tokio::test]
async fn away_summary_starts_after_unfocused_delay() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    configure_away_summary_chat(&mut chat);

    chat.handle_focus_lost();
    chat.away_summary.due_at = Some(Instant::now() - Duration::from_secs(1));
    chat.pre_draw_tick();

    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::StartAwaySummary { request_id: 1 })
    );
    assert_eq!(chat.away_summary.request_in_flight, Some(1));
}

#[tokio::test]
async fn away_summary_waits_for_running_turn_to_complete() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    configure_away_summary_chat(&mut chat);

    chat.agent_turn_running = true;
    chat.update_task_running_state();
    chat.handle_focus_lost();
    chat.away_summary.due_at = Some(Instant::now() - Duration::from_secs(1));
    chat.pre_draw_tick();

    assert!(rx.try_recv().is_err());
    assert!(chat.away_summary.pending_after_turn);

    chat.agent_turn_running = false;
    chat.update_task_running_state();
    chat.maybe_start_pending_away_summary();

    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::StartAwaySummary { request_id: 1 })
    );
}

#[tokio::test]
async fn focus_gain_does_not_clear_scheduled_away_summary() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    configure_away_summary_chat(&mut chat);

    chat.handle_focus_lost();
    chat.handle_focus_gained();

    assert!(chat.away_summary.focused);
    assert!(
        chat.away_summary.due_at.is_some(),
        "focus gain pulses should not erase the away timer"
    );

    chat.away_summary.due_at = Some(Instant::now() - Duration::from_secs(1));
    chat.pre_draw_tick();

    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::StartAwaySummary { request_id: 1 })
    );
    assert_eq!(chat.away_summary.request_in_flight, Some(1));
}

#[tokio::test]
async fn focus_gain_does_not_suppress_in_flight_away_summary_completion() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    configure_away_summary_chat(&mut chat);
    while rx.try_recv().is_ok() {}

    chat.away_summary.focused = false;
    chat.away_summary.request_in_flight = Some(9);
    chat.handle_focus_gained();

    assert!(rx.try_recv().is_err());
    assert_eq!(chat.away_summary.request_in_flight, Some(9));
    assert!(chat.away_summary.focused);

    chat.show_away_summary_completed(
        9,
        "You are debugging why away summaries disappear. Next, preserve the pending recap across focus pulses.".to_string(),
    );

    let rendered = drain_insert_history(&mut rx)
        .into_iter()
        .map(|lines| lines_to_single_string(&lines))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("You are debugging why away summaries disappear."),
        "expected completed away summary to render, got: {rendered}"
    );
}

#[tokio::test]
async fn away_summary_completion_inserts_dim_history_cell_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    configure_away_summary_chat(&mut chat);
    while rx.try_recv().is_ok() {}

    chat.away_summary.focused = false;
    chat.away_summary.request_in_flight = Some(3);
    chat.show_away_summary_completed(
        3,
        "You are implementing away summaries in the TUI. Next, wire the focus timer into the background request.".to_string(),
    );

    let rendered = drain_insert_history(&mut rx)
        .into_iter()
        .map(|lines| lines_to_single_string(&lines))
        .collect::<Vec<_>>()
        .join("\n");
    assert_chatwidget_snapshot!("away_summary_history_cell", rendered);
    assert!(chat.away_summary.summary_since_last_user_turn);
}
