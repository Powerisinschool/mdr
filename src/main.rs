mod assets;
mod compiler;

use std::borrow::Cow;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

#[derive(Debug, PartialEq)]
pub enum CliAction {
    Run(Options),
    Help,
    Version,
}

#[derive(Debug, PartialEq)]
pub struct Options {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub images_dir: PathBuf,
    pub images_url_prefix: String,
    pub refresh_images: bool,
    pub no_fetch: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            input_path: PathBuf::from("README.mdr"),
            output_path: PathBuf::from("README.md"),
            images_dir: PathBuf::from("./images/mdr"),
            images_url_prefix: "images/mdr".to_string(),
            refresh_images: false,
            no_fetch: false,
        }
    }
}

fn main() {
    let opts = match parse_args_from(env::args().skip(1)) {
        Ok(CliAction::Run(opts)) => opts,
        Ok(CliAction::Help) => {
            print_help();
            process::exit(0);
        }
        Ok(CliAction::Version) => {
            println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
            process::exit(0);
        }
        Err(err) => {
            eprintln!("Error: {err}");
            process::exit(1);
        }
    };

    let source = fs::read_to_string(&opts.input_path).unwrap_or_else(|err| {
        eprintln!("Failed to read {}: {err}", opts.input_path.display());
        process::exit(1);
    });

    let mut referenced_images = HashSet::new();
    let compiled = compiler::compile(&source, &opts, &mut referenced_images);

    if !opts.no_fetch {
        assets::prune_unused_images(&opts.images_dir, &referenced_images);
    }

    fs::write(&opts.output_path, compiled).unwrap_or_else(|err| {
        eprintln!("Failed to write {}: {err}", opts.output_path.display());
        process::exit(1);
    });
}

fn parse_args_from<I, S>(args: I) -> Result<CliAction, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut iter_args = args.into_iter().map(Into::into);

    let grouped_args = std::iter::from_fn(move || {
        let arg = iter_args.next()?;

        let take_n = match arg.as_str() {
            "--images-dir" | "--images-prefix" => 1,
            _ => 0,
        };

        let params: Vec<String> = iter_args.by_ref().take(take_n).collect();

        Some((arg, params))
    });

    let mut positional = Vec::new();
    let mut images_dir: Option<PathBuf> = None;
    let mut images_prefix: Option<String> = None;
    let mut refresh_images = false;
    let mut no_fetch = false;

    for (arg, params) in grouped_args {
        match arg.as_str() {
            "--help" | "-h" => return Ok(CliAction::Help),
            "--version" | "-V" => return Ok(CliAction::Version),
            "--images-dir" => {
                if params.is_empty() || params[0].starts_with('-') {
                    return Err("--images-dir requires a path".into());
                }
                images_dir = Some(PathBuf::from(&params[0]));
            }
            "--images-prefix" => {
                if params.is_empty() || params[0].starts_with('-') {
                    return Err("--images-prefix requires a path".into());
                }
                images_prefix = Some(params[0].trim_end_matches('/').to_string());
            }
            "--refresh-images" => refresh_images = true,
            "--no-fetch" => no_fetch = true,
            flag if flag.starts_with('-') => {
                return Err(format!("Unknown flag: {flag}"));
            }
            val => positional.push(val.to_string()),
        }
    }

    if positional.len() > 2 {
        return Err(format!(
            "Expected at most 2 positional arguments (INPUT and OUTPUT), got {}",
            positional.len()
        ));
    }

    let input_str = positional
        .first()
        .map(String::as_str)
        .unwrap_or("README.mdr");
    let output_str: Cow<'_, str> = positional
        .get(1)
        .map(|s| Cow::Borrowed(s.as_str()))
        .unwrap_or_else(|| {
            let input_path = Path::new(input_str);
            if input_path.file_stem().is_some() {
                Cow::Owned(
                    input_path
                        .with_extension("md")
                        .to_string_lossy()
                        .into_owned(),
                )
            } else {
                Cow::Borrowed("README.md")
            }
        });

    if Path::new(input_str) == Path::new(output_str.as_ref()) {
        return Err(format!(
            "Input and output paths cannot be the same: {input_str}"
        ));
    }

    let input_path = PathBuf::from(input_str);
    let output_path = PathBuf::from(output_str.to_string());

    let output_dir = output_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let resolved_images_dir = images_dir.unwrap_or_else(|| output_dir.join("images").join("mdr"));

    let images_url_prefix = images_prefix.unwrap_or_else(|| {
        if let Ok(stripped) = resolved_images_dir.strip_prefix(output_dir) {
            stripped
                .to_string_lossy()
                .trim_end_matches('/')
                .replace('\\', "/")
        } else if resolved_images_dir.is_relative() {
            resolved_images_dir
                .to_string_lossy()
                .trim_start_matches("./")
                .trim_end_matches('/')
                .replace('\\', "/")
        } else {
            "images/mdr".to_string()
        }
    });

    Ok(CliAction::Run(Options {
        input_path,
        output_path,
        images_dir: resolved_images_dir,
        images_url_prefix,
        refresh_images,
        no_fetch,
    }))
}

