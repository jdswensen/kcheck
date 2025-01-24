// Copyright (c) 2023 Jake Swensen
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::{
    error::{KcheckError, KcheckResult},
    kconfig::{KconfigOption, KconfigState},
    util,
};
use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use serde_json;
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

const ETC_KCHECK_TOML: &str = "/etc/kcheck.toml";
const ETC_KCHECK_JSON: &str = "/etc/kcheck.json";

/// A fragment of a [`KcheckConfig`].
///
/// A fragment represents a collection of config options that are potentially related.
#[derive(Builder, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct KcheckConfigFragment {
    /// Fragment name.
    name: Option<String>,
    /// A short description of the reason fragment options are selected.
    reason: Option<String>,
    /// A list of kernel options that are a part of this fragment.
    kernel: Vec<KconfigOption>,
}

impl KcheckConfigFragment {
    pub fn new(name: String, reason: String, kernel: Vec<KconfigOption>) -> Self {
        KcheckConfigFragment {
            name: Some(name),
            reason: Some(reason),
            kernel,
        }
    }

    /// Check if the fragment is empty.
    pub fn is_empty(&self) -> bool {
        self.name.is_none() && self.reason.is_none() && self.kernel.is_empty()
    }

    /// Fragment name.
    pub fn name(&self) -> Option<String> {
        self.name.clone()
    }

    /// A short description of the reason fragment options are selected.
    pub fn reason(&self) -> Option<String> {
        self.reason.clone()
    }

    /// A list of kernel options that are a part of this fragment.
    pub fn kernel(&self) -> Vec<KconfigOption> {
        self.kernel.clone()
    }
}

#[derive(Builder, Clone, Debug, Default)]
pub struct KcheckConfigBuilder {
    name: Option<String>,
    kernel: Option<Vec<KconfigOption>>,
    fragment: Option<Vec<KcheckConfigFragment>>,
    use_sys_cfg: bool,
    user_cfg_files: Vec<PathBuf>,
}

impl KcheckConfigBuilder {
    /// Assign a name to the global [`KcheckConfig`].
    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    /// Assign a list of kernel options for [`KcheckConfig`].
    pub fn kernel(mut self, kernel: Vec<KconfigOption>) -> Self {
        self.kernel = Some(kernel);
        self
    }

    /// Assign a list of `Kcheck` config fragments for [`KcheckConfig`].
    pub fn fragment(mut self, fragment: Vec<KcheckConfigFragment>) -> Self {
        self.fragment = Some(fragment);
        self
    }

    /// Use system config files to build [`KcheckConfig`].
    pub fn system(mut self) -> Self {
        self.use_sys_cfg = true;
        self
    }

    /// Add a user provided config file to build [`KcheckConfig`].
    pub fn config_files<P: AsRef<Path>>(mut self, files: Vec<P>) -> Self {
        for item in files {
            self.user_cfg_files.push(item.as_ref().to_path_buf());
        }
        self
    }

    /// Build a [`KcheckConfig`] object from the provided configuration.
    pub fn build(self) -> KcheckResult<KcheckConfig> {
        // Collection of config files and fragments
        let mut collection: Vec<KcheckConfig> = Vec::new();

        // Known config file locations
        let mut fragments = if self.use_sys_cfg {
            vec![ETC_KCHECK_TOML.to_owned(), ETC_KCHECK_JSON.to_owned()]
        } else {
            Vec::new()
        };

        // Collect all fragments into a single vector
        for item in self.user_cfg_files {
            let item_path = item.to_string_lossy().to_string();

            if item.exists() {
                fragments.push(item_path);
            } else {
                return Err(KcheckError::FileDoesNotExist(item_path));
            }
        }

        for fragment in fragments {
            match KcheckConfig::try_from_file(fragment) {
                Ok(cfg) => collection.push(cfg),
                Err(e) => match e {
                    KcheckError::FileDoesNotExist(_) => continue,
                    _ => return Err(e),
                },
            }
        }

        // Process API based fragments
        if self.name.is_some() || self.kernel.is_some() || self.fragment.is_some() {
            let mut api_fragment = KcheckConfig::default();
            if let Some(k) = self.kernel {
                api_fragment.kernel = Some(k);
            }

            if let Some(f) = self.fragment {
                api_fragment.fragment = Some(f);
            }

            if let Some(n) = self.name {
                api_fragment.name = Some(n);
            }

            collection.push(api_fragment);
        }

        // Combine all fragments into a single config object
        if !collection.is_empty() {
            // The first element can safely be removed because the collection is not empty
            let mut combined = collection.remove(0);

            for mut item in collection {
                combined.append(&mut item);
            }

            Ok(combined)
        } else {
            Err(KcheckError::NoConfig)
        }
    }
}

