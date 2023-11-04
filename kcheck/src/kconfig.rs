// Copyright (c) 2023 Jake Swensen
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use serde::{Deserialize, Serialize};

/// The state of a kernel config option.
///
/// This enumeration expands the tristate system used by the kernel config into
/// a system that allows for more granular control over the desired state of
/// the kernel config. This is useful when there is a desire to check the
/// explicit state of the kernel rather than depending on the implied state.
///
/// For example, there could be a requirement that a kernel config option be
/// `Enabled` meaning that is present in the system but there is no desire to
/// force it to be set to `y` or `m`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum KconfigState {
    /// Kernel config option is not found
    NotFound,
    /// Kernel config option is set to `is not set`
    NotSet,
    /// The kernel config option is set to `n`
    Off,
    /// The kernel config option is either `NotFound`, `NotSet`, or `Off`
    Disabled,
    /// Kernel config is set to `y`
    On,
    /// Kernel config is set to `m`
    Module,
    /// Kernel config is either `y` or `m`
    Enabled,
    /// Kernel config is set to a value
    Value(u64),
    /// Kernel config is set to a text string
    Text(String),
}

/// A Kconfig option.
///
/// Used to describe the desired state or value of kernel config options.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KconfigOption {
    /// The name of the kernel config option.
    name: String,
    /// A state representing the value of the kernel config option.
    state: KconfigState,
}

impl KconfigOption {
    /// Create a new `KconfigOption`
    pub fn new(name: &str, state: KconfigState) -> Self {
        KconfigOption {
            name: name.to_string(),
            state,
        }
    }

    /// Get the name of the kernel config option.
    pub fn name(&self) -> String {
        self.name.clone()
    }

    /// Get the state of the kernel config option.
    pub fn state(&self) -> KconfigState {
        self.state.clone()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn success_new() {
        let test_option = "CONFIG_TEST_OPTION";
        let test_state = KconfigState::On;

        let option = KconfigOption::new(test_option, test_state.clone());
        assert_eq!(option.name(), test_option);
        assert_eq!(option.state(), test_state);
    }
}
