// Copyright (c) 2023 Jake Swensen
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

#![doc = include_str!("../README.md")]

#[cfg(feature = "cli-table")]
use cli_table::{CellStruct, Color, Style, Table};
use std::path::PathBuf;

pub mod config;
pub mod error;
pub mod kconfig;
pub mod kernel;
mod util;

use config::{KcheckConfig, KcheckConfigBuilder};
pub use error::{KcheckError, KcheckResult};
use kconfig::KconfigState;
use kernel::{KernelConfig, KernelConfigBuilder};

#[derive(Copy, Clone, Debug, Default, PartialEq)]
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

/// Build a new [`Kcheck`] instance.
#[derive(Clone, Debug, Default)]
pub struct KcheckBuilder {
    use_system_kernel: bool,
    user_kernel_files: Vec<PathBuf>,

    use_system_config: bool,
    user_config_files: Vec<PathBuf>,
}

impl KcheckBuilder {
    /// Add new Kconfig parameters using the system's running kernel config.
    pub fn system_kernel(mut self) -> Self {
        self.use_system_kernel = true;
        self
    }

    /// Add new Kconfig parameters using a user-provided kernel config file.
    pub fn kernel_fragments(mut self, files: Vec<PathBuf>) -> Self {
        self.user_kernel_files.extend(files);
        self
    }

    /// Add new config parameters using the system's config files stored in the `/etc/` directory.
    pub fn system_config(mut self) -> Self {
        self.use_system_config = true;
        self
    }

    /// Add new config parameters using a user-provided config file.
    pub fn config_fragments(mut self, files: Vec<PathBuf>) -> Self {
        self.user_config_files.extend(files);
        self
    }

    /// Build the [`Kcheck`] instance using the provided configuration.
    pub fn build(self) -> KcheckResult<Kcheck> {
        // Gather all the kernel configuration files
        let mut user_kernel_config_builder = KernelConfigBuilder::default();
        if self.use_system_kernel {
            user_kernel_config_builder = user_kernel_config_builder.system();
        };

        if !self.user_kernel_files.is_empty() {
            for file in self.user_kernel_files {
                user_kernel_config_builder = user_kernel_config_builder.user(file);
            }
        }

        let user_kernel_config = user_kernel_config_builder.build()?;

        // Gather all the Kcheck configuration files
        let mut kcheck_config_builder = KcheckConfigBuilder::default();
        if self.use_system_config {
            kcheck_config_builder = kcheck_config_builder.system();
        };

        let kcheck_config = kcheck_config_builder
            .config_files(self.user_config_files)
            .build()?;

        Ok(Kcheck::new(kcheck_config, user_kernel_config))
    }
}

#[derive(Default)]
pub struct Kcheck {
    /// The desired kernel configuration options to check against.
    config: KcheckConfig,

    /// The kernel configuration to check.
    kernel: KernelConfig,
}

impl Kcheck {
    /// Create a new [`Kcheck`] instance with previously defined configuration.
    pub fn new(config: KcheckConfig, kernel: KernelConfig) -> Self {
        Self { config, kernel }
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

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use super::*;
    use config::KcheckConfigBuilder;
    use kconfig::KconfigOption;

    const EXPECTED_KERNEL_CONFIG: [&str; 4] = [
        "CONFIG_FOO=y",
        "CONFIG_BAR=m",
        "# CONFIG_BAZ is not set",
        "CONFIG_USB_ACM=y",
    ];

    static TEST_KCHECK_CONFIG: LazyLock<Vec<KconfigOption>> = LazyLock::new(|| {
        vec![
            KconfigOption::new("CONFIG_FOO", KconfigState::On),
            KconfigOption::new("CONFIG_BAR", KconfigState::Module),
            KconfigOption::new("CONFIG_BAZ", KconfigState::Off),
            KconfigOption::new("CONFIG_USB_ACM", KconfigState::Enabled),
        ]
    });

    const TEST_KCHECK_CONFIG_TOML: &str = r#"
        [[kernel]]
        name = "CONFIG_FOO"
        state = "On"

        [[kernel]]
        name = "CONFIG_BAR"
        state = "Module"

        [[kernel]]
        name = "CONFIG_BAZ"
        state = "Off"

        [[kernel]]
        name = "CONFIG_USB_ACM"
        state = "Enabled"
    "#;

    #[test]
    fn success_kcheck_perform_check() {
        let config = KcheckConfigBuilder::default()
            .kernel(TEST_KCHECK_CONFIG.to_owned())
            .build()
            .expect("Expected to build a Kcheck config");

        let kernel_cfg_contents = EXPECTED_KERNEL_CONFIG.join("\n");
        util::run_with_tmpfile("config", &kernel_cfg_contents, |path| {
            let kernel_cfg = KernelConfigBuilder::default()
                .user(path)
                .build()
                .expect("Expected to build a kernel config");

            let kcheck = Kcheck::new(config, kernel_cfg);
            let results = kcheck.perform_check().expect("Expected to perform check");

            for result in results {
                assert!(result.result == CheckResult::Pass);
            }
        });
    }

    #[test]
    fn success_kcheck_builder_toml() {
        let kernel_cfg_contents = EXPECTED_KERNEL_CONFIG.join("\n");
        util::run_with_tmpfile("kernel_cfg", &kernel_cfg_contents, |kernel_cfg_path| {
            util::run_with_tmpfile(
                "kcheck_cfg.toml",
                TEST_KCHECK_CONFIG_TOML,
                |kcheck_cfg_path| {
                    let kcheck = KcheckBuilder::default()
                        .kernel_fragments(vec![kernel_cfg_path])
                        .config_fragments(vec![kcheck_cfg_path])
                        .build()
                        .expect("Expected to build Kcheck structure");

                    let results = kcheck.perform_check().expect("Expected to perform check");

                    for result in results {
                        assert!(result.result == CheckResult::Pass);
                    }
                },
            );
        });
    }
}
