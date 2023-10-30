// Copyright (c) 2023 Jake Swensen
// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! A library for working with kernel config information.
//!
//! Works with:
//! - Systems that utilize Kconfig (Linux, Zephyr)
//!
//! Problem statement:
//! - Software may run on unknown system configurations
//! - Software may require specific kernel config options to be enabled
//! - Software may be run on a minimal config system
//! - User wants to understand the reasons behind a kernel config setting
//! - User wants to be able to check the state of kernel config options.
//! - User may want to enforce runtime checks on kernel config options.
//!
//! todo: derive readme from doc comments

pub mod config;
pub mod error;
pub mod kconfig;
pub mod kernel;
