// Copyright (c) 2023 Jake Swensen
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use thiserror::Error;

pub type KcheckResult<T> = Result<T, KcheckError>;

#[derive(Debug, Error)]
pub enum KcheckError {
    #[error("File does not exist: {0}")]
    FileDoesNotExist(String),
    #[error("File is not a valid: {0}")]
    InvalidFile(String),
    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Error parsing json file: {0}")]
    JsonParseError(#[from] serde_json::Error),
    #[error("Kernel config not found")]
    KernelConfigNotFound,
    #[error("No file extension found")]
    MissingFileExtension,
    #[error("Could not find a config file")]
    NoConfig,
    #[error("Error parsing toml file: {0}")]
    TomlParseError(#[from] toml::de::Error),
    #[error("Unknown file type: {0}")]
    UnknownFileType(String),
}
