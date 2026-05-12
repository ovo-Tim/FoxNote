fn typst_raw_literal(input: &str) -> String {
    let mut fence_len = 1usize;
    while input.contains(&"`".repeat(fence_len)) {
        fence_len += 1;
    }

    let fence = "`".repeat(fence_len);
    format!("{fence}{input}{fence}")
}

fn inline_latex_to_typst(latex: &str) -> String {
    let trimmed = latex.trim();
    if trimmed.is_empty() {
        return "$ $".to_string();
    }

    format!("#mi({})", typst_raw_literal(trimmed))
}

fn block_latex_to_typst(latex: &str) -> String {
    let trimmed = latex.trim();
    if trimmed.is_empty() {
        return "".to_string();
    }

    format!("#mitex({})", typst_raw_literal(trimmed))
}

fn escape_typst_string(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

fn convert_inline_links(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut result = String::new();
    let mut index = 0usize;

    while index < chars.len() {
        if chars[index] == '[' && (index == 0 || chars[index - 1] != '\\') {
            let mut label_end = index + 1;
            while label_end < chars.len() {
                if chars[label_end] == ']' && chars[label_end - 1] != '\\' {
                    break;
                }
                label_end += 1;
            }

            if label_end + 1 < chars.len() && chars[label_end + 1] == '(' {
                let mut url_end = label_end + 2;
                while url_end < chars.len() {
                    if chars[url_end] == ')' && chars[url_end - 1] != '\\' {
                        break;
                    }
                    url_end += 1;
                }

                if url_end < chars.len() {
                    let label: String = chars[index + 1..label_end].iter().collect();
                    let url: String = chars[label_end + 2..url_end].iter().collect();
                    let trimmed_url = url.trim();

                    if !trimmed_url.is_empty() {
                        result.push_str(&format!(
                            "#link(\"{}\")[{}]",
                            escape_typst_string(trimmed_url),
                            convert_inline_links(&convert_inline_latex_math(
                                &normalize_spaced_dollar_delimiters(label.trim())
                            ))
                        ));
                        index = url_end + 1;
                        continue;
                    }
                }
            }
        }

        result.push(chars[index]);
        index += 1;
    }

    result
}

fn convert_inline_markdown(input: &str) -> String {
    convert_inline_links(&convert_inline_latex_math(
        &normalize_spaced_dollar_delimiters(input),
    ))
}

fn markdown_image_to_typst(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with("![") {
        return None;
    }

    let close_alt = trimmed.find("](")?;
    if close_alt < 2 {
        return None;
    }
    let alt = &trimmed[2..close_alt];
    let path_part = &trimmed[close_alt + 2..];
    let close_path = path_part.rfind(')')?;
    let path = path_part[..close_path].trim();
    if path.is_empty() {
        return None;
    }

    if alt.trim().is_empty() {
        return Some(format!("#image(\"{}\", width: 100%)", path));
    }

    Some(format!(
        "#figure(image(\"{}\", width: 100%), caption: [{}])",
        path,
        convert_inline_markdown(alt.trim())
    ))
}

fn normalize_spaced_dollar_delimiters(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::new();
    let mut index = 0usize;

    while index < chars.len() {
        if chars[index] == '$' {
            let mut look = index + 1;
            while look < chars.len() && chars[look].is_whitespace() {
                look += 1;
            }
            if look < chars.len() && chars[look] == '$' {
                out.push('$');
                index = look + 1;
                continue;
            }
        }

        out.push(chars[index]);
        index += 1;
    }

    out
}

fn is_markdown_table_separator(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.contains('|') {
        return false;
    }

    let normalized = trimmed.trim_matches('|').replace(' ', "");
    !normalized.is_empty()
        && normalized
            .split('|')
            .all(|cell| !cell.is_empty() && cell.chars().all(|ch| ch == '-' || ch == ':'))
}

fn split_markdown_table_row(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

fn escape_typst_cell_text(input: &str) -> String {
    input.replace('[', "\\[").replace(']', "\\]")
}

fn markdown_table_to_typst(lines: &[&str], start: usize) -> Option<(String, usize)> {
    if start + 1 >= lines.len() {
        return None;
    }

    let header_line = lines[start].trim();
    let separator_line = lines[start + 1].trim();
    if !header_line.contains('|') || !is_markdown_table_separator(separator_line) {
        return None;
    }

    let header_cells = split_markdown_table_row(header_line);
    if header_cells.is_empty() {
        return None;
    }

    let column_count = header_cells.len();
    let mut all_rows: Vec<Vec<String>> = vec![header_cells];
    let mut cursor = start + 2;

    while cursor < lines.len() {
        let row_line = lines[cursor].trim();
        if row_line.is_empty() || !row_line.contains('|') {
            break;
        }
        let row = split_markdown_table_row(row_line);
        if row.is_empty() {
            break;
        }

        all_rows.push(row);
        cursor += 1;
    }

    let mut typst_lines = vec![format!("#table(columns: {},", column_count)];
    for row in all_rows {
        for index in 0..column_count {
            let raw = row.get(index).map(String::as_str).unwrap_or("");
            let converted = convert_inline_markdown(raw);
            let cell_content = converted.trim();
            let rendered = if cell_content.contains("#mi(")
                || cell_content.contains("#mitex(")
                || cell_content.contains("#math")
                || cell_content.contains("#link(")
            {
                cell_content.to_string()
            } else {
                escape_typst_cell_text(cell_content)
            };
            typst_lines.push(format!("  [{}],", rendered));
        }
    }
    typst_lines.push(")".to_string());

    Some((typst_lines.join("\n"), cursor - start))
}

fn convert_inline_latex_math(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut result = String::new();
    let mut index = 0;

    while index < chars.len() {
        let current = chars[index];
        if current == '$' && (index == 0 || chars[index - 1] != '\\') {
            let mut end = index + 1;
            while end < chars.len() {
                if chars[end] == '$' && chars[end - 1] != '\\' {
                    break;
                }
                end += 1;
            }

            if end < chars.len() {
                let latex: String = chars[index + 1..end].iter().collect();
                result.push_str(&inline_latex_to_typst(&latex));
                index = end + 1;
                continue;
            }
        }

        result.push(current);
        index += 1;
    }

    result
}

pub fn convert_markdown_to_typst(markdown: &str) -> String {
    let normalized = markdown.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    let mut output: Vec<String> = Vec::new();
    let mut index = 0;
    let mut in_code_fence = false;

    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim();

        if let Some((table_typst, consumed)) = markdown_table_to_typst(&lines, index) {
            output.push(table_typst);
            index += consumed;
            continue;
        }

        if trimmed.starts_with("```") {
            in_code_fence = !in_code_fence;
            output.push(trimmed.to_string());
            index += 1;
            continue;
        }

        if in_code_fence {
            output.push(line.to_string());
            index += 1;
            continue;
        }

        if trimmed.starts_with("$$") {
            let mut block_math = String::new();
            let mut closed = false;

            if trimmed != "$$" {
                block_math.push_str(trimmed.trim_start_matches("$$"));
                if block_math.ends_with("$$") {
                    block_math.truncate(block_math.len().saturating_sub(2));
                    closed = true;
                }
            }

            index += 1;
            while !closed && index < lines.len() {
                let math_line = lines[index];
                if math_line.trim_end().ends_with("$$") {
                    let content = math_line.trim_end_matches('$');
                    if !block_math.is_empty() && !content.is_empty() {
                        block_math.push('\n');
                    }
                    block_math.push_str(content);
                    index += 1;
                    break;
                }

                if !block_math.is_empty() {
                    block_math.push('\n');
                }
                block_math.push_str(math_line);
                index += 1;
            }

            let converted = block_latex_to_typst(block_math.trim());
            if !converted.is_empty() {
                output.push(converted);
            }
            continue;
        }

        if trimmed.starts_with('#') {
            let level = trimmed.chars().take_while(|ch| *ch == '#').count();
            let title = trimmed[level..].trim();
            if (1..=6).contains(&level) && !title.is_empty() {
                let heading_content = convert_inline_markdown(title);
                output.push(format!("{} {}", "=".repeat(level), heading_content));
                index += 1;
                continue;
            }
        }

        if let Some(image_typst) = markdown_image_to_typst(trimmed) {
            output.push(image_typst);
            index += 1;
            continue;
        }

        if let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| trimmed.strip_prefix("+ "))
        {
            output.push(format!("- {}", convert_inline_markdown(rest.trim())));
            index += 1;
            continue;
        }

        let ordered_start = trimmed
            .char_indices()
            .find(|(_, ch)| !ch.is_ascii_digit())
            .map(|(idx, ch)| (idx, ch));
        if let Some((marker_index, marker)) = ordered_start {
            if (marker == '.' || marker == ')') && trimmed[marker_index + 1..].starts_with(' ') {
                let item = trimmed[marker_index + 2..].trim();
                output.push(format!("+ {}", convert_inline_markdown(item)));
                index += 1;
                continue;
            }
        }

        if let Some(rest) = trimmed.strip_prefix('>') {
            output.push(format!("#quote[{}]", convert_inline_markdown(rest.trim())));
            index += 1;
            continue;
        }

        output.push(convert_inline_markdown(line));
        index += 1;
    }

    output.join("\n").trim().to_string()
}
