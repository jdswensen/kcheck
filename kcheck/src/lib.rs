// Copyright (c) 2023 Jake Swensen
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! A library for working with kernel config information.
//!
//! Works with:
//! - Systems that utilize Kconfig (Linux, Zephyr)
//!
//! Problem statement:
//! - Software may run on unknown system configurations
//! - Software may require specific kernel config options to be enabled
//! - Software may be run on a minimal config system
//! - User wants to understand the reasons behind a kernel config setting
//! - User wants to be able to check the state of kernel config options.
//! - User may want to enforce runtime checks on kernel config options.
//!
//! todo: derive readme from doc comments

#[cfg(feature = "cli-table")]
use cli_table::{CellStruct, Color, Style, Table};

pub mod config;
pub mod error;
pub mod kconfig;
pub mod kernel;
mod util;

use config::KcheckConfig;
pub use error::{KcheckError, KcheckResult};
use kconfig::KconfigState;
use kernel::{KernelConfig, KernelConfigBuilder};
use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq)]
enum CheckResult {
    Pass,
    #[default]
    Fail,
}

impl From<bool> for CheckResult {
    fn from(b: bool) -> Self {
        if b {
            CheckResult::Pass
        } else {
            CheckResult::Fail
        }
    }
}

impl std::fmt::Display for CheckResult {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            CheckResult::Pass => write!(f, "Pass"),
            CheckResult::Fail => write!(f, "Fail"),
        }
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "cli-table", derive(Table))]
pub struct KcheckConfigResult {
    #[cfg_attr(feature = "cli-table", table(title = "Config Option"))]
    name: String,
    #[cfg_attr(feature = "cli-table", table(title = "Desired State"))]
    desired_state: KconfigState,
    #[cfg_attr(feature = "cli-table", table(title = "Kernel State"))]
    kernel_state: KconfigState,
    #[cfg_attr(feature = "cli-table", table(title = "Result"))]
    #[cfg_attr(feature = "cli-table", table(customize_fn = "convert_check_result"))]
    result: CheckResult,
}

#[cfg(feature = "cli-table")]
fn convert_check_result(cell: CellStruct, result: &CheckResult) -> CellStruct {
    match result {
        CheckResult::Pass => cell.foreground_color(Some(Color::Green)),
        CheckResult::Fail => cell.foreground_color(Some(Color::Red)),
    }
}

pub struct Kcheck {
    config: KcheckConfig,
    kernel: KernelConfig,
}

impl Kcheck {
    /// Create a new [`Kcheck`] instance from the running system's kernel config.
    pub fn new_from_system<P: AsRef<Path>>(fragments: Vec<P>) -> KcheckResult<Self> {
        let config = KcheckConfig::generate(fragments)?;
        let kernel = KernelConfigBuilder::default().system().build()?;

        Ok(Self { config, kernel })
    }

    /// Create a new [`Kcheck`] instance from a user-provided kernel config.
    pub fn new_from_user<P: AsRef<Path>, K: AsRef<Path>>(
        fragments: Vec<P>,
        kernel: K,
    ) -> KcheckResult<Self> {
        let config = KcheckConfig::generate(fragments)?;
        let kernel = KernelConfigBuilder::default().user(kernel).build()?;

        Ok(Self { config, kernel })
    }

    /// Returns a list of desired configuration options and their current state in a kernel config.
    pub fn perform_check(&self) -> KcheckResult<Vec<KcheckConfigResult>> {
        let config = self.config.clone().into_iter();

        let mut results = Vec::new();

        for (name, desired_state) in config {
            let kernel_state = self.kernel.option(&name)?;
            let cfg_result = desired_state.check(kernel_state.clone());

            results.push(KcheckConfigResult {
                name,
                desired_state,
                kernel_state,
                result: cfg_result.into(),
            });
        }

        Ok(results)
    }
}
