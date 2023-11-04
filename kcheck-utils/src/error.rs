// Copyright (c) 2023 Jake Swensen
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use thiserror::Error;

pub type KcheckResult<T> = Result<T, KcheckError>;

#[derive(Debug, Error, PartialEq)]
pub enum KcheckError {
    #[error("Duplicate config found: {0}")]
    DuplicateConfig(String),
    #[error("File does not exist: {0}")]
    FileDoesNotExist(String),
    #[error("File is not a valid: {0}")]
    InvalidFile(String),
    #[error("IO Error: {0}")]
    IoError(String),
    #[error("Error parsing json file: {0}")]
    JsonParseError(String),
    #[error("Kernel config not found")]
    KernelConfigNotFound,
    #[error("Kernel config parse error")]
    KernelConfigParseError,
    #[error("No file extension found")]
    MissingFileExtension,
    #[error("Could not find a config file")]
    NoConfig,
    #[error("Error parsing toml file: {0}")]
    TomlParseError(#[from] toml::de::Error),
    #[error("Unknown file type: {0}")]
    UnknownFileType(String),
    #[error("Unknown kernel config option: {0}")]
    UnknownKernelConfigOption(String),
}

impl From<std::io::Error> for KcheckError {
    fn from(e: std::io::Error) -> Self {
        KcheckError::IoError(e.to_string())
    }
}

impl From<serde_json::Error> for KcheckError {
    fn from(e: serde_json::Error) -> Self {
        KcheckError::JsonParseError(e.to_string())
    }
}
