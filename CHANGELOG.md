# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.10.3] - 2026-02-16

### Fork: neocmakelsp-fast

This is a performance-focused fork of [neocmakelsp](https://github.com/neocmakelsp/neocmakelsp).

### Added

- **Path completion system** - Smart path completions for CMake commands:
  - `add_subdirectory()` - shows directories with CMakeLists.txt prioritized (✓ marker)
  - `add_executable()`, `add_library()`, `target_sources()` - shows source files (.c, .cpp, .h, etc.)
  - `include()` - shows .cmake files
  - `file()`, `configure_file()`, `install()` - shows all files
  - Automatic detection of path-like input (starts with `.`, `/`, `~`, or contains `/`)

- **Directory scanning with caching** - New `scanner` module:
  - `DirectoryCache` with TTL-based expiration (5 seconds default)
  - LRU eviction when cache exceeds 100 entries
  - Parallel directory scanning using `ignore` crate (respects .gitignore)
  - Pre-built scan options for different contexts (subdirectories, source files, includes)

- **File watcher for cache invalidation**:
  - Watches workspace directories for file system changes
  - Automatically invalidates cache on file create/delete/rename
  - Watches common CMake directories: src, include, lib, cmake, tests, modules

- **Signature help for CMake commands**:
  - Shows function signatures from `cmake --help-commands`
  - Displays parameter information and documentation
  - Highlights active parameter based on cursor position

### Changed

- Renamed binary and package to `neocmakelsp-fast`
- Updated dependencies: added `notify`, `num_cpus`, `tempfile`, `ignore`
- Extended `PositionType` enum with `SourceFile`, `AnyFile`, `Directory` variants
- Improved collapsible if statements per clippy recommendations

### Fixed

- Various clippy warnings (unused imports, dead code, collapsible conditions)
- TOML formatting (alphabetical order of dependencies)

[0.10.3]: https://github.com/NikitolProject/neocmakelsp-fast/compare/v0.10.0...v0.10.3

---

## Upstream Changelog (neocmakelsp)

## [0.10.0] - 2026-01-26

### Changed

- Same with 0.9.2, but because breaking changes in 0.9.2, so we need to bump a big version
- Set snippet default to false, since it is not good enough
- Change the config fields by @idealseal (it was a Breaking change, you can find details in https://github.com/neocmakelsp/neocmakelsp/pull/240)
- Switch to using etcetera for choosing config directory. This will fix the wrong directory on macos and windows

[0.10.0]: https://github.com/neocmakelsp/neocmakelsp/compare/v0.9.1...v0.10.0


## [0.9.2] - 2026-01-26

### Changed

- Set snippet default to false, since it is not good enough
- Change the config fields by @idealseal (it was a Breaking change, you can find details in https://github.com/neocmakelsp/neocmakelsp/pull/240)
- Switch to using etcetera for choosing config directory. This will fix the wrong directory on macos and windows

[0.9.2]: https://github.com/neocmakelsp/neocmakelsp/compare/v0.9.1...v0.9.2

## [0.9.1] - 2026-01-08

### Changed

- Allow all field be empty in config file
- Use dashmap to cache documents in backend
- Harden CI workflows
- Fix #213

### Fixed

- Explicitly set binary name for completion script
- Format within the biggest source range
- Fix small typos in readme

[0.9.1]: https://github.com/neocmakelsp/neocmakelsp/compare/v0.9.0...v0.9.1

## 0.9.0

- Fix: argument list did not have completiontions
- Feat: switch to rust specific release actions
- Feat: update MSRV to 1.90
- Breaking changes: add toml formatting rules

## 0.8.30

- This version use the forked lsp-types, bump the dependence of
  fluent-uri. This release contains a lot of experiment things, like using
  the async trait of rust instead of that of async-trait, like using
  fluent-uri instead of the crate of url.

- This release also is used to
  test the pr for fluent-uri. I believe the fluent-uri is better than that
  url, and also make a pr, but the author these days is very busy, and
  does have time to review my pr, so I added my modification to my forked
  lsp-types which is named as lsp-types-f. If there is any problem, please
  open an issue for me, I will try to fix it

## 0.8.20-beta4
- fix that every time save a file, the references will increase, which also cause problems when doing rename

## 0.8.20-beta3
- fix when using relative cmake path, reference not work properly

## 0.8.20-beta2
- fix rename do not work for include
- fix rename not work if position is on definition

## 0.8.20-beta1
- support real reference
- support rename
- to edition 2024

## 0.8.8
- futures-util v0.3.30 is yanked, so publish new release

## 0.8.7
- Fix complete when meet comment panic on windows
- Better way to find the platform prefix thanks to @idealseal
- improve logging for stdio transport @idealseal
- rename buildin to builtin, typo fix
- bring the cli color of clap
- add LTO support by @zamazan4ik

## 0.8.5
- Add a lot of unit tests
- Fix that fileapi cache data cannot be updated.
- Realize the lsp document_link
- Make the hovered information the same as completion information
- Support completing with cmake space.
- Change the way generate the snippet
- Now the `insert_final_newline` action will work.
- Fix the meson cargo wrapper again. I think this time it is usable now.
- Tidy up a lot of code.
- Now it can jump to `"${SOME_VARIABLE}/some.cmake"` or `"some.cmake"`. It supports to read the variable.
- Adjust some document format

Full changes: https://github.com/neocmakelsp/neocmakelsp/compare/v0.8.4...v0.8.5

## 0.8.4
- Fix jump to buildin cmake file still not works on temux
- Try to support find_package on MSYSTEM
- Add some unit test. Now it is 30% coverage!
- Now hover and complete will show the comment of cmake

## 0.8.3
- support reading value from fileapi and use it in completing
- fix jumping to buildin cmake file not works on temux
- fix meson build, induce a python wrapper

## 0.8.1

- Compatible with vcpkg package manager

## 0.8.0

- support file api
- use lazylock
- support jump from function to files

## 0.7.6

- feat: Update CompletionItem to meet the requirements of the LSP specification, by yangyingchao
- add completiontions for fish, bash and etc
- Use derive for subcommand

## 0.7.5

- fix panic when meet pkg_check_modules thanks to @yangyingchao
- better performance, reduce too many collect
- fix too much typo
