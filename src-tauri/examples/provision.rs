// SPDX-License-Identifier: GPL-2.0-or-later

//! Dev tool for the e2e runs, not a CLI for users: drives `provision` without the GUI.
//!
//!   cargo run --example provision -- probe <image>
//!   cargo run --example provision -- apply <image> <settings.json>
//!   cargo run --example provision -- render <settings.json> <dir>
//!
//! `settings.json` is an `AutoconfigConfig`, so it holds hashes, never a typed password.

// The app uses the rest of the module.
#![allow(dead_code, unused_imports)]

#[path = "../src/provision/mod.rs"]
mod provision;

use std::fs;
use std::path::Path;
use std::process::ExitCode;

use provision::AutoconfigConfig;

fn settings(path: &str) -> Result<AutoconfigConfig, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))
}

/// Writes the bundle into `dir` under the file names of the card.
fn render(cfg: &AutoconfigConfig, dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    for file in provision::render_bundle(cfg).map_err(|e| e.to_string())? {
        let name = file.path.rsplit('/').next().unwrap_or(&file.path);
        fs::write(dir.join(name), &file.data).map_err(|e| e.to_string())?;
        println!("{:o} {}", file.mode, file.path);
    }
    Ok(())
}

fn run(args: &[String]) -> Result<(), String> {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["probe", image] => {
            let support = provision::probe_image(Path::new(image)).map_err(|e| e.to_string())?;
            println!("{support:?}");
            Ok(())
        }
        ["apply", image, config] => {
            provision::provision_image(Path::new(image), &settings(config)?)
                .map_err(|e| e.to_string())
        }
        ["render", config, dir] => render(&settings(config)?, Path::new(dir)),
        _ => Err(
            "usage: probe <image> | apply <image> <settings.json> | render <settings.json> <dir>"
                .to_string(),
        ),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
