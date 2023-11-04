// Copyright (c) 2023 Jake Swensen
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! A tool for developing and debugging kernel config options.

use kcheck::config::KcheckConfig;
use kcheck::kernel::KernelConfig;
use std::path::PathBuf;

fn main() {
    let kcheck_serial = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("configs")
        .join("kcheck-serial.toml");
    println!("{kcheck_serial:?}");

    let kcheck_random = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("configs")
        .join("kcheck-random.toml");
    println!("{kcheck_random:?}");

    let files = vec![kcheck_serial, kcheck_random];

    let cfg = KcheckConfig::generate(files);
    println!("{cfg:#?}");

    let kernel_cfg = KernelConfig::try_from_system().unwrap();
    println!("{kernel_cfg:#?}");
}
