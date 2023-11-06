// Copyright (c) 2023 Jake Swensen
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::error::{KcheckError, KcheckResult};
use crate::kconfig::KconfigState;
use nix::sys::utsname::uname;
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum KernelConfigSource {
    #[default]
    String,
    File(PathBuf),
    Stdin,
}

impl From<PathBuf> for KernelConfigSource {
    fn from(path: PathBuf) -> Self {
        KernelConfigSource::File(path)
    }
}

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

/// A representation of a kernel config.
#[derive(Clone, Debug, Default)]
pub struct KernelConfig {
    src: KernelConfigSource,
    lines: Vec<String>,
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
    pub fn get_option(&self, option: &str) -> KcheckResult<KconfigState> {
        // Superset of the option string
        // Used to rule out false positives
        let super_string = format!("{option}_");

        // Seach the config for the desired option and store the result
        let mut found_state: Vec<KcheckResult<KconfigState>> = self.lines.iter().fold(
            Vec::<KcheckResult<KconfigState>>::new(),
            |mut result, line| {
                if line.contains(option) && !line.contains(&super_string) {
                    // The config option has been found, now split up the line
                    let line_parts: Vec<&str> = line.split_inclusive(option).collect();

                    if Self::is_comment(line_parts[0]) && Self::contains_is_not_set(line_parts[1]) {
                        result.push(Ok(KconfigState::NotSet));
                    } else if line_parts.len() > 1
                        && !Self::is_comment(line_parts[0])
                        && line_parts[1].contains('=')
                    {
                        let value = line_parts[1].split('=').collect::<Vec<&str>>()[1];
                        match value {
                            "y" => result.push(Ok(KconfigState::On)),
                            "m" => result.push(Ok(KconfigState::Module)),
                            "n" => result.push(Ok(KconfigState::Off)),
                            v => result
                                .push(Err(KcheckError::UnknownKernelConfigOption(v.to_string()))),
                        }
                    } else {
                        result.push(Err(KcheckError::KernelConfigParseError))
                    }
                }

                result
            },
        );

        // Parse results
        match found_state.len() {
            0 => Ok(KconfigState::NotFound),
            1 => found_state.remove(0),
            _ => Err(KcheckError::DuplicateConfig(option.to_string())),
        }
    }

    fn contains_is_not_set(option: &str) -> bool {
        option.contains("is not set")
    }

    fn is_comment(line: &str) -> bool {
        line.starts_with('#')
    }

    /// Check the state of a kernel config option.
    ///
    /// Returns true if the option is in the desired state, false otherwise.
    pub fn check_option(&self, desired_option: &str, desired_state: KconfigState) -> bool {
        match self.get_option(desired_option) {
            Ok(state) => state == desired_state,
            Err(_) => false,
        }
    }

