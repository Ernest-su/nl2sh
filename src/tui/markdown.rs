use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub(super) fn render(source: &str, width: usize, base: Color, ascii: bool) -> Vec<Line<'static>> {
    let source_lines: Vec<&str> = source.lines().collect();
    let mut rendered = Vec::new();
    let mut index = 0;
    let mut in_code = false;
    while index < source_lines.len() {
        let line = source_lines[index];
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
            index += 1;
            continue;
        }
        if in_code {
            rendered.push(Line::styled(
                line.to_owned(),
                Style::default()
                    .fg(Color::Yellow)
                    .bg(Color::Rgb(38, 38, 38)),
            ));
            index += 1;
            continue;
        }
        if index + 1 < source_lines.len()
            && parse_table_row(line).is_some()
            && is_table_separator(source_lines[index + 1])
        {
            let start = index;
            index += 2;
            while index < source_lines.len() && parse_table_row(source_lines[index]).is_some() {
                index += 1;
            }
            rendered.extend(render_table(
                &source_lines[start..index],
                width,
                base,
                ascii,
            ));
            continue;
        }
        rendered.push(render_line(line, base, ascii));
        index += 1;
    }
    if source.is_empty() {
        rendered.push(Line::default());
    }
    rendered
}

fn render_line(line: &str, base: Color, ascii: bool) -> Line<'static> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() {
        return Line::default();
    }
    if let Some(content) = heading_content(trimmed) {
        return Line::from(inline_spans(content, Color::LightGreen))
            .style(Style::default().add_modifier(Modifier::BOLD));
    }
    if let Some(content) = trimmed.strip_prefix("> ") {
        let marker = if ascii { "| " } else { "│ " };
        let mut spans = vec![Span::styled(
            marker.to_owned(),
            Style::default().fg(Color::DarkGray),
        )];
        spans.extend(inline_spans(content, base));
        return Line::from(spans);
    }
    if let Some(content) = list_content(trimmed) {
        let marker = if ascii { "- " } else { "• " };
        let mut spans = vec![Span::styled(marker.to_owned(), Style::default().fg(base))];
        spans.extend(inline_spans(content, base));
        return Line::from(spans);
    }
    if trimmed == "---" || trimmed == "***" {
        return Line::styled(
            if ascii { "-" } else { "─" }.repeat(40),
            Style::default().fg(Color::DarkGray),
        );
    }
    Line::from(inline_spans(line, base))
}

fn heading_content(line: &str) -> Option<&str> {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&hashes) && line.as_bytes().get(hashes) == Some(&b' ') {
        Some(line[hashes + 1..].trim())
    } else {
        None
    }
}

fn list_content(line: &str) -> Option<&str> {
    line.strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .or_else(|| line.strip_prefix("+ "))
}

fn inline_spans(mut text: &str, base: Color) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    while !text.is_empty() {
        let Some((index, marker)) = next_marker(text) else {
            spans.push(styled_text(text, base, Modifier::empty(), None));
            break;
        };
        if index > 0 {
            spans.push(styled_text(&text[..index], base, Modifier::empty(), None));
            text = &text[index..];
        }
        match marker {
            "**" => {
                if let Some(end) = text[2..].find("**") {
                    spans.push(styled_text(&text[2..end + 2], base, Modifier::BOLD, None));
                    text = &text[end + 4..];
                } else {
                    push_first_char(&mut spans, &mut text, base);
                }
            }
            "`" => {
                if let Some(end) = text[1..].find('`') {
                    spans.push(styled_text(
                        &text[1..end + 1],
                        Color::White,
                        Modifier::empty(),
                        Some(Color::Rgb(52, 52, 52)),
                    ));
                    text = &text[end + 2..];
                } else {
                    push_first_char(&mut spans, &mut text, base);
                }
            }
            "[" => {
                if let Some((consumed, label, url)) = parse_link(text) {
                    spans.push(styled_text(label, Color::Cyan, Modifier::UNDERLINED, None));
                    spans.push(styled_text(
                        &format!(" ({url})"),
                        Color::DarkGray,
                        Modifier::empty(),
                        None,
                    ));
                    text = &text[consumed..];
                } else {
                    push_first_char(&mut spans, &mut text, base);
                }
            }
            "*" => {
                if let Some(end) = text[1..].find('*') {
                    spans.push(styled_text(&text[1..end + 1], base, Modifier::ITALIC, None));
                    text = &text[end + 2..];
                } else {
                    push_first_char(&mut spans, &mut text, base);
                }
            }
            _ => push_first_char(&mut spans, &mut text, base),
        }
    }
    spans
}

