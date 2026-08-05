use super::{app::App, markdown};
use crate::config::UiLanguage;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};
use unicode_width::UnicodeWidthChar;
pub fn draw(f: &mut Frame, app: &App) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(4),
        ])
        .split(f.area());
    let title = Paragraph::new(format!(
        "nl2sh v{} | {} | {} | {} | {} | {}",
        env!("CARGO_PKG_VERSION"),
        app.mode,
        app.root,
        app.model,
        app.api_type,
        if app.ascii { "ASCII" } else { "Unicode" }
    ))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, areas[0]);
    let conversation_width = areas[1].width.saturating_sub(2) as usize;
    let lines = wrap_rendered_lines(
        conversation_lines(app, conversation_width),
        conversation_width,
    );
    let visible_rows = areas[1].height.saturating_sub(2) as usize;
    let rendered_rows = lines.len();
    let bottom = rendered_rows.saturating_sub(visible_rows);
    let scroll = bottom
        .saturating_sub(app.conversation_scroll.min(bottom))
        .min(u16::MAX as usize) as u16;
    f.render_widget(
        Paragraph::new(Text::from(lines)).scroll((scroll, 0)).block(
            Block::default()
                .borders(Borders::ALL)
                .title(match app.language {
                    UiLanguage::ZhCn => "对话历史",
                    UiLanguage::En => "Conversation",
                }),
        ),
        areas[1],
    );
    let input_background = Color::Rgb(52, 52, 52);
    let input_row =
        ratatui::layout::Rect::new(areas[2].x, areas[2].y.saturating_add(1), areas[2].width, 1);
    f.render_widget(
        Block::default().style(Style::default().bg(input_background)),
        input_row,
    );
    let input = Paragraph::new(Text::from(vec![
        Line::styled(
            format!("> {}", app.input.text),
            Style::default().fg(Color::White).bg(input_background),
        ),
        Line::from(match app.language {
            UiLanguage::ZhCn => format!(
                "状态：{} | 对话轮次 {} | 剩余上下文 {}",
                app.status,
                app.turn,
                app.max_context.saturating_sub(app.turn)
            ),
            UiLanguage::En => format!(
                "status: {} | turn {} | context remaining {}",
                app.status,
                app.turn,
                app.max_context.saturating_sub(app.turn)
            ),
        }),
    ]))
    .block(
        Block::default()
            .borders(Borders::TOP | Borders::BOTTOM)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(match app.language {
                UiLanguage::ZhCn => "Enter 发送 | F2 结果 | 滚轮/PgUp/PgDn 历史 | Ctrl+Q 退出",
                UiLanguage::En => "Enter send | F2 results | Wheel/PgUp/PgDn history | Ctrl+Q quit",
            }),
    );
    f.render_widget(input, areas[2]);
    if let Some(popup) = &app.popup {
        let area = centered_rect(78, 45, f.area());
        f.render_widget(Clear, area);
        f.render_widget(
            Paragraph::new(popup.lines.join("\n"))
                .wrap(Wrap { trim: false })
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(popup.title.as_str()),
                ),
            area,
        );
    }
}

fn wrap_rendered_lines(lines: Vec<Line<'_>>, width: usize) -> Vec<Line<'static>> {
    if width == 0 {
        return Vec::new();
    }
    let mut output = Vec::new();
    for line in lines {
        let mut current_spans = Vec::new();
        let mut current_width = 0;
        for span in line.spans {
            let style = line.style.patch(span.style);
            let mut segment = String::new();
            for character in span.content.chars() {
                let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
                if current_width > 0 && current_width + character_width > width {
                    if !segment.is_empty() {
                        current_spans.push(ratatui::text::Span::styled(
                            std::mem::take(&mut segment),
                            style,
                        ));
                    }
                    output.push(Line::from(std::mem::take(&mut current_spans)));
                    current_width = 0;
                }
                segment.push(character);
                current_width += character_width;
            }
            if !segment.is_empty() {
                current_spans.push(ratatui::text::Span::styled(segment, style));
            }
        }
        output.push(Line::from(current_spans));
    }
    output
}

fn conversation_lines(app: &App, width: usize) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    for entry in &app.history {
        if let Some(encoded) = entry.strip_prefix(super::session::TOOL_RESULT_PREFIX) {
            let (prefix, details) = encoded.split_once('\n').unwrap_or((encoded, ""));
            if app.tool_results_expanded {
                let label = match app.language {
                    UiLanguage::ZhCn => "工具结果：",
                    UiLanguage::En => "Tool result:",
                };
                lines.push(conversation_line_owned(format!("{prefix} {label}")));
                lines.extend(details.lines().map(conversation_line));
            } else {
                let label = match app.language {
                    UiLanguage::ZhCn => "工具结果已折叠（F2 展开）",
                    UiLanguage::En => "Tool result collapsed (F2 to expand)",
                };
                lines.push(conversation_line_owned(format!("{prefix} {label}")));
            }
        } else if let Some(visible) = entry.strip_prefix(super::session::LIVE_OUTPUT_PREFIX) {
            lines.push(conversation_line(visible));
        } else if starts_with_any(entry, &["[AGENT]", "🤖"]) {
            lines.extend(markdown::render(entry, width, Color::Green, app.ascii));
        } else {
            append_multiline_entry(&mut lines, entry);
        }
    }
    lines
}

fn append_multiline_entry<'a>(lines: &mut Vec<Line<'a>>, entry: &'a str) {
    let continuation_color = conversation_color(entry);
    for (index, line) in entry.lines().enumerate() {
        if index == 0 || continuation_color == Color::Reset {
            lines.push(conversation_line(line));
        } else {
            lines.push(Line::styled(line, Style::default().fg(continuation_color)));
        }
    }
    if entry.is_empty() {
        lines.push(Line::default());
    }
}

