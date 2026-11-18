mod assets;
mod compiler;

use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

pub struct Options {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub images_dir: PathBuf,
    pub images_url_prefix: String,
    pub refresh_images: bool,
    pub no_fetch: bool,
}

fn main() {
    let opts = parse_args();

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

fn parse_args() -> Options {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut positional = Vec::new();
    let mut images_dir: Option<PathBuf> = None;
    let mut images_prefix: Option<String> = None;
    let mut refresh_images = false;
    let mut no_fetch = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--images-dir" => {
                i += 1;
                let val = args.get(i).unwrap_or_else(|| {
                    eprintln!("--images-dir requires a path");
                    process::exit(1);
                });
                images_dir = Some(PathBuf::from(val));
            }
            "--images-prefix" => {
                i += 1;
                let val = args.get(i).unwrap_or_else(|| {
                    eprintln!("--images-prefix requires a relative URL path");
                    process::exit(1);
                });
                images_prefix = Some(val.trim_end_matches('/').to_string());
            }
            "--refresh-images" => refresh_images = true,
            "--no-fetch" => no_fetch = true,
            flag if flag.starts_with('-') => {
                eprintln!("Unknown flag: {flag}");
                process::exit(1);
            }
            val => positional.push(val.to_string()),
        }
        i += 1;
    }

    let input_path = PathBuf::from(
        positional
            .first()
            .map(String::as_str)
            .unwrap_or("README.mdr"),
    );
    let output_path = PathBuf::from(positional.get(1).map(String::as_str).unwrap_or("README.md"));

    let output_dir = output_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let resolved_images_dir = images_dir.unwrap_or_else(|| output_dir.join("images").join("mdr"));

    let images_url_prefix = images_prefix.unwrap_or_else(|| {
        if resolved_images_dir.is_relative() {
            resolved_images_dir
                .to_string_lossy()
                .trim_start_matches("./")
                .trim_end_matches('/')
                .replace('\\', "/")
        } else {
            resolved_images_dir
                .strip_prefix(output_dir)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| "images/mdr".to_string())
        }
    });

    Options {
        input_path,
        output_path,
        images_dir: resolved_images_dir,
        images_url_prefix,
        refresh_images,
        no_fetch,
    }
}
