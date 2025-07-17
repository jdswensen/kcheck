[![Crates.io][crates-badge]][crates-url]
[![MPL 2.0][mpl-badge]][mpl-url]
[![Build Status][actions-badge]][actions-url]

[crates-badge]: https://img.shields.io/crates/v/kcheck.svg
[crates-url]: https://crates.io/crates/kcheck
[mpl-badge]: https://img.shields.io/badge/License-MPL_2.0-brightgreen.svg
[mpl-url]: https://github.com/jdswensen/kcheck/blob/main/LICENSE
[actions-badge]: https://img.shields.io/github/actions/workflow/status/jdswensen/kcheck/ci.yml?branch=main
[actions-url]: https://github.com/jdswensen/kcheck/actions/workflows/ci.yml?query=workflow%3ACI+branch%3Amain

# kcheck

A library for checking system configurations.

## Motivation

When writing low-level software applications there can be contraints placed on a developer based on
system configuration options. These options could be kernel configuration or runtime sysctl settings.

Full desktop kernel distributions will often come with a ton of settings already ready to go. However,
minimal config systems such as custom embedded Linux kernels or some server configs may not have a
required feature enabled.

In these unknown system configurations, it would be helpful to both developers and users to know why
an application may not be working properly. The goal of this software is to develop a library that
is capable of parsing both build time and runtime system configuration information to help determine
where a system may be misconfigured.

## Installation

Add `kcheck` to your `Cargo.toml`

```
[dependencies]
kcheck = "0.2"
```

Optionally, install the command line tool.
```
cargo install kcheck-cli
```

## Features

- [x] Parse Kconfig files and fragments from a build system (does not need to be on the target system) and compare it to a provided configuration file
- [x] Parse a running Linux kernel config (if one exists on the system) and compare it to a provided configuration file
- [x] Optionally, utilize the `kcheck` library to develop app defined configuration checks
- [x] Parse a desired kernel config from config fragments located in a specific location
- [ ] Parse kernel runtime parameters via `sysctl`
- [ ] Compare a desired configuration to the running Linux kernel config at boot
- [ ] Generate Linux kernel config fragments from `kcheck` config fragments
- [ ] Generate `kcheck` config fragements from Linux kernel config fragments

## Configuration File Format

`kcheck` configuration files can be written in either JSON or TOML, but TOML files
are probably easier to read. Each config file can contain one or more fragments and each fragment
has a `name` and a `reason`. These are mostly to help with printing helpful messages if a configuration
fails. `name` is required, `reason` is optional.

After the initial fragment definition, the file can contain kernel configuration fragments which
require a `name` and a `state`. The `name` is the name of the variable as it shows up in the `Kconfig`
output. The `state` is an expansion of the `Kconfig` tri-state system. An application might not care
if the setting is `On` or a `Module`, only that it is `Enabled`. Alternatively, a security conscious
application may want to ensure that no modules have been configured.

```
[[fragment]]
name = "usb-serial"
reason = "Serial USB support"

[[fragment.kernel]]
name = "CONFIG_USB_ACM"
state = "On"

[[fragment.kernel]]
name = "CONFIG_USB_SERIAL"
state = "Module"
```

## Usage

Once a configuration file is defined, it can then be used as input into `kcheck` to check against a
system configuration. For example, the `kcheck-cli` command can be used to check the example serial
configuration fragment against a running kernel:

```
kcheck-cli -c ./kcheck-serial.toml

+-------------------+---------------+--------------+--------+
| Config Option     | Desired State | Kernel State | Result |
+-------------------+---------------+--------------+--------+
| CONFIG_USB_ACM    | On            | Module       | Fail   |
+-------------------+---------------+--------------+--------+
| CONFIG_USB_SERIAL | Module        | Module       | Pass   |
+-------------------+---------------+--------------+--------+
```

It can also be used to check a specific non-running kernel config:

```
kcheck-cli -k /boot/config-5.15.0-143-generic -c ./kcheck-serial.toml

+-------------------+---------------+--------------+--------+
| Config Option     | Desired State | Kernel State | Result |
+-------------------+---------------+--------------+--------+
| CONFIG_USB_ACM    | On            | Module       | Fail   |
+-------------------+---------------+--------------+--------+
| CONFIG_USB_SERIAL | Module        | Module       | Pass   |
+-------------------+---------------+--------------+--------+
```

See the [examples](examples) folder for additional examples of how to use the
`kcheck` library in an application directly.

## License

Licensed under the [Mozilla Public License Version 2.0](https://www.mozilla.org/en-US/MPL/2.0/).