fn print_help() {
    let help_text = r#"Usage: mdr [OPTIONS] [INPUT] [OUTPUT]
Options:
  --images-dir <DIR>       Directory to store cached images (default: ./images/mdr)
  --images-prefix <URL>    URL prefix for images (default: relative path to images directory)
  --refresh-images         Refresh cached images even if they exist
  --no-fetch               Do not fetch or prune images
  -h, --help               Print this help message
  -V, --version            Print version information
Arguments:
  INPUT                    Input Markdown file (default: README.mdr)
  OUTPUT                   Output Markdown file (default: <INPUT> with .md extension)
"#;
    println!("{help_text}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infers_output_path_from_single_mdr_arg() {
        let CliAction::Run(opts) = parse_args_from(["docs/guide.mdr"]).unwrap() else {
            panic!("expected CliAction::Run");
        };

        assert_eq!(opts.input_path, PathBuf::from("docs/guide.mdr"));
        assert_eq!(opts.output_path, PathBuf::from("docs/guide.md"));
    }

    #[test]
    fn infers_proper_images_dir_and_prefix() {
        let CliAction::Run(opts) = parse_args_from(["docs/guide.mdr"]).unwrap() else {
            panic!("expected CliAction::Run");
        };

        assert_eq!(opts.images_dir, PathBuf::from("docs/images/mdr"));
        assert_eq!(opts.images_url_prefix, "images/mdr");
    }

    #[test]
    fn images_prefix_trims_when_given_trailing_slash() {
        let CliAction::Run(opts) = parse_args_from(["--images-prefix", "assets/mdr/"]).unwrap()
        else {
            panic!("expected CliAction::Run");
        };

        assert_eq!(opts.images_url_prefix, "assets/mdr");
    }

    #[test]
    fn flag_invalidates_with_missing_argument() {
        let err = parse_args_from(["--images-dir", "--no-fetch"]).unwrap_err();
        assert!(err.contains("--images-dir requires a path"));
    }

    #[test]
    fn zero_arguments_properly_defaults() {
        let args: Vec<String> = vec![];
        let CliAction::Run(opts) = parse_args_from(args).unwrap() else {
            panic!("expected CliAction::Run");
        };

        assert_eq!(opts.input_path, PathBuf::from("README.mdr"));
        assert_eq!(opts.output_path, PathBuf::from("README.md"));
        assert_eq!(opts.images_dir, PathBuf::from("./images/mdr"));
        assert_eq!(opts.images_url_prefix, "images/mdr");
    }

    #[test]
    fn too_many_positional_arguments() {
        let err = parse_args_from(["input.mdr", "output.md", "extra"]).unwrap_err();
        assert!(err.contains("2"));
    }

    #[test]
    fn help_flag_triggers_help_action() {
        let action = parse_args_from(["--help"]).unwrap();
        assert_eq!(action, CliAction::Help);
    }

    #[test]
    fn version_flag_triggers_version_action() {
        let action = parse_args_from(["--version"]).unwrap();
        assert_eq!(action, CliAction::Version);
    }

    #[test]
    fn rejects_overwriting_existing_md_input() {
        let err = parse_args_from(["README.md"]).unwrap_err();
        assert!(err.contains("same"));
    }
}
