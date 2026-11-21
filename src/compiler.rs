use crate::assets;
use crate::value::{ThemeValue, Value};
use crate::Options;
use std::collections::{BTreeMap, HashMap, HashSet};

struct MacroDef {
    params: Vec<String>,
    body: String,
}

#[derive(Default)]
struct Context {
    symbols: BTreeMap<String, Value>,
    macros: HashMap<String, MacroDef>,
}

struct ImageAttrs {
    is_live: bool,
    html_attrs: String,
}

pub fn compile(source: &str, opts: &Options, referenced_images: &mut HashSet<String>) -> String {
    let mut ctx = Context::default();
    let mut body_lines = Vec::new();
    let mut skip_next_blank = false;
    let mut in_fence = false;

    // Pass 1: Collect @theme and @def declarations, omitting fenced code blocks and blank lines after declarations
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            skip_next_blank = false;
            body_lines.push(line);
            continue;
        }

        if !in_fence {
            if let Some(rest) = trimmed.strip_prefix("@theme ") {
                if let Some((name, vals)) = rest.split_once('=') {
                    if let Some((dark, light)) = vals.split_once('|') {
                        ctx.symbols.insert(
                            name.trim().to_string(),
                            Value::Theme(ThemeValue {
                                dark: dark.trim().to_string(),
                                light: light.trim().to_string(),
                            }),
                        );
                        skip_next_blank = true;
                        continue;
                    }
                }
            } else if let Some(rest) = trimmed.strip_prefix("@text ") {
                if let Some((name, val)) = rest.split_once('=') {
                    ctx.symbols
                        .insert(name.trim().to_string(), Value::Text(val.trim().to_string()));
                    skip_next_blank = true;
                    continue;
                }
            } else if let Some(rest) = trimmed.strip_prefix("@tuple ") {
                if let Some((name, raw_tuple)) = rest.split_once('=') {
                    if let Some(tuple_val) = parse_tuple(raw_tuple.trim(), &ctx.symbols) {
                        ctx.symbols.insert(name.trim().to_string(), tuple_val);
                    } // If parsing fails, skip adding the symbol
                    skip_next_blank = true;
                    continue;
                }
            } else if let Some(rest) = trimmed.strip_prefix("@def ") {
                if let Some((sig, body)) = rest.split_once('=') {
                    if let (Some(open), Some(close)) = (sig.find('('), sig.rfind(')')) {
                        let name = sig[..open].trim().to_string();
                        let params = sig[open + 1..close]
                            .split(',')
                            .map(|p| p.trim().to_string())
                            .filter(|p| !p.is_empty())
                            .collect();
                        ctx.macros.insert(
                            name,
                            MacroDef {
                                params,
                                body: body.trim().to_string(),
                            },
                        );
                        skip_next_blank = true;
                        continue;
                    }
                }
            }
        }

        if skip_next_blank && trimmed.is_empty() {
            skip_next_blank = false;
            continue;
        }
        skip_next_blank = false;
        body_lines.push(line);
    }

    // Pass 2 & 3: Expand macros, localize images, and render <picture> blocks
    let mut out = Vec::with_capacity(body_lines.len());
    let mut in_fence = false;
    for line in body_lines {
        let trimmed = line.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            out.push(line.to_string());
            continue;
        }

        if in_fence {
            out.push(line.to_string());
        } else {
            let expanded = expand_macros(line, &ctx.macros);
            let interpolated = interpolate_symbols(&expanded, &ctx.symbols);
            out.push(process_images(&interpolated, &ctx, opts, referenced_images));
        }
    }

    let mut result = out.join("\n");
    if source.ends_with('\n') {
        result.push('\n');
    }
    result
}

fn expand_macros(line: &str, macros: &HashMap<String, MacroDef>) -> String {
    let mut current = line.to_string();

    loop {
        let mut replaced = false;
        let bytes = current.as_bytes();
        let mut i = 0;

        while i < bytes.len() {
            if bytes[i] == b'@' {
                let start_ident = i + 1;
                let mut end_ident = start_ident;
                while end_ident < bytes.len()
                    && (bytes[end_ident].is_ascii_alphanumeric() || bytes[end_ident] == b'_')
                {
                    end_ident += 1;
                }

                if end_ident > start_ident && end_ident < bytes.len() && bytes[end_ident] == b'(' {
                    let name = &current[start_ident..end_ident];
                    if let Some(def) = macros.get(name) {
                        if let Some(close_idx) = find_matching_paren(&current, end_ident) {
                            let raw_args = &current[end_ident + 1..close_idx];
                            let args = split_top_level(raw_args, ',');

                            let mut expansion = def.body.clone();
                            for (param, arg) in def.params.iter().zip(args.iter()) {
                                expansion = expansion.replace(&format!("{{{param}}}"), arg);
                            }

                            current.replace_range(i..=close_idx, &expansion);
                            replaced = true;
                            break;
                        }
                    }
                }
            }
            i += 1;
        }

        if !replaced {
            break;
        }
    }

    current
}