/// A structure representing a desired kernel checking configuration.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct KcheckConfig {
    /// Global `kcheck` config name.
    pub(crate) name: Option<String>,
    /// Global `kcheck` kernel options that have not been grouped into fragments.
    pub(crate) kernel: Option<Vec<KconfigOption>>,
    /// Groups of kernel options that are related.
    pub(crate) fragment: Option<Vec<KcheckConfigFragment>>,
}

impl KcheckConfig {
    pub fn try_from_file<P: AsRef<Path>>(path: P) -> KcheckResult<Self> {
        let contents = util::file_contents_as_string(path.as_ref())?;

        let cfg: KcheckConfig = match path.as_ref().extension().and_then(OsStr::to_str) {
            Some("toml") => toml::from_str(&contents)?,
            Some("json") => serde_json::from_str(&contents)?,
            Some(f) => return Err(KcheckError::UnknownFileType(f.to_string())),
            None => return Err(KcheckError::MissingFileExtension),
        };

        Ok(cfg)
    }

    /// Move all the configuration data from `other` into `self`.
    ///
    /// The resulting [`KcheckConfig`] object will have the global name from
    /// `self`.
    pub fn append(&mut self, other: &mut Self) {
        let new_kernel = util::option_vector_append(self.kernel.take(), other.kernel.take());
        self.kernel = new_kernel;

        let new_fragment = util::option_vector_append(self.fragment.take(), other.fragment.take());
        self.fragment = new_fragment;
    }

    /// Returns `true` if the [`KcheckConfig`] is empty.
    ///
    /// An empty [`KcheckConfig`] has no name, kernel options, or fragments.
    pub fn is_empty(&self) -> bool {
        let fragment_is_empty = match &self.fragment {
            Some(f) => f.is_empty(),
            None => true,
        };

        let kernel_is_empty = match &self.kernel {
            Some(k) => k.is_empty(),
            None => true,
        };

        self.name.is_none() && kernel_is_empty && fragment_is_empty
    }
}

impl IntoIterator for KcheckConfig {
    type Item = (String, KconfigState);
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        let mut kernel: Vec<KconfigOption> = match self.kernel {
            Some(k) => k,
            None => Vec::new(),
        };

        let fragments = match self.fragment {
            Some(f) => f.into_iter().flat_map(|f| f.kernel.into_iter()).collect(),
            None => Vec::new(),
        };

        kernel.extend(fragments);
        kernel
            .iter()
            .map(|f| (f.name().clone(), f.state()))
            .collect::<Vec<(String, KconfigState)>>()
            .into_iter()
    }
}

#[cfg(test)]
mod test {
    use std::sync::LazyLock;

    use super::*;
    use crate::kconfig::{KconfigOption, KconfigState};

    const TEST_REASON: &str = "Testing";
    const TEST_GLOBAL_NAME: &str = "GLOBAL_TEST";
    const TEST_FRAGMENT_NAME: &str = "TEST_FRAGMENT";
    const TEST_FRAGMENT_NAME_TWO: &str = "TEST_FRAGMENT_TWO";
    const TEST_FRAGMENT_NAME_THREE: &str = "TEST_FRAGMENT_THREE";
    const TEST_FRAGMENT_CONFIG_ENABLED: &str = "CONFIG_TEST_OPTION_ENABLED";
    const TEST_FRAGMENT_CONFIG_ON: &str = "CONFIG_TEST_OPTION_ON";
    const TEST_FRAGMENT_CONFIG_OFF: &str = "CONFIG_TEST_OPTION_OFF";
    const TEST_FRAGMENT_CONFIG_MODULE: &str = "CONFIG_TEST_OPTION_MODULE";

