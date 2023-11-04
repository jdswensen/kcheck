// Copyright (c) 2023 Jake Swensen
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::error::KcheckResult;
use crate::kconfig::KconfigState;
use nix::sys::utsname::uname;
use std::path::{Path, PathBuf};
use std::str::FromStr;

impl FromStr for KernelConfig {
    type Err = KcheckError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let all_lines = s.lines();
        let mut lines: Vec<String> = Vec::new();

        for line in all_lines {
            lines.push(line.to_string());
        }

        Ok(KernelConfig {
            src: KernelConfigSource::default(),
            lines,
        })
    }
}

#[derive(Clone, Debug, Default)]
pub enum KernelConfigSource {
    #[default]
    String,
    File(PathBuf),
    Stdin,
}

/// A representation of a kernel config.
#[derive(Clone, Debug, Default)]
pub struct KernelConfig {
    src: KernelConfigSource,
    lines: Vec<String>,
}

impl From<PathBuf> for KernelConfigSource {
    fn from(path: PathBuf) -> Self {
        KernelConfigSource::File(path)
    }
}

impl KernelConfig {
    /// Create a new kernel config object from a path to the kernel config file.
    ///
    /// This is useful for checking kernel configs that are not a part of the
    /// running system, not the default kernel, or are in non-standard locations.
    fn try_from_file<P: AsRef<Path>>(path: P) -> KcheckResult<Self> {
        let contents = kcheck_utils::file_contents_as_string(path.as_ref())?;
        let mut config = Self::from_str(contents.as_str())?;

        // Set the source type to a file
        config.src = path.as_ref().to_path_buf().into();
        Ok(config)
    }

    /// Create a `KernelConfig` object from the system`s kernel config file.
    ///
    /// Looks for the config in the following default paths:
    /// - /proc/config.gz
    /// - /boot/config
    /// - /boot/config-$(uname -r)
    pub fn try_from_system() -> KcheckResult<Self> {
        let proc_config_gz = Path::new("/proc/config.gz");
        let boot_config = Path::new("/boot/config");
        let boot_config_release_string = format!(
            "/boot/config-{}",
            uname().unwrap().release().to_string_lossy()
        );

        if proc_config_gz.exists() {
            let contents = kcheck_utils::inflate_gzip_file(proc_config_gz)?;
            Self::from_str(contents.as_str())
        } else if boot_config.exists() {
            Self::try_from_file(boot_config)
        } else if Path::new(&boot_config_release_string).exists() {
            Self::try_from_file(boot_config_release_string)
        } else {
            Err(crate::error::KcheckError::KernelConfigNotFound)
        }
    }

    /// Get the state of a kernel config option.
    fn get_option(&self, option: String) -> KconfigState {
        todo!()
    }

    /// Check the state of a kernel config option.
    ///
    /// Returns true if the option is in the desired state, false otherwise.
    fn check_option(&self, desired_option: String, desired_state: KconfigState) -> bool {
        todo!()
    }
}
