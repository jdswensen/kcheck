// Copyright (c) 2023 Jake Swensen
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::error::{KcheckError, KcheckResult};
use crate::kconfig::KconfigState;
use nix::sys::utsname::uname;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// Meta file information for a kernel config file.
#[derive(Clone, Debug, Default)]
struct KernelConfigFileInfo(PathBuf, RequiresInflate);

impl KernelConfigFileInfo {
    const PROC_CONFIG_GZ: &'static str = "/proc/config.gz";
    const BOOT_CONFIG: &'static str = "/boot/config";

    /// Determine if the provided path is a valid file.
    pub(crate) fn try_from_user<P: AsRef<Path>>(path: P) -> KcheckResult<Self> {
        let cfg = match Self::find_user_cfg(path.as_ref()) {
            Some(s) => Ok(s),
            None => Err(KcheckError::FileDoesNotExist(
                path.as_ref().to_string_lossy().to_string(),
            )),
        }?;

        Ok(cfg)
    }

    /// Search the provided path for a kernel config file.
    ///
    /// Determines if the path exists. If it does, determines if the file needs
    /// to be inflated (is a gzipped file).
    fn find_user_cfg<P: AsRef<Path>>(path: P) -> Option<Self> {
        if path.as_ref().exists() {
            let inflate = match path.as_ref().extension().and_then(OsStr::to_str) {
                Some("gz") => RequiresInflate::True,
                _ => RequiresInflate::False,
            };

            Some(Self(path.as_ref().to_path_buf(), inflate))
        } else {
            None
        }
    }

    /// Find the location of the system kernel config file.
    ///
    /// Looks in the following default paths:
    /// - /proc/config.gz
    /// - /boot/config
    /// - /boot/config-$(uname -r)
    pub(crate) fn try_from_system() -> KcheckResult<Self> {
        let sys_cfg = match Self::find_system_cfg() {
            Some(s) => s,
            None => Self::try_boot_config_release()?.ok_or(KcheckError::KernelConfigNotFound)?,
        };

        Ok(sys_cfg)
    }

    /// Search through standard system locations to find the running system config.
    ///
    /// Looks for the config in the following default paths:
    /// - /proc/config.gz
    /// - /boot/config
    ///
    /// Returns `Some` if a config file is found and exists, `None` otherwise.
    fn find_system_cfg() -> Option<Self> {
        let proc_config_gz = PathBuf::from(Self::PROC_CONFIG_GZ);
        let boot_config = PathBuf::from(Self::BOOT_CONFIG);

        if proc_config_gz.exists() {
            Some(Self(proc_config_gz, RequiresInflate::True))
        } else if boot_config.exists() {
            Some(Self(boot_config, RequiresInflate::False))
        } else {
            None
        }
    }

    /// Attempt to find the system location to a config file that corresponds to `uname -r`.
    ///
    /// This function should only be called in the event that other methods of attempting
    /// to set a kernel config file path have been unsuccessful.
    fn try_boot_config_release() -> KcheckResult<Option<Self>> {
        let boot_config_release: PathBuf = match uname()
            .ok()
            .and_then(|u| Some(u.release().to_owned()))
            .map(|r| format!("{}-{}", Self::BOOT_CONFIG, r.to_string_lossy()))
        {
            Some(s) => Ok(PathBuf::from(s)),
            None => Err(KcheckError::KernelConfigBuildError(
                "Could not get release string from uname".to_string(),
            )),
        }?;

        if boot_config_release.exists() {
            Ok(Some(Self(boot_config_release, RequiresInflate::False)))
        } else {
            Ok(None)
        }
    }
}

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

/// Enum that indicates the file type is a gzipped kernel config.
#[derive(Clone, Debug, Default)]
pub(crate) enum RequiresInflate {
    True,
    #[default]
    False,
}

#[derive(Clone, Debug, Default)]
pub struct KernelConfigBuilder {
    /// Path to the user provided kernel config file.
    usr_cfg_file: Option<PathBuf>,
    /// Flag indicating that the system kernel config should be used.
    sys_cfg_flag: bool,
    /// Meta file information for a kernel config file.
    file_info: Option<KernelConfigFileInfo>,
    /// Raw kernel config file lines.
    lines: Vec<String>,
}

