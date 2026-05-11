use chrono::Local;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[path = "../core/markdown.rs"]
mod markdown;

#[derive(Debug, Clone, Serialize)]
struct NoteBlock {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    level: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    folded: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct NoteDocument {
    title: String,
    date: String,
    #[serde(rename = "type")]
    note_type: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    content: Vec<NoteBlock>,
}

#[derive(Debug, Default)]
struct Frontmatter {
    object_type: Option<String>,
    creation_date: Option<String>,
    tags: Vec<String>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("[anytype-convert] error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        return Err(
            "usage: cargo run --manifest-path src-tauri/Cargo.toml --bin anytype-convert -- <anytype-export-dir> <foxnote-output-dir>"
                .to_string(),
        );
    }

    let input_dir = PathBuf::from(args[1].trim());
    let output_dir = PathBuf::from(args[2].trim());

    if !input_dir.is_dir() {
        return Err(format!(
            "input directory does not exist or is not a directory: {}",
            input_dir.display()
        ));
    }

    fs::create_dir_all(&output_dir)
        .map_err(|error| format!("failed to create output directory: {error}"))?;

    let mut entries: Vec<PathBuf> = fs::read_dir(&input_dir)
        .map_err(|error| format!("failed to read input directory: {error}"))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("md")
        })
        .collect();
    entries.sort();

    let mut used_note_dirs = HashSet::<String>::new();
    let mut converted_count = 0usize;

    for file_path in entries {
        let raw = fs::read_to_string(&file_path)
            .map_err(|error| format!("failed to read {}: {error}", file_path.display()))?;

        let (frontmatter, markdown_body) = split_frontmatter(&raw);
        let stem = file_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("note");
        let (title, body_without_title) = extract_title_and_body(&markdown_body, stem);

        let note_dir_name = next_unique_note_dir_name(&title, stem, &mut used_note_dirs);
        let note_dir = output_dir.join(&note_dir_name);
        fs::create_dir_all(&note_dir).map_err(|error| {
            format!(
                "failed to create note directory {}: {error}",
                note_dir.display()
            )
        })?;

        let attachments_dir = note_dir.join("attachments");
        let anytype_files_dir = input_dir.join("files");
        let (rewritten_markdown, copied_attachments) = rewrite_and_copy_attachments(
            &body_without_title,
            &anytype_files_dir,
            &attachments_dir,
        )?;

        let typst = markdown::convert_markdown_to_typst(&rewritten_markdown);
        let date = normalize_creation_date(frontmatter.creation_date.as_deref());
        let note_type = normalize_object_type(frontmatter.object_type.as_deref());
        let content_blocks = build_note_blocks_from_typst(&typst);

        let document = NoteDocument {
            title: title.clone(),
            date,
            note_type,
            tags: dedupe_trimmed(frontmatter.tags),
            content: if content_blocks.is_empty() {
                vec![NoteBlock {
                    block_type: "typst".to_string(),
                    content: Some(format!("= {title}\n")),
                    path: None,
                    width: None,
                    height: None,
                    level: None,
                    folded: None,
                    summary: None,
                }]
            } else {
                content_blocks
            },
        };

        let toml_raw = toml::to_string_pretty(&document)
            .map_err(|error| format!("failed to serialize note.toml for {}: {error}", title))?;
        fs::write(note_dir.join("note.toml"), toml_raw)
            .map_err(|error| format!("failed to write note.toml for {}: {error}", title))?;

        converted_count += 1;
        println!(
            "[anytype-convert] converted '{}' -> {} (attachments: {})",
            file_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("unknown"),
            note_dir_name,
            copied_attachments
        );
    }

    println!(
        "[anytype-convert] done. converted {} notes into {}",
        converted_count,
        output_dir.display()
    );

    Ok(())
}

fn image_block_from_typst_line(line: &str) -> Option<(String, Option<String>)> {
    let trimmed = line.trim();

    let figure_prefix = "#figure(image(\"";
    let image_prefix = "#image(\"";

    let (path_start, is_figure) = if let Some(pos) = trimmed.find(figure_prefix) {
        (pos + figure_prefix.len(), true)
    } else if let Some(pos) = trimmed.find(image_prefix) {
        (pos + image_prefix.len(), false)
    } else {
        return None;
    };

    let after = &trimmed[path_start..];
    let end_quote = after.find('"')?;
    let image_path = after[..end_quote].trim();
    if image_path.is_empty() {
        return None;
    }

    let normalized_path = image_path
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string();

    if !is_figure {
        return Some((normalized_path, None));
    }

    let caption = if let Some(caption_start) = trimmed.find("caption: [") {
        let caption_after = &trimmed[caption_start + "caption: [".len()..];
        caption_after
            .find(']')
            .map(|caption_end| caption_after[..caption_end].trim().to_string())
            .filter(|value| !value.is_empty())
    } else {
        None
    };

    Some((normalized_path, caption))
}