fn conversation_line(value: &str) -> Line<'_> {
    Line::styled(value, Style::default().fg(conversation_color(value)))
}

fn conversation_line_owned(value: String) -> Line<'static> {
    let color = conversation_color(&value);
    Line::styled(value, Style::default().fg(color))
}

fn conversation_color(value: &str) -> Color {
    if value.starts_with("> ") {
        Color::Cyan
    } else if starts_with_any(value, &["[AGENT]", "🤖"]) {
        Color::Green
    } else if starts_with_any(value, &["[TOOL]", "🔧"]) {
        Color::Magenta
    } else if starts_with_any(value, &["[CMD]", "💻", "[WARN]", "⚠️"]) {
        Color::Yellow
    } else if starts_with_any(value, &["[OUT]", "[OK]", "✅"]) {
        Color::LightGreen
    } else if starts_with_any(value, &["[ERR]", "[ERROR]", "[TIMEOUT]", "❌", "⏱", "⛔"]) {
        Color::LightRed
    } else if value.starts_with("[CONFIG]") {
        Color::LightBlue
    } else {
        Color::Reset
    }
}

fn starts_with_any(value: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|prefix| value.starts_with(prefix))
}

fn centered_rect(
    percent_x: u16,
    percent_y: u16,
    area: ratatui::layout::Rect,
) -> ratatui::layout::Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::input::Input;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn input_and_status_use_separate_rows() -> anyhow::Result<()> {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend)?;
        let mut app = App {
            input: Input {
                text: "hello".into(),
            },
            history: Vec::new(),
            conversation_scroll: 0,
            tool_results_expanded: false,
            model: "test".into(),
            root: "Normal".into(),
            ascii: true,
            language: UiLanguage::En,
            api_type: "Responses".into(),
            mode: "Agent".into(),
            turn: 2,
            max_context: 10,
            status: "idle".into(),
            popup: None,
        };
        terminal.draw(|frame| draw(frame, &app))?;
        let buffer = terminal.backend().buffer();
        let row = |y| {
            (0..100)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        };
        assert!(row(27).contains("> hello"));
        assert!(row(28).contains("status: idle | turn 2 | context remaining 8"));
        assert_eq!(buffer[(10, 27)].bg, Color::Rgb(52, 52, 52));
        assert_eq!(buffer[(10, 28)].bg, Color::Reset);
        assert_eq!(buffer[(10, 26)].bg, Color::Reset);
        assert_eq!(buffer[(10, 29)].bg, Color::Reset);
        assert!(row(26).contains('─'));
        assert!(row(29).contains('─'));

        app.history = vec![crate::tui::session::encode_tool_result(
            "[OK]",
            "executed_command=id\nexit=Some(0)\nstdout:\nuid=0",
        )];
        let collapsed = conversation_lines(&app, 98);
        assert_eq!(collapsed.len(), 1);
        assert!(collapsed[0].to_string().contains("collapsed"));
        assert!(!collapsed[0].to_string().contains("executed_command"));
        app.tool_results_expanded = true;
        let expanded = conversation_lines(&app, 98);
        assert!(expanded.len() > 1);
        assert!(expanded
            .iter()
            .any(|line| line.to_string().contains("executed_command=id")));

        app.history = vec![
            "[AGENT] 查询结果汇总：\n\n## 内存占用\n| 应用 | RSS |\n|---|---|\n| example | 288 MB |"
                .into(),
        ];
        app.tool_results_expanded = false;
        let markdown = conversation_lines(&app, 98);
        assert!(markdown.len() > 6);
        assert_eq!(markdown[2].to_string(), "内存占用");
        assert!(markdown
            .iter()
            .any(|line| line.to_string().starts_with('+')));
        assert!(markdown
            .iter()
            .any(|line| line.to_string().contains("example")));
        Ok(())
    }

    #[test]
    fn conversation_types_have_distinct_colors() {
        assert_eq!(conversation_line("> user").style.fg, Some(Color::Cyan));
        assert_eq!(
            conversation_line("[TOOL] call").style.fg,
            Some(Color::Magenta)
        );
        assert_eq!(
            conversation_line("[AGENT] answer").style.fg,
            Some(Color::Green)
        );
        assert_ne!(
            conversation_line("> user").style.fg,
            conversation_line("[TOOL] call").style.fg
        );
        assert_ne!(
            conversation_line("[TOOL] call").style.fg,
            conversation_line("[AGENT] answer").style.fg
        );
    }

    #[test]
    fn expanded_wrapped_tool_result_stays_bottom_aligned() -> anyhow::Result<()> {
        let backend = TestBackend::new(40, 12);
        let mut terminal = Terminal::new(backend)?;
        let mut app = App {
            input: Input::default(),
            history: vec![crate::tui::session::encode_tool_result(
                "[OK]",
                &format!("executed_command={}\nLAST_MARKER", "x".repeat(240)),
            )],
            conversation_scroll: 0,
            tool_results_expanded: true,
            model: "test".into(),
            root: "Root".into(),
            ascii: true,
            language: UiLanguage::En,
            api_type: "Responses".into(),
            mode: "Agent".into(),
            turn: 1,
            max_context: 10,
            status: "idle".into(),
            popup: None,
        };
        terminal.draw(|frame| draw(frame, &app))?;
        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("LAST_MARKER"));

        app.conversation_scroll = u16::MAX as usize;
        terminal.draw(|frame| draw(frame, &app))?;
        let rendered_top = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered_top.contains("Tool result"));
        Ok(())
    }
}