impl KernelConfigBuilder {
    /// Create a new kernel config struct from meta file information
    ///
    /// Opens the file and inflates it if necessary.
    fn try_from_file_info(info: KernelConfigFileInfo) -> KcheckResult<KernelConfig> {
        let path = info.0;
        let inflate = info.1;

        let contents = match inflate {
            RequiresInflate::True => kcheck_utils::inflate_gzip_file(path.clone())?,
            RequiresInflate::False => kcheck_utils::file_contents_as_string(path.clone())?,
        };

        let mut config = KernelConfig::from_str(contents.as_str())?;

        // Set the source type to a file
        config.src = path.into();
        Ok(config)
    }

    /// Indicate that the system kernel config should be used.
    pub fn system(mut self) -> Self {
        self.sys_cfg_flag = true;
        self
    }

    /// Indicate that the user provided kernel config should be used.
    pub fn user<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.usr_cfg_file = Some(path.as_ref().to_path_buf());
        self
    }

    /// Add an option to the kernel config directly.
    ///
    /// Mutually exclusive operation to `system` or `user`.
    pub fn option(mut self, option: &str, state: KconfigState) -> Self {
        let line = match state {
            KconfigState::NotFound => String::default(),
            KconfigState::NotSet => format!("# {option} is not set"),
            KconfigState::Off | KconfigState::Disabled => format!("{option}=n"),
            KconfigState::On | KconfigState::Enabled => format!("{option}=y"),
            KconfigState::Module => format!("{option}=m"),
            KconfigState::Value(v) => todo!(),
            KconfigState::Text(s) => format!("{option}=\"{s}\""),
        };

        self.lines.push(line);
        self
    }

    /// Add multiple options to the kernel config directly.
    pub fn options(mut self, options: &[(&str, KconfigState)]) -> Self {
        for (option, state) in options {
            self = self.option(option, state.clone());
        }

        self
    }

    /// Consume the builder object and produce a `KernelConfig` object.
    pub fn build(mut self) -> KcheckResult<KernelConfig> {
        if !self.lines.is_empty() && (self.sys_cfg_flag || self.usr_cfg_file.is_some()) {
            return Err(KcheckError::KernelConfigBuildError(
                "Cannot set options manually when another builder method is used".to_string(),
            ));
        }

        if self.sys_cfg_flag && self.usr_cfg_file.is_some() {
            return Err(KcheckError::KernelConfigBuildError(
                "Both system and user config build methods are set".to_string(),
            ));
        }

        if let Some(path) = self.usr_cfg_file {
            self.file_info = Some(KernelConfigFileInfo::try_from_user(path)?);
        }

        if self.sys_cfg_flag {
            self.file_info = Some(KernelConfigFileInfo::try_from_system()?);
        }

        match self.file_info {
            Some(info) => Self::try_from_file_info(info),
            None => {
                if self.lines.is_empty() {
                    Err(KcheckError::KernelConfigBuildError(
                        "No config file information found".to_string(),
                    ))
                } else {
                    let mut config = KernelConfig::default();
                    config.lines = self.lines;
                    Ok(config)
                }
            }
        }
    }
}

/// A representation of a kernel config.
#[derive(Clone, Debug, Default)]
pub struct KernelConfig {
    src: KernelConfigSource,
    lines: Vec<String>,
}

