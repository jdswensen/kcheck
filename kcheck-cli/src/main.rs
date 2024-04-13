// Copyright (c) 2023 Jake Swensen
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use clap::Parser;
use cli_table::WithTitle;
use std::path::PathBuf;

/// A tool for developing and debugging kernel config options.
#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the kernel config file.
    #[arg(short, long)]
    kconfig: Option<PathBuf>,

    /// Path to Kcheck config files or fragments.
    #[arg(short, long)]
    configs: Vec<PathBuf>,
}

fn main() {
    let args = Args::parse();

    let kcheck = match args.kconfig.clone() {
        Some(k) => kcheck::Kcheck::new_from_user(args.configs.clone(), k),
        None => kcheck::Kcheck::new_from_system(args.configs.clone()),
    };

    let system = match kcheck {
        Ok(system) => system,
        Err(e) => {
            eprintln!("Failed to create Kcheck system: {e}");
            std::process::exit(1);
        }
    };

    let results = system.perform_check().unwrap();
    let table = results.with_title().display().unwrap();
    println!("{}", table);
}