fn find_matching_paren(s: &str, open_idx: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, ch) in s[open_idx..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open_idx + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn process_images(
    line: &str,
    ctx: &Context,
    opts: &Options,
    referenced_images: &mut HashSet<String>,
) -> String {
    let indent_len = line.len() - line.trim_start().len();
    let indent = &line[..indent_len];
    let trimmed = line.trim();

    // Linked image: [![alt](url){attrs}](href)
    if trimmed.starts_with("[![") && trimmed.ends_with(')') {
        if let Some(href_open) = trimmed.rfind("](") {
            let inner = &trimmed[1..href_open];
            let href = &trimmed[href_open + 2..trimmed.len() - 1];
            if let Some(rendered) =
                render_image(inner, Some(href), indent, ctx, opts, referenced_images)
            {
                return rendered;
            }
        }
    }

    // Standalone image: ![alt](url){attrs}
    if trimmed.starts_with("![") {
        if let Some(rendered) = render_image(trimmed, None, indent, ctx, opts, referenced_images) {
            return rendered;
        }
    }

    line.to_string()
}

fn interpolate_symbols(line: &str, symbols: &BTreeMap<String, Value>) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;

    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after_open = &rest[open + 1..];

        if let Some(close) = after_open.find('}') {
            let expr = after_open[..close].trim();
            let (root, path) = expr.split_once('.').unwrap_or((expr, ""));

            if let Some(val) = symbols.get(root) {
                let keep_for_picture = matches!(val, Value::Theme(_)) && path.is_empty();
                if !keep_for_picture {
                    if let Some(resolved) = val.resolve_text(path) {
                        out.push_str(resolved);
                        rest = &after_open[close + 1..];
                        continue;
                    }
                }
            }

            out.push('{');
            out.push_str(&after_open[..=close]);
            rest = &after_open[close + 1..];
        } else {
            out.push('{');
            rest = after_open;
            break;
        }
    }

    out.push_str(rest);
    out
}

