// Copyright (c) 2023 Jake Swensen
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::error::KcheckResult;
use crate::kconfig::KconfigState;
use nix::sys::utsname::uname;
use std::path::Path;

/// A representation of a kernel config.
#[derive(Clone, Debug)]
pub struct KernelConfig(String);

impl KernelConfig {
    /// Create a new kernel config object from a path to the kernel config file.
    ///
    /// This is useful for checking kernel configs that are not a part of the
    /// running system, not the default kernel, or are in non-standard locations.
    fn try_from_file<P: AsRef<Path>>(path: P) -> KcheckResult<Self> {
        let contents = kcheck_utils::file_contents_as_string(path)?;
        Ok(KernelConfig(contents))
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
            kcheck_utils::inflate_gzip_file(proc_config_gz)
                .map(|contents| KernelConfig(contents))
                .map_err(|e| e.into())
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
