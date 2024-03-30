// Copyright (c) 2023 Jake Swensen
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::{
    error::{KcheckError, KcheckResult},
    kconfig::KconfigOption,
};
use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use serde_json;
use std::{ffi::OsStr, path::Path};

const ETC_KCHECK_TOML: &'static str = "/etc/kcheck.toml";
const ETC_KCHECK_JSON: &'static str = "/etc/kcheck.json";

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

/// A structure representing a desired kernel checking configuration.
#[derive(Builder, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[builder(build_fn(error = "KcheckError"))]
pub struct KcheckConfig {
    /// Global `kcheck` config name.
    pub(crate) name: Option<String>,
    /// Global `kcheck` kernel options that have not been grouped into fragments.
    pub(crate) kernel: Option<Vec<KconfigOption>>,
    /// Groups of kernel options that are related.
    pub(crate) fragment: Option<Vec<KcheckConfigFragment>>,
}

impl KcheckConfig {
    /// Generate a single [`KcheckConfig`] object from a collection of config files.
    pub fn generate<P: AsRef<Path>>(files: Vec<P>) -> KcheckResult<Self> {
        // collection of config files and fragments
        let mut collection: Vec<Self> = Vec::new();

        // Known config file locations
        let mut fragments = vec![ETC_KCHECK_TOML.to_owned(), ETC_KCHECK_JSON.to_owned()];

        // Collect all fragments into a single vector
        for item in files {
            let item_path = item.as_ref().to_string_lossy().to_string();

            if item.as_ref().exists() {
                fragments.push(item_path);
            } else {
                return Err(KcheckError::FileDoesNotExist(item_path));
            }
        }

        for fragment in fragments {
            match Self::try_from_file(fragment) {
                Ok(cfg) => collection.push(cfg),
                Err(e) => match e {
                    KcheckError::FileDoesNotExist(_) => continue,
                    _ => return Err(e),
                },
            }
        }

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

    pub fn try_from_file<P: AsRef<Path>>(path: P) -> KcheckResult<Self> {
        let contents = kcheck_utils::file_contents_as_string(path.as_ref())?;

        let cfg: KcheckConfig = match path.as_ref().extension().and_then(OsStr::to_str) {
            Some("toml") => toml::from_str(&contents)?,
            Some("json") => serde_json::from_str(&contents)?,
            Some(f) => return Err(KcheckError::UnknownFileType(f.to_string())),
            None => return Err(KcheckError::MissingFileExtension),
        };

        cfg.validate()?;
        Ok(cfg)
    }

    /// Move all the configuration data from `other` into `self`.
    ///
    /// The resulting [`KcheckConfig`] object will have the global name from
    /// `self`.
    pub fn append(&mut self, other: &mut Self) {
        let new_kernel =
            kcheck_utils::option_vector_append(self.kernel.take(), other.kernel.take());
        self.kernel = new_kernel;

        let new_fragment =
            kcheck_utils::option_vector_append(self.fragment.take(), other.fragment.take());
        self.fragment = new_fragment;
    }

    pub fn validate(&self) -> KcheckResult<()> {
        Ok(())
    }

    /// Add a config fragment to the [`KcheckConfig`] struct.
    pub fn add_fragment(&mut self, fragment: KcheckConfigFragment) {
        match self.fragment.as_mut() {
            Some(f) => f.push(fragment),
            None => self.fragment = Some(vec![fragment]),
        }
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::kconfig::{KconfigOption, KconfigState};
    use lazy_static::lazy_static;
    use std::{fs::File, path::PathBuf};
    use tempfile;

    const TEST_REASON: &str = "Testing";
    const TEST_GLOBAL_NAME: &str = "GLOBAL_TEST";
    const TEST_FRAGMENT_NAME: &str = "TEST_FRAGMENT";
    const TEST_FRAGMENT_NAME_TWO: &str = "TEST_FRAGMENT_TWO";
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

    lazy_static! {
        static ref TEST_FRAGMENT_ON: KconfigOption =
            KconfigOption::new(TEST_FRAGMENT_CONFIG_ON, KconfigState::On);
        static ref TEST_FRAGMENT_OFF: KconfigOption =
            KconfigOption::new(TEST_FRAGMENT_CONFIG_OFF, KconfigState::Off);
        static ref TEST_FRAGMENT_MODULE: KconfigOption =
            KconfigOption::new(TEST_FRAGMENT_CONFIG_MODULE, KconfigState::Module);
        static ref EXPECTED_KCHECK_CONFIG: KcheckConfig = KcheckConfig {
            name: Some(TEST_GLOBAL_NAME.to_string()),
            kernel: None,
            fragment: Some(vec![
                KcheckConfigFragment::new(
                    TEST_FRAGMENT_NAME.to_string(),
                    TEST_REASON.to_owned(),
                    vec![TEST_FRAGMENT_ON.clone(), TEST_FRAGMENT_OFF.clone()]
                ),
                KcheckConfigFragment::new(
                    TEST_FRAGMENT_NAME_TWO.to_string(),
                    TEST_REASON.to_string(),
                    vec![TEST_FRAGMENT_MODULE.clone()]
                )
            ])
        };
    }

    fn run_with_tmpfile<F>(filename: &str, contents: &str, f: F)
    where
        F: FnOnce(PathBuf),
    {
        use std::io::Write;
        let tmpdir = tempfile::tempdir().expect("Failed to create temp dir");

        let file_path = tmpdir.as_ref().join(filename);
        File::create(&file_path)
            .expect("Failed to create temp file")
            .write_all(contents.as_bytes())
            .expect("Failed to write to temp file");

        f(file_path);
    }

    #[test]
    fn success_new_fragment() {
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
    fn success_fragment_is_empty() {
        let test_cfg = KcheckConfigFragment::default();
        assert!(test_cfg.is_empty());
    }

    #[test]
    fn fail_kconfig_builder_no_config() {
        let test_cfg = KcheckConfigBuilder::default().build();
        assert!(matches!(test_cfg, Err(KcheckError::UninitializedField(_))));
    }

    #[test]
    fn success_add_fragment_to_empty() {
        let mut test_cfg = KcheckConfig::default();
        test_cfg.name = Some("test".to_string());
        let test_fragment = KcheckConfigFragment::default();

        test_cfg.add_fragment(test_fragment.clone());
        assert_eq!(test_cfg.fragment, Some(vec![test_fragment]));
    }

    #[test]
    fn success_add_fragment_to_existing() {
        let mut test_cfg = KcheckConfig::default();
        test_cfg.name = Some("test".to_string());
        let existing_fragment = KcheckConfigFragment::new(
            "CONFIG_TEST_OPTION".to_string(),
            "Test".to_string(),
            Vec::new(),
        );

        let test_fragment = KcheckConfigFragment::default();

        test_cfg.add_fragment(existing_fragment.clone());
        test_cfg.add_fragment(test_fragment.clone());
        assert_eq!(
            test_cfg.fragment,
            Some(vec![existing_fragment, test_fragment])
        );
    }

    #[test]
    fn success_kconfig_builder_with_file() {
        run_with_tmpfile("test.toml", EXPECTED_FILE_CONTENTS, |file_path| {
            let cfg =
                KcheckConfig::try_from_file(file_path).expect("Failed to build config from file");
            assert_eq!(cfg, *EXPECTED_KCHECK_CONFIG);
        });
    }
}