fn build_note_blocks_from_typst(typst: &str) -> Vec<NoteBlock> {
    let normalized = typst.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    let mut blocks: Vec<NoteBlock> = Vec::new();
    let mut typst_lines: Vec<String> = Vec::new();

    let flush_typst = |lines: &mut Vec<String>, blocks: &mut Vec<NoteBlock>| {
        let content = lines.join("\n").trim().to_string();
        lines.clear();
        if content.is_empty() {
            return;
        }
        blocks.push(NoteBlock {
            block_type: "typst".to_string(),
            content: Some(content),
            path: None,
            width: None,
            height: None,
            level: None,
            folded: None,
            summary: None,
        });
    };

    for raw_line in lines {
        if let Some((path, caption)) = image_block_from_typst_line(raw_line) {
            flush_typst(&mut typst_lines, &mut blocks);
            blocks.push(NoteBlock {
                block_type: "image".to_string(),
                content: None,
                path: Some(path),
                width: None,
                height: None,
                level: None,
                folded: None,
                summary: None,
            });

            if let Some(text) = caption {
                blocks.push(NoteBlock {
                    block_type: "image_caption".to_string(),
                    content: Some(text),
                    path: None,
                    width: None,
                    height: None,
                    level: None,
                    folded: None,
                    summary: None,
                });
            }
            continue;
        }

        typst_lines.push(raw_line.to_string());
    }

    flush_typst(&mut typst_lines, &mut blocks);
    blocks
}

fn split_frontmatter(input: &str) -> (Frontmatter, String) {
    let normalized = input.replace("\r\n", "\n");
    if !normalized.starts_with("---\n") {
        return (Frontmatter::default(), normalized);
    }

    let mut lines = normalized.lines();
    let _ = lines.next();
    let mut frontmatter_lines: Vec<String> = Vec::new();
    let mut body_lines: Vec<String> = Vec::new();
    let mut closed = false;

    for line in lines {
        if !closed && line.trim() == "---" {
            closed = true;
            continue;
        }

        if !closed {
            frontmatter_lines.push(line.to_string());
            continue;
        }

        body_lines.push(line.to_string());
    }

    if !closed {
        return (Frontmatter::default(), normalized);
    }

    let frontmatter = parse_frontmatter_lines(&frontmatter_lines);
    let body = body_lines.join("\n");
    (frontmatter, body)
}

fn parse_frontmatter_lines(lines: &[String]) -> Frontmatter {
    let mut data = HashMap::<String, Vec<String>>::new();
    let mut current_key = String::new();

    for raw_line in lines {
        let line = raw_line.trim_end();
        if line.is_empty() || line.starts_with("#") {
            continue;
        }

        if let Some((key, value)) = split_key_value(line) {
            current_key = key.to_string();
            let entry = data.entry(current_key.clone()).or_default();
            let cleaned = trim_wrapped_quotes(value.trim());
            if !cleaned.is_empty() {
                entry.push(cleaned.to_string());
            }
            continue;
        }

        let trimmed = line.trim_start();
        if let Some(item) = trimmed.strip_prefix("- ") {
            if !current_key.is_empty() {
                data.entry(current_key.clone())
                    .or_default()
                    .push(trim_wrapped_quotes(item.trim()).to_string());
            }
        }
    }

    Frontmatter {
        object_type: data
            .remove("Object type")
            .and_then(|values| values.into_iter().find(|value| !value.trim().is_empty())),
        creation_date: data
            .remove("Creation date")
            .and_then(|values| values.into_iter().find(|value| !value.trim().is_empty())),
        tags: data.remove("Tag").unwrap_or_default(),
    }
}

fn split_key_value(line: &str) -> Option<(&str, &str)> {
    let mut parts = line.splitn(2, ':');
    let key = parts.next()?.trim();
    let value = parts.next()?.trim_start();
    if key.is_empty() {
        return None;
    }
    Some((key, value))
}