    const EXPECTED_FILE_CONTENTS: &str = r#"
    name = "GLOBAL_TEST"

    [[fragment]]
    name = "TEST_FRAGMENT"
    reason = "Testing"

    [[fragment.kernel]]
    name = "CONFIG_TEST_OPTION_ON"
    state = "On"

    [[fragment.kernel]]
    name = "CONFIG_TEST_OPTION_OFF"
    state = "Off"

    [[fragment]]
    name = "TEST_FRAGMENT_TWO"
    reason = "Testing"

    [[fragment.kernel]]
    name = "CONFIG_TEST_OPTION_MODULE"
    state = "Module"
    "#;

    const TEST_FILE_CONTENTS_TWO: &str = r#"
    name = "GLOBAL_TEST_TWO"

    [[fragment]]
    name = "TEST_FRAGMENT_THREE"
    reason = "Testing"

    [[fragment.kernel]]
    name = "CONFIG_TEST_OPTION_ENABLED"
    state = "Enabled"
    "#;

    static TEST_FRAGMENT_ON: LazyLock<KconfigOption> =
        LazyLock::new(|| KconfigOption::new(TEST_FRAGMENT_CONFIG_ON, KconfigState::On));

    static TEST_FRAGMENT_ENABLED: LazyLock<KconfigOption> =
        LazyLock::new(|| KconfigOption::new(TEST_FRAGMENT_CONFIG_ENABLED, KconfigState::Enabled));

    static TEST_FRAGMENT_OFF: LazyLock<KconfigOption> =
        LazyLock::new(|| KconfigOption::new(TEST_FRAGMENT_CONFIG_OFF, KconfigState::Off));

    static TEST_FRAGMENT_MODULE: LazyLock<KconfigOption> =
        LazyLock::new(|| KconfigOption::new(TEST_FRAGMENT_CONFIG_MODULE, KconfigState::Module));

    static EXPECTED_KCHECK_CONFIG: LazyLock<KcheckConfig> = LazyLock::new(|| KcheckConfig {
        name: Some(TEST_GLOBAL_NAME.to_string()),
        kernel: None,
        fragment: Some(vec![
            KcheckConfigFragment::new(
                TEST_FRAGMENT_NAME.to_string(),
                TEST_REASON.to_owned(),
                vec![TEST_FRAGMENT_ON.clone(), TEST_FRAGMENT_OFF.clone()],
            ),
            KcheckConfigFragment::new(
                TEST_FRAGMENT_NAME_TWO.to_string(),
                TEST_REASON.to_string(),
                vec![TEST_FRAGMENT_MODULE.clone()],
            ),
        ]),
    });

    static EXPECTED_KCHECK_CONFIG_MULTIPLE_FILES: LazyLock<KcheckConfig> =
        LazyLock::new(|| KcheckConfig {
            name: Some(TEST_GLOBAL_NAME.to_string()),
            kernel: None,
            fragment: Some(vec![
                KcheckConfigFragment::new(
                    TEST_FRAGMENT_NAME.to_string(),
                    TEST_REASON.to_owned(),
                    vec![TEST_FRAGMENT_ON.clone(), TEST_FRAGMENT_OFF.clone()],
                ),
                KcheckConfigFragment::new(
                    TEST_FRAGMENT_NAME_TWO.to_string(),
                    TEST_REASON.to_string(),
                    vec![TEST_FRAGMENT_MODULE.clone()],
                ),
                KcheckConfigFragment::new(
                    TEST_FRAGMENT_NAME_THREE.to_string(),
                    TEST_REASON.to_owned(),
                    vec![TEST_FRAGMENT_ENABLED.clone()],
                ),
            ]),
        });

