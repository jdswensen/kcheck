// Copyright (c) 2023 Jake Swensen
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::error::{KcheckError, KcheckResult};
use crate::kconfig::KconfigOption;
use serde::{Deserialize, Serialize};
use serde_json;
use std::ffi::OsStr;
use std::path::Path;

/// A fragment of a `kcheck` config file.
///
/// A fragment represents a collection of config options that are potentially related.
///
/// todo: custom deserialize, serialize
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct KcheckConfigFragment {
    /// Fragment name.
    name: Option<String>,
    /// A short description of the reason fragment options are selected.
    reason: Option<String>,
    /// A list of kernel options that are a part of this fragment.
    kernel: Vec<KconfigOption>,
}

impl KcheckConfigFragment {
    pub fn new(name: Option<String>, reason: Option<String>, kernel: Vec<KconfigOption>) -> Self {
        KcheckConfigFragment {
            name,
            reason,
            kernel,
        }
    }

    /// Check if the fragment is empty.
    pub fn is_empty(&self) -> bool {
        self.name.is_none() && self.reason.is_none() && self.kernel.is_empty()
    }
}

/// A structure representing a desired kernel checking configuration.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct KcheckConfig {
    /// Global `kcheck` config name.
    name: Option<String>,
    /// Global `kcheck` kernel options that have not been grouped into fragments.
    kernel: Option<Vec<KconfigOption>>,
    /// Groups of kernel options that are related.
    fragment: Option<Vec<KcheckConfigFragment>>,
}

impl KcheckConfig {
    /// Generate a single `KcheckConfig` object from a collection of config files.
    pub fn generate<P: AsRef<Path>>(files: Vec<P>) -> KcheckResult<Self> {
        // collection of config files and fragments
        let mut collection: Vec<Self> = Vec::new();

        // Known config file locations
        let mut fragments = vec![
            "/etc/kcheck.toml".to_string(),
            "/etc/kcheck.json".to_string(),
        ];

        // Collect all fragments into a single vector
        for item in files {
            fragments.push(item.as_ref().to_string_lossy().to_string());
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
    /// The resulting `KcheckConfig` object will have the global name from
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

    fn add_fragment(&mut self, fragment: KcheckConfigFragment) {
        todo!()
    }

    /// Check if the config is empty.
    fn is_empty(&self) -> bool {
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
    type Item = (KconfigOption, String);
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        // return (KconfigOption, "reason")
        todo!()
    }
}