fn trim_wrapped_quotes(input: &str) -> &str {
    let trimmed = input.trim();
    if trimmed.len() >= 2
        && ((trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
    {
        return &trimmed[1..trimmed.len() - 1];
    }

    trimmed
}

fn extract_title_and_body(body: &str, fallback_stem: &str) -> (String, String) {
    let normalized = body.replace("\r\n", "\n");
    let mut lines: Vec<String> = normalized.lines().map(|line| line.to_string()).collect();
    let mut title: Option<String> = None;
    let mut title_index: Option<usize> = None;

    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("# ") {
            let candidate = rest.trim();
            if !candidate.is_empty() {
                title = Some(candidate.to_string());
                title_index = Some(index);
                break;
            }
        }
        break;
    }

    if let Some(index) = title_index {
        lines.remove(index);
    }

    let resolved_title = title.unwrap_or_else(|| {
        let candidate = fallback_stem.replace('-', " ").trim().to_string();
        if candidate.is_empty() {
            "Untitled Note".to_string()
        } else {
            candidate
        }
    });

    (resolved_title, lines.join("\n").trim().to_string())
}

fn next_unique_note_dir_name(
    title: &str,
    fallback_stem: &str,
    used: &mut HashSet<String>,
) -> String {
    let base = sanitize_path_segment(title)
        .or_else(|| sanitize_path_segment(fallback_stem))
        .unwrap_or_else(|| "note".to_string());
    if !used.contains(&base) {
        used.insert(base.clone());
        return base;
    }

    let mut attempt = 2usize;
    loop {
        let candidate = format!("{base}-{attempt}");
        if !used.contains(&candidate) {
            used.insert(candidate.clone());
            return candidate;
        }
        attempt += 1;
    }
}

fn rewrite_and_copy_attachments(
    markdown: &str,
    anytype_files_dir: &Path,
    output_attachments_dir: &Path,
) -> Result<(String, usize), String> {
    let mut rewritten = String::new();
    let mut cursor = 0usize;
    let mut copied = 0usize;
    let mut copied_map = HashMap::<String, String>::new();
    let mut used_names = HashSet::<String>::new();

    while let Some(start_offset) = markdown[cursor..].find("(files/") {
        let start = cursor + start_offset;
        let path_start = start + "(".len();
        let Some(end_offset) = markdown[path_start..].find(')') else {
            break;
        };
        let path_end = path_start + end_offset;
        let relative = &markdown[path_start..path_end];

        rewritten.push_str(&markdown[cursor..start]);

        let replacement = if let Some(existing) = copied_map.get(relative) {
            existing.clone()
        } else {
            let source_relative = relative.trim_start_matches("files/").trim();
            let source_path = anytype_files_dir.join(source_relative);
            if !source_path.is_file() {
                format!("({relative})")
            } else {
                fs::create_dir_all(output_attachments_dir)
                    .map_err(|error| format!("failed to create attachments directory: {error}"))?;

                let file_name = source_path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("attachment.bin");
                let target_name = next_unique_file_name(file_name, &mut used_names);
                let target_path = output_attachments_dir.join(&target_name);
                fs::copy(&source_path, &target_path).map_err(|error| {
                    format!(
                        "failed to copy attachment {} -> {}: {error}",
                        source_path.display(),
                        target_path.display()
                    )
                })?;
                copied += 1;

                let value = format!("(./attachments/{target_name})");
                copied_map.insert(relative.to_string(), value.clone());
                value
            }
        };

        rewritten.push_str(&replacement);
        cursor = path_end + 1;
    }

    rewritten.push_str(&markdown[cursor..]);
    Ok((rewritten, copied))
}

fn next_unique_file_name(file_name: &str, used: &mut HashSet<String>) -> String {
    let cleaned = sanitize_file_name(file_name);
    if !used.contains(&cleaned) {
        used.insert(cleaned.clone());
        return cleaned;
    }

    let dot = cleaned.rfind('.');
    let (stem, ext) = match dot {
        Some(index) if index > 0 => (&cleaned[..index], &cleaned[index..]),
        _ => (cleaned.as_str(), ""),
    };

    let mut attempt = 2usize;
    loop {
        let candidate = format!("{stem}-{attempt}{ext}");
        if !used.contains(&candidate) {
            used.insert(candidate.clone());
            return candidate;
        }
        attempt += 1;
    }
}

fn sanitize_file_name(raw: &str) -> String {
    let mut result = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
            result.push(ch.to_ascii_lowercase());
        } else {
            result.push('_');
        }
    }

    let collapsed = result
        .split('_')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<&str>>()
        .join("_");

    if collapsed.is_empty() {
        "attachment.bin".to_string()
    } else {
        collapsed
    }
}

fn normalize_creation_date(raw: Option<&str>) -> String {
    let Some(value) = raw else {
        return Local::now().format("%Y-%m-%d").to_string();
    };

    let trimmed = value.trim();
    if trimmed.len() >= 10 {
        let prefix = &trimmed[..10];
        if prefix.chars().enumerate().all(|(index, ch)| {
            if index == 4 || index == 7 {
                ch == '-'
            } else {
                ch.is_ascii_digit()
            }
        }) {
            return prefix.to_string();
        }
    }

    Local::now().format("%Y-%m-%d").to_string()
}

fn normalize_object_type(raw: Option<&str>) -> String {
    match raw.unwrap_or("Note").trim().to_ascii_lowercase().as_str() {
        "page" | "note" | "notes" => "notes".to_string(),
        value if value.is_empty() => "notes".to_string(),
        value => value.to_string(),
    }
}

fn dedupe_trimmed(tags: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::<String>::new();
    let mut result = Vec::new();
    for tag in tags {
        let cleaned = tag.trim();
        if cleaned.is_empty() {
            continue;
        }
        let normalized = cleaned.to_string();
        if seen.insert(normalized.clone()) {
            result.push(normalized);
        }
    }
    result
}

fn sanitize_path_segment(raw: &str) -> Option<String> {
    let mut sanitized = String::new();

    for ch in raw.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            sanitized.push(ch.to_ascii_lowercase());
        } else if ch == '-' || ch == '_' {
            sanitized.push(ch);
        } else if ch.is_whitespace() {
            sanitized.push('-');
        }
    }

    let collapsed = sanitized
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<&str>>()
        .join("-");

    if collapsed.is_empty() {
        None
    } else {
        Some(collapsed)
    }
}