    /// Internal function for appending kernel config options to the KernelConfig struct.
    pub(crate) fn push_option(&mut self, option: &str, state: KconfigState) {
        let string = match state {
            KconfigState::NotFound => String::default(),
            KconfigState::NotSet => format!("# {option} is not set"),
            KconfigState::Off | KconfigState::Disabled => format!("{option}=n"),
            KconfigState::On | KconfigState::Enabled => format!("{option}=y"),
            KconfigState::Module => format!("{option}=m"),
            KconfigState::Value(v) => todo!(),
            KconfigState::Text(s) => format!("{option}=\"{s}\""),
        };

        self.lines.push(string);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    enum AssertMatch {
        True,
        False,
    }

    fn helper_create_kernel_cfg(options: &[(&str, KconfigState)]) -> KernelConfig {
        let mut kernel_cfg = KernelConfig::default();
        for (option, state) in options {
            kernel_cfg.push_option(option, state.clone());
        }

        kernel_cfg
    }

    fn helper_assert_option_state_ok(
        kernel_cfg: &KernelConfig,
        option: &str,
        expected: KconfigState,
        assert_match: AssertMatch,
    ) {
        let result = kernel_cfg
            .get_option(option)
            .expect("Expected to get an option state");

        if assert_match == AssertMatch::True {
            assert_eq!(expected, result);
        } else {
            assert_ne!(expected, result);
        }
    }

    fn helper_assert_option_state_err(
        kernel_cfg: &KernelConfig,
        option: &str,
        expected: KcheckError,
    ) {
        let result = kernel_cfg
            .get_option(option)
            .expect_err("Expected to get an option state error");
        assert_eq!(expected, result);
    }

    #[test]
    fn success_get_option_on() {
        let test_option = "CONFIG_TEST";
        let test_state = KconfigState::On;
        let test_data = [(test_option, test_state.clone())];
        let kernel_cfg = helper_create_kernel_cfg(&test_data);

        helper_assert_option_state_ok(
            &kernel_cfg,
            test_option,
            test_state.clone(),
            AssertMatch::True,
        );
        assert!(kernel_cfg.check_option(test_option, test_state));
    }

    #[test]
    fn success_get_option_off() {
        let test_option = "CONFIG_TEST_OFF";
        let test_state = KconfigState::Off;
        let test_data = [(test_option, test_state.clone())];
        let kernel_cfg = helper_create_kernel_cfg(&test_data);

        helper_assert_option_state_ok(
            &kernel_cfg,
            test_option,
            test_state.clone(),
            AssertMatch::True,
        );
        assert!(kernel_cfg.check_option(test_option, test_state));
    }

    #[test]
    fn success_get_option_not_set() {
        let test_option = "CONFIG_TEST_NOT_SET";
        let test_state = KconfigState::NotSet;
        let test_data = [(test_option, test_state.clone())];
        let kernel_cfg = helper_create_kernel_cfg(&test_data);

        helper_assert_option_state_ok(
            &kernel_cfg,
            test_option,
            test_state.clone(),
            AssertMatch::True,
        );
        assert!(kernel_cfg.check_option(test_option, test_state));
    }

    #[test]
    fn success_get_option_not_found() {
        let test_option = "CONFIG_DOES_NOT_EXIST";
        let test_state = KconfigState::NotFound;
        let test_data = [
            ("CONFIG_TEST_ONE", KconfigState::On),
            ("CONFIG_TEST_TWO", KconfigState::Off),
            ("CONFIG_TEST_THREE", KconfigState::Module),
        ];
        let kernel_cfg = helper_create_kernel_cfg(&test_data);

        helper_assert_option_state_ok(
            &kernel_cfg,
            test_option,
            test_state.clone(),
            AssertMatch::True,
        );
        assert!(kernel_cfg.check_option(test_option, test_state));
    }

    #[test]
    fn fail_wrong_option_state() {
        let test_option = "CONFIG_TEST";
        let test_state = KconfigState::On;
        let test_data = [(test_option, test_state.clone())];
        let kernel_cfg = helper_create_kernel_cfg(&test_data);

        let expected = KconfigState::Off;
        helper_assert_option_state_ok(&kernel_cfg, test_option, expected, AssertMatch::False);
    }

    #[test]
    fn fail_unknown_option() {
        let test_option = "CONFIG_INCORRECT";
        let test_state = KconfigState::Text("incorrect".to_string());

        let test_data = [(test_option, test_state.clone())];
        let kernel_cfg = helper_create_kernel_cfg(&test_data);

        let expected = KcheckError::UnknownKernelConfigOption("\"incorrect\"".to_string());
        helper_assert_option_state_err(&kernel_cfg, test_option, expected);

        // On a failed option lookup via `get_option`, `check_option` should return false
        assert!(!kernel_cfg.check_option(test_option, test_state));
    }

    #[test]
    fn fail_duplicate_option() {
        let test_option = "CONFIG_TEST";
        let test_state = KconfigState::On;
        let test_data = [
            (test_option, test_state.clone()),
            (test_option, test_state.clone()),
        ];
        let kernel_cfg = helper_create_kernel_cfg(&test_data);

        let expected = KcheckError::DuplicateConfig(test_option.to_string());
        helper_assert_option_state_err(&kernel_cfg, test_option, expected);
    }

    #[test]
    fn fail_kernel_config_parse() {
        let test_option = "CONFIG_TEST";
        let mut kernel_cfg = KernelConfig::default();
        kernel_cfg.lines.push(test_option.to_string());

        let expected = KcheckError::KernelConfigParseError;
        helper_assert_option_state_err(&kernel_cfg, test_option, expected)
    }

    #[test]
    fn success_kernel_config_from_str() {
        let raw_one = "CONFIG_TEST_ONE=y";
        let raw_two = "CONFIG_TEST_TWO=n";
        let raw_three = "# CONFIG_TEST_THREE is not set";
        let raw_config = format!("{raw_one}\n{raw_two}\n{raw_three}");
        let cfg = KernelConfig::from_str(&raw_config)
            .expect("Expected to create a kernel config from a string");

        helper_assert_option_state_ok(&cfg, "CONFIG_TEST_ONE", KconfigState::On, AssertMatch::True);
        helper_assert_option_state_ok(
            &cfg,
            "CONFIG_TEST_TWO",
            KconfigState::Off,
            AssertMatch::True,
        );
        helper_assert_option_state_ok(
            &cfg,
            "CONFIG_TEST_THREE",
            KconfigState::NotSet,
            AssertMatch::True,
        );
    }

    #[test]
    fn success_kernel_config_source_from_pathbuf() {
        let path = PathBuf::from("/path/to/config");
        let source = KernelConfigSource::from(path.clone());
        assert_eq!(source, KernelConfigSource::File(path));
    }
}