fn render_image(
    md_img: &str,
    href: Option<&str>,
    indent: &str,
    ctx: &Context,
    opts: &Options,
    referenced_images: &mut HashSet<String>,
) -> Option<String> {
    let rest = md_img.strip_prefix("![")?;
    let alt_end = rest.find("](")?;
    let alt = &rest[..alt_end];

    let after_alt = &rest[alt_end + 2..];
    let url_end = find_url_end(after_alt)?;
    let raw_url = &after_alt[..url_end];

    let trailing = after_alt[url_end + 1..].trim();
    let attrs = parse_attrs(trailing);

    let used_themes = ctx
        .symbols
        .iter()
        .filter_map(|(k, v)| match v {
            Value::Theme(theme) if raw_url.contains(&format!("{{{k}}}")) => Some((k, theme)),
            _ => None,
        })
        .collect::<Vec<_>>();

    if !used_themes.is_empty() {
        let mut dark_url = raw_url.to_string();
        let mut light_url = raw_url.to_string();
        for (key, theme) in used_themes {
            let token = format!("{{{key}}}");
            dark_url = dark_url.replace(&token, theme.dark.as_str());
            light_url = light_url.replace(&token, theme.light.as_str());
        }

        // Download both variants (even for live images so local fallback snapshots stay saved)
        let dark_local =
            assets::localize_url(&dark_url, alt, Some("dark"), opts, referenced_images);
        let light_local =
            assets::localize_url(&light_url, alt, Some("light"), opts, referenced_images);

        let (srcset_dark, srcset_light, img_fallback) = if attrs.is_live {
            // Live images serve remote URLs in <source> and use the downloaded snapshot as <img> fallback
            (dark_url, light_url, dark_local)
        } else {
            // Static images serve localized files everywhere (or direct URL if fetch failed)
            let fallback = dark_local.clone();
            (dark_local, light_local, fallback)
        };

        let pic_indent = if href.is_some() {
            format!("{indent}  ")
        } else {
            indent.to_string()
        };

        let pic = format!(
            "{pic_indent}<picture>\n\
             {pic_indent}  <source media=\"(prefers-color-scheme: dark)\" srcset=\"{srcset_dark}\" />\n\
             {pic_indent}  <source media=\"(prefers-color-scheme: light)\" srcset=\"{srcset_light}\" />\n\
             {pic_indent}  <img {}alt=\"{alt}\" src=\"{img_fallback}\" />\n\
             {pic_indent}</picture>",
            attrs.html_attrs
        );

        return Some(match href {
            Some(link) => format!("{indent}<a href=\"{link}\">\n{pic}\n{indent}</a>"),
            None => pic,
        });
    }

    // Non-themed external image
    if assets::is_external_url(raw_url) {
        let local = assets::localize_url(raw_url, alt, None, opts, referenced_images);

        if attrs.is_live {
            let pic_indent = if href.is_some() {
                format!("{indent}  ")
            } else {
                indent.to_string()
            };

            let pic = format!(
                "{pic_indent}<picture>\n\
                 {pic_indent}  <source srcset=\"{raw_url}\" />\n\
                 {pic_indent}  <img {}alt=\"{alt}\" src=\"{local}\" />\n\
                 {pic_indent}</picture>",
                attrs.html_attrs
            );

            return Some(match href {
                Some(link) => format!("{indent}<a href=\"{link}\">\n{pic}\n{indent}</a>"),
                None => pic,
            });
        }

        if !attrs.html_attrs.is_empty() {
            let img = format!("<img {}alt=\"{alt}\" src=\"{local}\" />", attrs.html_attrs);
            return Some(match href {
                Some(link) => format!("{indent}<a href=\"{link}\">{img}</a>"),
                None => format!("{indent}{img}"),
            });
        }

        return Some(match href {
            Some(link) => format!("{indent}[![{alt}]({local})]({link})"),
            None => format!("{indent}![{alt}]({local})"),
        });
    }

    // Already-local relative image path
    if !attrs.html_attrs.is_empty() {
        let img = format!(
            "<img {}alt=\"{alt}\" src=\"{raw_url}\" />",
            attrs.html_attrs
        );
        return Some(match href {
            Some(link) => format!("{indent}<a href=\"{link}\">{img}</a>"),
            None => format!("{indent}{img}"),
        });
    }

    Some(match href {
        Some(link) => format!("{indent}[![{alt}]({raw_url})]({link})"),
        None => format!("{indent}![{alt}]({raw_url})"),
    })
}

fn find_url_end(s: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, ch) in s.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                if depth == 0 {
                    return Some(idx);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

fn parse_attrs(trailing: &str) -> ImageAttrs {
    if !(trailing.starts_with('{') && trailing.ends_with('}')) {
        return ImageAttrs {
            is_live: false,
            html_attrs: String::new(),
        };
    }

    let inner = &trailing[1..trailing.len() - 1];
    let mut is_live = false;
    let mut html_attrs = String::new();

    for part in inner.split_whitespace() {
        if part == "live" {
            is_live = true;
        } else if let Some((k, v)) = part.split_once('=') {
            let clean_v = v.trim_matches('"');
            if k == "live" {
                is_live = clean_v == "true" || clean_v == "1";
            } else {
                html_attrs.push_str(&format!("{k}=\"{clean_v}\" "));
            }
        }
    }

    ImageAttrs {
        is_live,
        html_attrs,
    }
}

fn parse_value(raw: &str, symbols: &BTreeMap<String, Value>) -> Option<Value> {
    let trimmed = raw.trim();

    if trimmed.is_empty() {
        None
    } else if let Some(inner) = trimmed.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        Some(Value::Text(inner.to_string()))
    } else if trimmed.starts_with('(') && trimmed.ends_with(')') {
        parse_tuple(trimmed, symbols)
    } else if let Some(val) = symbols.get(trimmed) {
        Some(val.clone())
    } else {
        Some(Value::Text(trimmed.to_string())) // Unresolved text is treated as a literal string
    }
}

fn parse_tuple(raw: &str, symbols: &BTreeMap<String, Value>) -> Option<Value> {
    let mut items = Vec::new();

    if let Some(inner) = raw.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
        for item in split_top_level(inner, ',') {
            if let Some(val) = parse_value(item, symbols) {
                items.push(val);
            }
        }

        Some(Value::Tuple(items))
    } else {
        None
    }
}

