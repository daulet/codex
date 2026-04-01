use std::cell::Cell;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

use crate::render::renderable::Renderable;
use crate::wrapping::RtOptions;
use crate::wrapping::adaptive_wrap_lines;

const BTW_PANEL_MAX_ROWS: u16 = 9;
const BTW_PANEL_MIN_WIDTH: u16 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BtwPanelStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BtwPanelContent {
    pub(crate) question: String,
    pub(crate) detail: String,
    pub(crate) status: BtwPanelStatus,
}

impl BtwPanelContent {
    pub(crate) fn running(question: String) -> Self {
        Self {
            question,
            detail: "Running /btw in parallel...".to_string(),
            status: BtwPanelStatus::Running,
        }
    }

    pub(crate) fn completed(question: String, detail: String) -> Self {
        Self {
            question,
            detail,
            status: BtwPanelStatus::Completed,
        }
    }

    pub(crate) fn failed(question: String, detail: String) -> Self {
        Self {
            question,
            detail,
            status: BtwPanelStatus::Failed,
        }
    }
}

pub(crate) struct BtwPanel {
    content: Option<BtwPanelContent>,
    scroll_offset: Cell<usize>,
    last_max_scroll: Cell<usize>,
}

impl BtwPanel {
    pub(crate) fn new() -> Self {
        Self {
            content: None,
            scroll_offset: Cell::new(0),
            last_max_scroll: Cell::new(0),
        }
    }

    pub(crate) fn is_visible(&self) -> bool {
        self.content.is_some()
    }

    pub(crate) fn set_content(&mut self, content: BtwPanelContent) {
        self.content = Some(content);
        self.scroll_offset.set(0);
        self.last_max_scroll.set(0);
    }

    pub(crate) fn clear(&mut self) {
        self.content = None;
        self.scroll_offset.set(0);
        self.last_max_scroll.set(0);
    }

    pub(crate) fn handle_key_event(&mut self, key_event: &KeyEvent) -> bool {
        if self.content.is_none() {
            return false;
        }

        match key_event.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ') => {
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                    self.clear();
                }
                true
            }
            KeyCode::Up => {
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                    self.scroll_offset
                        .set(self.scroll_offset.get().saturating_sub(1));
                }
                true
            }
            KeyCode::Down => {
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                    self.scroll_offset.set(
                        self.scroll_offset
                            .get()
                            .saturating_add(1)
                            .min(self.last_max_scroll.get()),
                    );
                }
                true
            }
            _ => false,
        }
    }

    fn body_lines(&self, width: u16) -> Vec<Line<'static>> {
        let Some(content) = &self.content else {
            return Vec::new();
        };

        let content_width = width as usize;
        let mut lines = adaptive_wrap_lines(
            std::iter::once(Line::from(vec![
                "/btw".magenta(),
                format!(" {}", content.question).into(),
            ])),
            RtOptions::new(content_width)
                .initial_indent(Line::from("  "))
                .subsequent_indent(Line::from("  ")),
        );
        lines.push(Line::from(""));

        for detail_line in content.detail.lines() {
            if detail_line.is_empty() {
                lines.push(Line::from(""));
                continue;
            }
            let styled: Line<'static> = match content.status {
                BtwPanelStatus::Running => Line::from(detail_line.to_string().dim()),
                BtwPanelStatus::Completed => Line::from(detail_line.to_string()),
                BtwPanelStatus::Failed => Line::from(detail_line.to_string().red()),
            };
            lines.extend(adaptive_wrap_lines(
                std::iter::once(styled),
                RtOptions::new(content_width)
                    .initial_indent(Line::from("    "))
                    .subsequent_indent(Line::from("    ")),
            ));
        }

        lines
    }

    fn hint_line(&self) -> Line<'static> {
        Line::from(vec![
            "  ".into(),
            "↑/↓ to scroll · Space, Enter, or Escape to dismiss".dim(),
        ])
    }
}

impl Renderable for BtwPanel {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() || area.width < BTW_PANEL_MIN_WIDTH || self.content.is_none() {
            return;
        }

        let footer_rows = 2;
        let body_height = area.height.saturating_sub(footer_rows);
        if body_height == 0 {
            return;
        }

        let lines = self.body_lines(area.width);
        let max_scroll = lines.len().saturating_sub(body_height as usize);
        self.last_max_scroll.set(max_scroll);
        let scroll = self.scroll_offset.get().min(max_scroll);
        self.scroll_offset.set(scroll);

        let visible_lines = lines
            .into_iter()
            .skip(scroll)
            .take(body_height as usize)
            .collect::<Vec<_>>();

        let body_area = Rect::new(area.x, area.y, area.width, body_height);
        Paragraph::new(visible_lines).render(body_area, buf);

        let hint_area = Rect::new(
            area.x,
            area.y.saturating_add(body_height).saturating_add(1),
            area.width,
            1,
        );
        Paragraph::new(vec![self.hint_line()]).render(hint_area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        if self.content.is_none() || width < BTW_PANEL_MIN_WIDTH {
            return 0;
        }

        let body_rows = self.body_lines(width).len() as u16;
        (body_rows.saturating_add(2)).min(BTW_PANEL_MAX_ROWS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn render_lines(panel: &BtwPanel, width: u16, height: u16) -> Vec<String> {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        panel.render(area, &mut buf);
        (0..height)
            .map(|row| {
                (0..width)
                    .map(|col| {
                        let symbol = buf[(col, row)].symbol();
                        if symbol.is_empty() {
                            ' '
                        } else {
                            symbol.chars().next().unwrap_or(' ')
                        }
                    })
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn renders_question_detail_and_hint() {
        let mut panel = BtwPanel::new();
        panel.set_content(BtwPanelContent::completed(
            "what path we in".to_string(),
            "The current working directory is:\n\n/tmp/demo".to_string(),
        ));

        let height = panel.desired_height(/*width*/ 60);
        let rendered = render_lines(&panel, /*width*/ 60, height).join("\n");
        assert!(rendered.contains("/btw what path we in"));
        assert!(rendered.contains("The current working directory is:"));
        assert!(rendered.contains("/tmp/demo"));
        assert!(rendered.contains("Space, Enter, or Escape to dismiss"));
    }

    #[test]
    fn down_scrolls_and_enter_dismisses_panel() {
        let mut panel = BtwPanel::new();
        panel.set_content(BtwPanelContent::completed(
            "long response".to_string(),
            (0..20)
                .map(|idx| format!("line {idx}"))
                .collect::<Vec<_>>()
                .join("\n"),
        ));

        let _ = render_lines(
            &panel,
            /*width*/ 40,
            panel.desired_height(/*width*/ 40),
        );
        let down = KeyEvent::new(KeyCode::Down, crossterm::event::KeyModifiers::NONE);
        assert!(panel.handle_key_event(&down));
        assert_eq!(panel.scroll_offset.get(), 1);

        let enter = KeyEvent::new(KeyCode::Enter, crossterm::event::KeyModifiers::NONE);
        assert!(panel.handle_key_event(&enter));
        assert!(!panel.is_visible());
    }
}