fn next_marker(text: &str) -> Option<(usize, &'static str)> {
    ["**", "`", "[", "*"]
        .into_iter()
        .filter_map(|marker| text.find(marker).map(|index| (index, marker)))
        .min_by_key(|(index, marker)| (*index, std::cmp::Reverse(marker.len())))
}

fn parse_link(text: &str) -> Option<(usize, &str, &str)> {
    let label_end = text.find("](")?;
    let url_end = text[label_end + 2..].find(')')? + label_end + 2;
    Some((
        url_end + 1,
        &text[1..label_end],
        &text[label_end + 2..url_end],
    ))
}

fn push_first_char(spans: &mut Vec<Span<'static>>, text: &mut &str, base: Color) {
    if let Some(character) = text.chars().next() {
        spans.push(styled_text(
            &character.to_string(),
            base,
            Modifier::empty(),
            None,
        ));
        *text = &text[character.len_utf8()..];
    }
}

fn styled_text(
    text: &str,
    foreground: Color,
    modifier: Modifier,
    background: Option<Color>,
) -> Span<'static> {
    let mut style = Style::default().fg(foreground).add_modifier(modifier);
    if let Some(background) = background {
        style = style.bg(background);
    }
    Span::styled(text.to_owned(), style)
}

fn render_table(lines: &[&str], width: usize, base: Color, ascii: bool) -> Vec<Line<'static>> {
    let Some(headers) = parse_table_row(lines[0]) else {
        return lines
            .iter()
            .map(|line| Line::styled((*line).to_owned(), Style::default().fg(base)))
            .collect();
    };
    let rows: Vec<Vec<String>> = lines[2..]
        .iter()
        .filter_map(|line| parse_table_row(line))
        .collect();
    let columns = headers.len();
    if columns == 0 || rows.iter().any(|row| row.len() != columns) {
        return lines
            .iter()
            .map(|line| Line::styled((*line).to_owned(), Style::default().fg(base)))
            .collect();
    }
    let mut widths = vec![1; columns];
    for row in std::iter::once(&headers).chain(rows.iter()) {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(UnicodeWidthStr::width(cell.as_str()));
        }
    }
    let minimum_total = columns.saturating_mul(7).saturating_add(1);
    if width < minimum_total {
        return render_table_as_list(&headers, &rows, base);
    }
    while table_width(&widths) > width {
        let Some((index, _)) = widths.iter().enumerate().max_by_key(|(_, value)| **value) else {
            break;
        };
        if widths[index] <= 4 {
            return render_table_as_list(&headers, &rows, base);
        }
        widths[index] -= 1;
    }
    render_bordered_table(&headers, &rows, &widths, base, ascii)
}

fn parse_table_row(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim();
    if !trimmed.contains('|') {
        return None;
    }
    let body = trimmed.strip_prefix('|').unwrap_or(trimmed);
    let body = body.strip_suffix('|').unwrap_or(body);
    let cells: Vec<String> = body
        .split('|')
        .map(|cell| plain_inline(cell.trim()))
        .collect();
    (!cells.is_empty()).then_some(cells)
}

fn is_table_separator(line: &str) -> bool {
    parse_table_row(line).is_some_and(|cells| {
        cells.iter().all(|cell| {
            let cell = cell.trim_matches(':').trim();
            cell.len() >= 3 && cell.chars().all(|c| c == '-')
        })
    })
}

fn plain_inline(text: &str) -> String {
    text.replace(['`', '*'], "")
}

fn table_width(widths: &[usize]) -> usize {
    widths.iter().sum::<usize>() + widths.len().saturating_mul(3) + 1
}

fn render_bordered_table(
    headers: &[String],
    rows: &[Vec<String>],
    widths: &[usize],
    base: Color,
    ascii: bool,
) -> Vec<Line<'static>> {
    let chars = if ascii {
        ('+', '+', '+', '+', '+', '+', '-', '|')
    } else {
        ('┌', '┬', '┐', '├', '┼', '┤', '─', '│')
    };
    let mut output = vec![styled_table_line(
        border(widths, chars.0, chars.1, chars.2, chars.6),
        Color::DarkGray,
        false,
    )];
    output.extend(render_table_row(headers, widths, chars.7, base, true));
    output.push(styled_table_line(
        border(widths, chars.3, chars.4, chars.5, chars.6),
        Color::DarkGray,
        false,
    ));
    for row in rows {
        output.extend(render_table_row(row, widths, chars.7, base, false));
    }
    let bottom = if ascii {
        border(widths, '+', '+', '+', '-')
    } else {
        border(widths, '└', '┴', '┘', '─')
    };
    output.push(styled_table_line(bottom, Color::DarkGray, false));
    output
}

