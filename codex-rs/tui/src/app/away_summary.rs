use super::App;
use crate::app_server_session::AppServerSession;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::TurnStatus;
use codex_protocol::ThreadId;
use codex_protocol::user_input::UserInput;

#[derive(Debug, Clone)]
pub(super) struct AwaySummaryRequestState {
    pub(super) request_id: u64,
    pub(super) turn_id: Option<String>,
    pub(super) response: Option<String>,
    pub(super) error: Option<String>,
    pub(super) cancelled: bool,
}

const AWAY_SUMMARY_PROMPT: &str = "The user stepped away and is coming back. Write exactly 1-3 short sentences. Start by stating the high-level task: what they are building or debugging, not implementation details. Next: the concrete next step. Skip status reports and commit recaps. Do not run tools.";

impl App {
    pub(super) async fn start_away_summary_request(
        &mut self,
        app_server: &mut AppServerSession,
        request_id: u64,
    ) {
        let Some(origin_thread_id) = self.active_thread_id.or(self.chat_widget.thread_id()) else {
            self.chat_widget.finish_away_summary_request(request_id);
            return;
        };

        let mut fork_config = self.config.clone();
        fork_config.ephemeral = true;

        let started = match app_server.fork_thread(fork_config, origin_thread_id).await {
            Ok(started) => started,
            Err(err) => {
                tracing::warn!(error = %err, "failed to fork thread for away summary");
                self.chat_widget.finish_away_summary_request(request_id);
                return;
            }
        };

        let fork_thread_id = started.session.thread_id;
        self.away_summary_requests.insert(
            fork_thread_id,
            AwaySummaryRequestState {
                request_id,
                turn_id: None,
                response: None,
                error: None,
                cancelled: false,
            },
        );

        let turn_result = app_server
            .turn_start(
                fork_thread_id,
                vec![UserInput::Text {
                    text: AWAY_SUMMARY_PROMPT.to_string(),
                    text_elements: Vec::new(),
                }],
                started.session.cwd,
                started.session.approval_policy,
                started.session.approvals_reviewer,
                started.session.sandbox_policy,
                started.session.model,
                started.session.reasoning_effort,
                /*summary*/ None,
                started.session.service_tier.map(Some),
                /*collaboration_mode*/ None,
                /*personality*/ None,
                /*output_schema*/ None,
            )
            .await;

        match turn_result {
            Ok(response) => {
                if let Some(state) = self.away_summary_requests.get_mut(&fork_thread_id) {
                    state.turn_id = Some(response.turn.id);
                }
            }
            Err(err) => {
                tracing::warn!(error = %err, "failed to submit away summary prompt");
                if let Some(state) = self.away_summary_requests.remove(&fork_thread_id) {
                    self.chat_widget
                        .finish_away_summary_request(state.request_id);
                }
            }
        }
    }

    pub(super) async fn cancel_away_summary_requests(&mut self, app_server: &mut AppServerSession) {
        let mut interrupts = Vec::new();
        for (thread_id, state) in &mut self.away_summary_requests {
            state.cancelled = true;
            state.response = None;
            state.error = None;
            if let Some(turn_id) = state.turn_id.clone() {
                interrupts.push((*thread_id, turn_id));
            }
        }

        for (thread_id, turn_id) in interrupts {
            if let Err(err) = app_server.turn_interrupt(thread_id, turn_id).await {
                tracing::warn!(error = %err, %thread_id, "failed to interrupt cancelled away summary");
            }
        }
    }

    pub(super) fn handle_away_summary_thread_notification(
        &mut self,
        thread_id: ThreadId,
        notification: &ServerNotification,
    ) -> bool {
        let Some(state) = self.away_summary_requests.get_mut(&thread_id) else {
            return false;
        };

        match notification {
            ServerNotification::TurnStarted(notification) => {
                state.turn_id = Some(notification.turn.id.clone());
            }
            ServerNotification::ItemCompleted(notification) => {
                if !state.cancelled
                    && let ThreadItem::AgentMessage { text, phase, .. } = &notification.item
                    && matches!(
                        phase,
                        Some(codex_protocol::models::MessagePhase::FinalAnswer) | None
                    )
                {
                    state.response = Some(text.clone());
                }
            }
            ServerNotification::Error(notification) => {
                if !state.cancelled {
                    state.error = Some(notification.error.message.clone());
                }
            }
            ServerNotification::TurnCompleted(notification) => {
                if !state.cancelled
                    && matches!(notification.turn.status, TurnStatus::Failed)
                    && let Some(error) = &notification.turn.error
                {
                    state.error = Some(error.message.clone());
                }
                if let Some(state) = self.away_summary_requests.remove(&thread_id) {
                    self.finish_away_summary_request(state);
                }
            }
            ServerNotification::ThreadClosed(_) => {
                if let Some(state) = self.away_summary_requests.remove(&thread_id) {
                    self.finish_away_summary_request(state);
                }
            }
            _ => {}
        }

        true
    }

    fn finish_away_summary_request(&mut self, state: AwaySummaryRequestState) {
        if state.cancelled {
            self.chat_widget
                .finish_away_summary_request(state.request_id);
            return;
        }

        if let Some(error) = state.error {
            tracing::warn!(error, "away summary generation failed");
            self.chat_widget
                .finish_away_summary_request(state.request_id);
            return;
        }

        let Some(response) = state
            .response
            .filter(|response| !response.trim().is_empty())
        else {
            self.chat_widget
                .finish_away_summary_request(state.request_id);
            return;
        };
        self.chat_widget
            .show_away_summary_completed(state.request_id, response);
    }
}