fn split_top_level(s: &str, delimiter: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escape = false;
    let mut last_split = 0usize;

    for (idx, ch) in s.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        match ch {
            '\\' if in_string => escape = true,
            '"' => in_string = !in_string,
            '(' | '[' | '{' if !in_string => depth += 1,
            ')' | ']' | '}' if !in_string => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            _ if !in_string && ch == delimiter && depth == 0 => {
                parts.push(s[last_split..idx].trim());
                last_split = idx + 1;
            }
            _ => {}
        }
    }

    if last_split < s.len() {
        parts.push(s[last_split..].trim());
    }

    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn offline_opts() -> Options {
        Options {
            input_path: PathBuf::from("README.mdr"),
            output_path: PathBuf::from("README.md"),
            images_dir: PathBuf::from("/tmp/mdr_nonexistent_test_images/mdr"),
            images_url_prefix: "images/mdr".to_string(),
            refresh_images: false,
            no_fetch: true,
        }
    }

    #[test]
    fn strips_declarations_and_expands_macros() {
        let src = "\
@theme card = 161B22 | F6F8FA
@theme accent = 14B8A6 | 0F766E

@def badge(alt, label, logo) = ![{alt}](https://img.shields.io/badge/{label}-{card}?logo={logo}&logoColor={accent})

<p>
  @badge(C++, C%2B%2B, cplusplus)
</p>
";
        let mut refs = HashSet::new();
        let out = compile(src, &offline_opts(), &mut refs);

        assert!(!out.contains("@theme"));
        assert!(!out.contains("@def"));
        assert!(out.starts_with("<p>\n  <picture>"));
        assert!(out.contains("alt=\"C++\""));
        assert!(out.contains("src=\"images/mdr/cpp-dark-"));
        assert!(out.ends_with('\n'));
        assert_eq!(refs.len(), 2);
    }

    #[test]
    fn handles_balanced_parentheses_in_urls() {
        let src = "\
@theme accent = 14B8A6 | 0F766E
![Wiki](https://example.com/Rust_(programming_language)?color={accent})
";
        let mut refs = HashSet::new();
        let out = compile(src, &offline_opts(), &mut refs);

        assert!(out.contains("<picture>"));
        assert!(out.contains("alt=\"Wiki\""));
        assert_eq!(refs.len(), 2);
    }

    #[test]
    fn renders_live_linked_picture_with_remote_sources_and_local_fallback() {
        let src = "\
@theme bg = 0D1117 | F6F8FA
@theme accent = 14B8A6 | 0F766E
[![Stats](https://stats.example/api?bg={bg}&color={accent}){height=175 live}](https://github.com/Powerisinschool)
";
        let mut refs = HashSet::new();
        let out = compile(src, &offline_opts(), &mut refs);

        let expected_dark_src = "https://stats.example/api?bg=0D1117&color=14B8A6";
        let expected_light_src = "https://stats.example/api?bg=F6F8FA&color=0F766E";

        assert!(out.starts_with("<a href=\"https://github.com/Powerisinschool\">\n  <picture>"));
        assert!(out.contains(&format!("srcset=\"{expected_dark_src}\"")));
        assert!(out.contains(&format!("srcset=\"{expected_light_src}\"")));
        assert!(out.contains("<img height=\"175\" alt=\"Stats\" src=\"images/mdr/stats-dark-"));
        assert!(!out.contains("live"));
        assert!(out.ends_with("</picture>\n</a>\n"));
    }

    #[test]
    fn renders_local_and_non_themed_live_images() {
        let src = "\
![Diagram](./docs/architecture.png)
![LivePing](https://example.com/ping.svg){live}
![StaticLogo](https://example.com/logo.png){width=120}
";
        let mut refs = HashSet::new();
        let out = compile(src, &offline_opts(), &mut refs);

        assert!(out.contains("![Diagram](./docs/architecture.png)"));
        assert!(out.contains(
            "<picture>\n  <source srcset=\"https://example.com/ping.svg\" />\n  <img alt=\"LivePing\" src=\"images/mdr/liveping-"
        ));
        assert!(out.contains("<img width=\"120\" alt=\"StaticLogo\" src=\"images/mdr/staticlogo-"));
        assert_eq!(refs.len(), 2);
    }
}