fn render_table_row(
    row: &[String],
    widths: &[usize],
    vertical: char,
    color: Color,
    bold: bool,
) -> Vec<Line<'static>> {
    let wrapped: Vec<Vec<String>> = row
        .iter()
        .zip(widths)
        .map(|(cell, width)| wrap_display(cell, *width))
        .collect();
    let height = wrapped.iter().map(Vec::len).max().unwrap_or(1);
    (0..height)
        .map(|line_index| {
            let mut line = vertical.to_string();
            for (index, width) in widths.iter().enumerate() {
                let cell = wrapped[index]
                    .get(line_index)
                    .map(String::as_str)
                    .unwrap_or("");
                line.push(' ');
                line.push_str(cell);
                line.push_str(&" ".repeat(width.saturating_sub(UnicodeWidthStr::width(cell)) + 1));
                line.push(vertical);
            }
            styled_table_line(line, color, bold)
        })
        .collect()
}

fn render_table_as_list(
    headers: &[String],
    rows: &[Vec<String>],
    base: Color,
) -> Vec<Line<'static>> {
    if rows.is_empty() {
        return vec![Line::styled(
            headers.join(" | "),
            Style::default().fg(base).add_modifier(Modifier::BOLD),
        )];
    }
    let mut output = Vec::new();
    for (row_index, row) in rows.iter().enumerate() {
        if row_index > 0 {
            output.push(Line::default());
        }
        for (header, value) in headers.iter().zip(row) {
            output.push(Line::from(vec![
                styled_text(&format!("{header}: "), base, Modifier::BOLD, None),
                styled_text(value, base, Modifier::empty(), None),
            ]));
        }
    }
    output
}

fn styled_table_line(text: String, color: Color, bold: bool) -> Line<'static> {
    let modifier = if bold {
        Modifier::BOLD
    } else {
        Modifier::empty()
    };
    Line::styled(text, Style::default().fg(color).add_modifier(modifier))
}

fn border(widths: &[usize], left: char, middle: char, right: char, fill: char) -> String {
    let mut line = left.to_string();
    for (index, width) in widths.iter().enumerate() {
        line.push_str(&fill.to_string().repeat(width + 2));
        line.push(if index + 1 == widths.len() {
            right
        } else {
            middle
        });
    }
    line
}

fn wrap_display(text: &str, width: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let mut lines = vec![String::new()];
    let mut current_width = 0;
    for character in text.chars() {
        let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
        if current_width > 0 && current_width + character_width > width {
            lines.push(String::new());
            current_width = 0;
        }
        if let Some(line) = lines.last_mut() {
            line.push(character);
        }
        current_width += character_width;
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_headings_inline_styles_lists_quotes_and_code() {
        let source = "## 标题\n- **粗体** 和 `code`\n> 引用\n```sh\necho hello\n```";
        let lines = render(source, 80, Color::Green, false);
        assert_eq!(lines[0].to_string(), "标题");
        assert_eq!(lines[1].to_string(), "• 粗体 和 code");
        assert_eq!(lines[2].to_string(), "│ 引用");
        assert_eq!(lines[3].to_string(), "echo hello");
        assert!(lines[0].style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn aligns_chinese_table_and_degrades_on_narrow_windows() {
        let source = "| 应用 | RSS |\n|---|---|\n| 前台应用 | 288 MB |\n| system | 271 MB |";
        let table = render(source, 60, Color::Green, false);
        assert!(table[0].to_string().starts_with('┌'));
        let row_widths: Vec<usize> = table
            .iter()
            .map(|line| UnicodeWidthStr::width(line.to_string().as_str()))
            .collect();
        assert!(row_widths.windows(2).all(|pair| pair[0] == pair[1]));

        let narrow = render(source, 10, Color::Green, false);
        assert!(narrow.iter().any(|line| line.to_string().contains("应用:")));
        assert!(!narrow.iter().any(|line| line.to_string().contains('┌')));
    }

    #[test]
    fn malformed_markdown_falls_back_to_visible_text() {
        let lines = render("未关闭的 **粗体 和 [链接", 80, Color::Green, false);
        assert_eq!(lines[0].to_string(), "未关闭的 **粗体 和 [链接");
    }
}
