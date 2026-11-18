use std::collections::HashMap;
use std::env;
use std::fs;
use std::process;

struct MacroDef {
    params: Vec<String>,
    body: String,
}

#[derive(Default)]
struct Context {
    themes: HashMap<String, (String, String)>,
    macros: HashMap<String, MacroDef>,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let input_path = args.get(1).map(String::as_str).unwrap_or("README.mdr");
    let output_path = args.get(2).map(String::as_str).unwrap_or("README.md");

    let source = fs::read_to_string(input_path).unwrap_or_else(|err| {
        eprintln!("Failed to read {input_path}: {err}");
        process::exit(1);
    });

    let compiled = compile(&source);

    fs::write(output_path, compiled).unwrap_or_else(|err| {
        eprintln!("Failed to write {output_path}: {err}");
        process::exit(1);
    });
}

fn compile(source: &str) -> String {
    let mut ctx = Context::default();
    let mut body_lines = Vec::new();
    let mut skip_next_blank = false;

    // Collect @theme and @def declarations
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("@theme ") {
            if let Some((name, vals)) = rest.split_once('=') {
                if let Some((dark, light)) = vals.split_once('|') {
                    ctx.themes.insert(
                        name.trim().to_string(),
                        (dark.trim().to_string(), light.trim().to_string()),
                    );
                    skip_next_blank = true;
                    continue;
                }
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

        if skip_next_blank && trimmed.is_empty() {
            skip_next_blank = false;
            continue;
        }
        skip_next_blank = false;
        body_lines.push(line);
    }

    // Expand macros and responsive <picture> blocks
    let mut out = Vec::with_capacity(body_lines.len());
    for line in body_lines {
        let expanded = expand_macros(line, &ctx.macros);
        out.push(expand_responsive_images(&expanded, &ctx.themes));
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
                            let args: Vec<&str> = raw_args.split(',').map(str::trim).collect();

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

fn expand_responsive_images(line: &str, themes: &HashMap<String, (String, String)>) -> String {
    let indent_len = line.len() - line.trim_start().len();
    let indent = &line[..indent_len];
    let trimmed = line.trim();

    // Linked responsive image: [![alt](url){attrs}](href)
    if trimmed.starts_with("[![") && trimmed.ends_with(')') {
        if let Some(href_open) = trimmed.rfind("](") {
            let inner = &trimmed[1..href_open];
            let href = &trimmed[href_open + 2..trimmed.len() - 1];
            if let Some(pic) = render_picture(inner, themes, &format!("{indent}  ")) {
                return format!(
                    "{indent}<a href=\"{href}\">\n{pic}\n{indent}</a>"
                );
            }
        }
    }

    // Standalone responsive image: ![alt](url){attrs}
    if trimmed.starts_with("![") {
        if let Some(pic) = render_picture(trimmed, themes, indent) {
            return pic;
        }
    }

    line.to_string()
}

fn render_picture(
    md_img: &str,
    themes: &HashMap<String, (String, String)>,
    indent: &str,
) -> Option<String> {
    let rest = md_img.strip_prefix("![")?;
    let alt_end = rest.find("](")?;
    let alt = &rest[..alt_end];

    let after_alt = &rest[alt_end + 2..];
    let url_end = find_url_end(after_alt)?;
    let raw_url = &after_alt[..url_end];

    // Only convert into <picture> if the URL actually uses a @theme token
    let uses_theme = themes
        .keys()
        .any(|k| raw_url.contains(&format!("{{{k}}}")));
    if !uses_theme {
        return None;
    }

    let trailing = after_alt[url_end + 1..].trim();
    let extra_attrs = parse_attrs(trailing);

    let mut dark_url = raw_url.to_string();
    let mut light_url = raw_url.to_string();
    for (key, (dark, light)) in themes {
        let token = format!("{{{key}}}");
        dark_url = dark_url.replace(&token, dark);
        light_url = light_url.replace(&token, light);
    }

    Some(format!(
        "{indent}<picture>\n\
         {indent}  <source media=\"(prefers-color-scheme: dark)\" srcset=\"{dark_url}\" />\n\
         {indent}  <source media=\"(prefers-color-scheme: light)\" srcset=\"{light_url}\" />\n\
         {indent}  <img {extra_attrs}alt=\"{alt}\" src=\"{dark_url}\" />\n\
         {indent}</picture>"
    ))
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

fn parse_attrs(trailing: &str) -> String {
    if !(trailing.starts_with('{') && trailing.ends_with('}')) {
        return String::new();
    }
    let inner = &trailing[1..trailing.len() - 1];
    let mut out = String::new();
    for part in inner.split_whitespace() {
        if let Some((k, v)) = part.split_once('=') {
            let clean_v = v.trim_matches('"');
            out.push_str(&format!("{k}=\"{clean_v}\" "));
        }
    }
    out
}