    #[test]
    fn success_kcheck_config_fragment_new() {
        let test_name = "CONFIG_TEST_OPTION";
        let test_reason = "Test reason";
        let test_kernel_cfg = Vec::new();

        let fragment = KcheckConfigFragment::new(
            test_name.to_string(),
            test_reason.to_string(),
            test_kernel_cfg.clone(),
        );

        assert_eq!(fragment.name(), Some(test_name.to_string()));
        assert_eq!(fragment.reason(), Some(test_reason.to_string()));
        assert_eq!(fragment.kernel(), test_kernel_cfg);
    }

    #[test]
    fn success_kcheck_config_fragment_is_empty() {
        let test_cfg = KcheckConfigFragment::default();
        assert!(test_cfg.is_empty());
    }

    #[test]
    fn success_kcheck_config_builder_multiple_cfg_files() {
        util::run_with_tmpfile(
            "kcheck_cfg_one.toml",
            EXPECTED_FILE_CONTENTS,
            |cfg_one_path| {
                util::run_with_tmpfile(
                    "kcheck_cfg_two.toml",
                    TEST_FILE_CONTENTS_TWO,
                    |cfg_two_path| {
                        let kcheck = KcheckConfigBuilder::default()
                            .config_files(vec![cfg_one_path, cfg_two_path])
                            .build()
                            .expect("Failed to build config");

                        assert_eq!(kcheck, *EXPECTED_KCHECK_CONFIG_MULTIPLE_FILES);
                    },
                );
            },
        );
    }

    #[test]
    fn fail_kcheck_config_builder_no_config() {
        let test_cfg = KcheckConfigBuilder::default().build();
        assert_eq!(test_cfg, Err(KcheckError::NoConfig));
    }

    #[test]
    fn success_kcheck_config_is_empty() {
        let test_cfg = KcheckConfig::default();
        assert!(test_cfg.is_empty());
    }

    #[test]
    fn success_kcheck_config_is_not_empty() {
        let test_cfg = KcheckConfigBuilder::default()
            .name(TEST_GLOBAL_NAME)
            .kernel(vec![TEST_FRAGMENT_ON.clone()])
            .build()
            .expect("Failed to build config");
        assert!(!test_cfg.is_empty());

        let test_cfg = KcheckConfigBuilder::default()
            .name(TEST_GLOBAL_NAME)
            .fragment(vec![KcheckConfigFragment::new(
                TEST_FRAGMENT_NAME.to_string(),
                TEST_REASON.to_string(),
                vec![TEST_FRAGMENT_ON.clone()],
            )])
            .build()
            .expect("Failed to build config");
        assert!(!test_cfg.is_empty());
    }

    #[test]
    fn success_kcheck_config_try_from_file() {
        util::run_with_tmpfile("test.toml", EXPECTED_FILE_CONTENTS, |file_path| {
            let cfg =
                KcheckConfig::try_from_file(file_path).expect("Failed to build config from file");
            assert_eq!(cfg, *EXPECTED_KCHECK_CONFIG);
        });
    }

    #[test]
    fn fail_kcheck_config_try_from_file_does_not_exist() {
        let result = KcheckConfig::try_from_file(PathBuf::from("kcheck-no-exist.toml"));
        assert_eq!(
            result,
            Err(KcheckError::FileDoesNotExist(
                "kcheck-no-exist.toml".to_string()
            ))
        );
    }

    #[test]
    fn fail_kcheck_config_try_from_file_missing_extension() {
        util::run_with_tmpfile("kcheck-missing-ext", EXPECTED_FILE_CONTENTS, |file_path| {
            let result = KcheckConfig::try_from_file(file_path);
            assert_eq!(result, Err(KcheckError::MissingFileExtension));
        });
    }

    #[test]
    fn fail_kcheck_config_try_from_file_unknown_extension() {
        util::run_with_tmpfile(
            "kcheck-missing.unknown",
            EXPECTED_FILE_CONTENTS,
            |file_path| {
                let result = KcheckConfig::try_from_file(file_path);
                assert_eq!(
                    result,
                    Err(KcheckError::UnknownFileType("unknown".to_string()))
                );
            },
        );
    }
}