impl KernelConfig {
    /// Get the state of a kernel config option.
    pub fn option(&self, option: &str) -> KcheckResult<KconfigState> {
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
        match self.option(desired_option) {
            Ok(state) => state == desired_state,
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rand::distributions::{Alphanumeric, DistString};
    use std::{env, io::Write};

    #[derive(Clone, Debug, PartialEq)]
    enum AssertMatch {
        True,
        False,
    }

    fn helper_create_tmpfile_with_extension(ext: Option<String>) -> KcheckResult<PathBuf> {
        let manifest_dir =
            env::var("CARGO_MANIFEST_DIR").expect("Expected to get CARGO_TARGET_DIR");
        let tests_dir = format!("{manifest_dir}/target/tests");

        let rand_string = Alphanumeric.sample_string(&mut rand::thread_rng(), 8);
        std::fs::create_dir_all(tests_dir.clone())?;

        let filename = match ext {
            Some(e) => format!("tmpfile-{rand_string}.{e}"),
            None => format!("tmpfile-{rand_string}"),
        };

        let tmpfile_path = format!("{tests_dir}/{filename}");
        let _ = std::fs::File::create(tmpfile_path.clone())?;

        Ok(PathBuf::from(tmpfile_path))
    }

    fn helper_create_tmpfile() -> KcheckResult<PathBuf> {
        helper_create_tmpfile_with_extension(None)
    }

    fn helper_create_gz_tmpfile(content: &str) -> KcheckResult<PathBuf> {
        let tmpfile_path = helper_create_tmpfile_with_extension(Some("gz".to_string()))?;
        let tmpfile = std::fs::File::create(tmpfile_path.clone())?;

        let mut gz = flate2::write::GzEncoder::new(&tmpfile, flate2::Compression::default());
        gz.write_all(content.as_bytes())?;

        Ok(tmpfile_path)
    }

    fn helper_create_kernel_cfg(options: &[(&str, KconfigState)]) -> KernelConfig {
        KernelConfigBuilder::default()
            .options(options)
            .build()
            .expect("Expected to build a kernel config successfully")
    }

    fn helper_assert_option_state_ok(
        kernel_cfg: &KernelConfig,
        option: &str,
        expected: KconfigState,
        assert_match: AssertMatch,
    ) {
        let result = kernel_cfg
            .option(option)
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
            .option(option)
            .expect_err("Expected to get an option state error");
        assert_eq!(expected, result);
    }

    #[test]
    fn success_option_on() {
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
    fn success_option_off() {
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
    fn success_option_not_set() {
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
    fn success_option_not_found() {
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

        // On a failed option lookup via `option`, `check_option` should return false
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

    #[test]
    fn fail_kernel_config_file_info() {
        let path = PathBuf::from("/path/to/config/does/not/exist");
        let info = KernelConfigFileInfo::try_from_user(path);
        assert!(info.is_err());
    }

    #[test]
    fn success_kernel_config_gz_file_info() {
        let tmpfile_path = helper_create_gz_tmpfile("CONFIG_TEST=y\nCONFIG_TEST_TWO=n")
            .expect("Expected to create a tmpfile");

        let info = KernelConfigFileInfo::try_from_user(tmpfile_path.clone());
        assert!(info.is_ok());

        let cfg = KernelConfigBuilder::default()
            .user(tmpfile_path.clone())
            .build()
            .expect("Expected to create a kernel config from a path");

        assert_eq!(cfg.option("CONFIG_TEST").unwrap(), KconfigState::On);
        assert_eq!(cfg.option("CONFIG_TEST_TWO").unwrap(), KconfigState::Off);
    }

    #[test]
    fn success_kernel_config_builder() {
        let _ = KernelConfigBuilder::default();
    }

    #[test]
    fn success_kernel_config_user() {
        let tmpfile_path = helper_create_tmpfile().expect("Expected to create a tmpfile");

        let cfg = KernelConfigBuilder::default()
            .user(tmpfile_path.clone())
            .build()
            .expect("Expected to create a kernel config from a path");

        assert_eq!(cfg.src, KernelConfigSource::File(tmpfile_path));
    }

    #[test]
    fn fail_kernel_config_user_with_option() {
        let tmpfile_path = helper_create_tmpfile().expect("Expected to create a tmpfile");

        let cfg = KernelConfigBuilder::default()
            .user(tmpfile_path)
            .option("CONFIG_TEST", KconfigState::On)
            .build();

        assert!(cfg.is_err());
    }

    #[test]
    fn fail_kernel_config_user_with_system() {
        let tmpfile_path = helper_create_tmpfile().expect("Expected to create a tmpfile");

        let cfg = KernelConfigBuilder::default()
            .user(tmpfile_path)
            .system()
            .build();

        assert!(cfg.is_err());
    }
}
