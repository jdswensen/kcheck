// Copyright (c) 2023 Jake Swensen
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use crate::error::{KcheckError, KcheckResult};
use flate2::read::GzDecoder;
use std::{io::Read, path::Path};

/// Create a temporary file with the given contents and run a function with the file path.
#[cfg(test)]
pub(crate) fn run_with_tmpfile<F>(filename: &str, contents: &str, f: F)
where
    F: FnOnce(std::path::PathBuf),
{
    use std::io::Write;
    let tmpdir = tempfile::tempdir().expect("Failed to create temp dir");

    let file_path = tmpdir.as_ref().join(filename);
    std::fs::File::create(&file_path)
        .expect("Failed to create temp file")
        .write_all(contents.as_bytes())
        .expect("Failed to write to temp file");

    f(file_path);
}

/// Inflate a gzip'd file into a string.
pub fn inflate_gzip_file<P: AsRef<Path>>(path: P) -> KcheckResult<String> {
    let contents = file_contents_as_bytes(path)?;
    let mut gz = GzDecoder::new(&contents[..]);
    let mut s = String::new();
    gz.read_to_string(&mut s)?;
    Ok(s)
}

/// Open a file.
///
/// Function that provides basic file opening and error handling.
pub fn open_file<P: AsRef<Path>>(path: P) -> KcheckResult<std::fs::File> {
    if !path.as_ref().exists() {
        let path_string: String = path.as_ref().to_string_lossy().to_string();
        return Err(KcheckError::FileDoesNotExist(path_string));
    }

    let file = std::fs::File::open(path)?;
    Ok(file)
}

/// Parse file contents into a vector of bytes.
pub fn file_contents_as_bytes<P: AsRef<Path>>(path: P) -> KcheckResult<Vec<u8>> {
    let mut file = open_file(path)?;
    let mut contents = Vec::<u8>::new();
    file.read_to_end(&mut contents)?;
    Ok(contents)
}

/// Parse file contents into a string.
pub fn file_contents_as_string<P: AsRef<Path>>(path: P) -> KcheckResult<String> {
    let mut file = open_file(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

/// Take two `Option<Vec<T>>` and append the second to the first.
///
/// Returns the resulting `Option<Vec<T>>`.
/// todo: this could probably be written in a more ergonomic way
pub fn option_vector_append<T>(
    mut orig: Option<Vec<T>>,
    mut other: Option<Vec<T>>,
) -> Option<Vec<T>> {
    if other.is_some() {
        if orig.is_none() {
            // The orginal vector is empty, so just take the other vector
            orig = other.take();
        } else {
            // At this point, both orig and other are `Some`
            let mut new_orig = orig.take().unwrap();
            let mut new_other = other.take().unwrap();
            new_orig.append(&mut new_other);
            orig = Some(new_orig);
        }
    }

    orig
}
