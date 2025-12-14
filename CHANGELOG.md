# Mecomp Changelog

## v0.6.2

### v0.6.2 What's Changed
* fix(storage/tests): analysis migration by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/429
* Update surrql! macro documentation with practical usage examples by @Copilot in https://github.com/AnthonyMichaelTDM/mecomp/pull/431
* feat: surrql macro for compile-time query validation by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/430
* fix analysis migrations and improve error reporting by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/432
* feat(cli): make pipe commands implicit, and simplify interface by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/433

**Full Changelog**: https://github.com/AnthonyMichaelTDM/mecomp/compare/v0.6.1...v0.6.2

## v0.6.1

### v0.6.1 What's Changed

* fix(storage): clear analysis_to_song relations table during migration to prevent dangling relations by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/427

**Full Changelog**: https://github.com/AnthonyMichaelTDM/mecomp/compare/v0.6.0...v0.6.1

## v0.6.0

> [!Attention]
> This release introduces breaking changes to both the RPC interface and the internal audio analysis features.
> Clients interacting with the daemon must be updated to accommodate these changes.
> For users, your song analysis data will be reset when the updated daemon starts for the first time. To regenerate your analysis features and restore radio and collection functionality, run `mecomp-cli analyze`.

### v0.6.0 What's Changed
* fix failing test and optimize display of song attributes by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/410
* chore: update dependencies by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/411
* feat: migrate from tarpc to tonic gRPC by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/414
* chore(deps): bump surrealdb from 2.3.10 to 2.4.0 by @dependabot[bot] in https://github.com/AnthonyMichaelTDM/mecomp/pull/413
* chore(deps): bump crate-ci/typos from 1.38.1 to 1.39.0 by @dependabot[bot] in https://github.com/AnthonyMichaelTDM/mecomp/pull/407
* chore(deps): bump github/codeql-action from 4.30.8 to 4.31.2 by @dependabot[bot] in https://github.com/AnthonyMichaelTDM/mecomp/pull/408
* chore: update cargo-dist version to 0.30.2 in configuration by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/417
* perf(analysis): Branch prediction hints, and improved ndarray usage by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/416
* feat: add commit-msg hook to verify release builds by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/424
* fix(general): build issues related to new release by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/425
* feat(surrealqlx): implement migration handling by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/427
* feat(analysis): port over changes to chroma from bliss-rs by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/426

**Full Changelog**: https://github.com/AnthonyMichaelTDM/mecomp/compare/v0.5.9...v0.6.0

## v0.5.9

### v0.5.9 What's Changed

* chore: change Dependabot update schedule from daily to weekly by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/400
* feat: replace tap dependency with inspect_err for error logging by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/402
* feat: add QueueChanged state to audio events and update handling by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/403


**Full Changelog**: https://github.com/AnthonyMichaelTDM/mecomp/compare/v0.5.8...v0.5.9

## v0.5.8

### v0.5.8 What's Changed

* fix(daemon): panic in terminator thread by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/395
* feat: add cargo-hakari cleanup step to release workflow by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/396
* reduce size of release binaries by removing workspace-hack before building by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/397
    * this ended up not doing much, which sucks because I spent quite a while on it :(
* perf(analysis): parallelize reference set generation by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/398
* feat: remove support for x86_64-apple-darwin by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/399


**Full Changelog**: https://github.com/AnthonyMichaelTDM/mecomp/compare/v0.5.7...v0.5.8

## v0.5.7

### v0.5.7 What's Changed

* chore(deps): bump taiki-e/install-action from 2.62.21 to 2.62.29 by @dependabot[bot] in https://github.com/AnthonyMichaelTDM/mecomp/pull/380
* chore(deps): bump github/codeql-action from 3.30.6 to 4.30.8 by @dependabot[bot] in https://github.com/AnthonyMichaelTDM/mecomp/pull/374
* chore(deps): bump crate-ci/typos from 1.37.2 to 1.38.1 by @dependabot[bot] in https://github.com/AnthonyMichaelTDM/mecomp/pull/372
* chore(deps): bump softprops/action-gh-release from 2.3.4 to 2.4.1 by @dependabot[bot] in https://github.com/AnthonyMichaelTDM/mecomp/pull/377
* fix(tui): handle "_ in progress" errors for certain actions by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/382
* fix(tui): properly handle delete key for input-box by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/383
* feat(dynamic updates): use locks for the dynamic updates to hopefully… by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/386
* feat(tui): add a refresh key  by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/387
* Some cleanup + optimization by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/393
* Less state synchro by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/394


**Full Changelog**: https://github.com/AnthonyMichaelTDM/mecomp/compare/v0.5.6...v0.5.7

## v0.5.6

### v0.5.6 What's Changed

* Udp improvements, and clippy by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/352
* pin dependencies by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/353
* chore(deps): bump codecov/codecov-action from 5.4.0 to 5.4.2 by @dependabot[bot] in https://github.com/AnthonyMichaelTDM/mecomp/pull/276
* feat(tui): default to "connecting with retry" behavior when initializing client by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/362
* feat: update to surrealdb v2.3.10 by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/363
* chore(deps): bump taiki-e/install-action from 2.62.13 to 2.62.16 by @dependabot[bot] in https://github.com/AnthonyMichaelTDM/mecomp/pull/357
* chore(deps): bump github/codeql-action from 3.30.5 to 3.30.6 by @dependabot[bot] in https://github.com/AnthonyMichaelTDM/mecomp/pull/358
* chore(deps): bump crate-ci/typos from 1.36.3 to 1.37.2 by @dependabot[bot] in https://github.com/AnthonyMichaelTDM/mecomp/pull/361
* feat(analysis): migrate to linfa 0.8.0, and improve clustering performance by @AnthonyMichaelTDM in https://github.com/AnthonyMichaelTDM/mecomp/pull/365
* chore(deps): bump softprops/action-gh-release from 2.3.3 to 2.3.4 by @dependabot[bot] in https://github.com/AnthonyMichaelTDM/mecomp/pull/360

**Full Changelog**: https://github.com/AnthonyMichaelTDM/mecomp/compare/v0.5.5...v0.5.6

## v0.5.5

### v0.5.5 What's Changed

this release fixes an issue with the `mecomp-storage` crate not compiling when the `db` feature wasn't enabled.

* fix(storage): compilation error when db feature not enabled by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/346>

### **Full Changelog**: <https://github.com/AnthonyMichaelTDM/mecomp/compare/v0.5.4...v0.5.5>

## v0.5.4

### v0.5.4 What's Changed

This release is mostly refactoring and updates to dependencies, but it also includes some new features and bug fixes.

* feat(tui): overhaul click handling for checktree's by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/338>
* migrate to criterion 0.7.0 by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/339>
* refactor: box OneOrMany::One variant, and other refactoring by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/340>
* feat: migrate to rodio v0.21.1 by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/341>
* feat: migrate to surrealdb v2.3.7 by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/344>

### **Full Changelog**: <https://github.com/AnthonyMichaelTDM/mecomp/compare/v0.5.3...v0.5.4>

## v0.5.3

### v0.5.3: What's Changed

Aside from some minor bugfixes, the main feature of this release is that the queue now persists across shutdowns

* fix(daemon): issue where interrupt receiver could lag by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/333>
* feat(daemon): persist queue state across shutdowns by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/332>
* feat(tui): update play/pause symbols by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/335>
* fix(tui): adjust mouse hitboxes for misaligned components by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/337>

### **Full Changelog**: <https://github.com/AnthonyMichaelTDM/mecomp/compare/v0.5.2...v0.5.3>

## v0.5.2

### v0.5.2: What's Changed

* feat(tui): make color theme configurable by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/329>
* chore: add example Zellij layout and pywal template for mecomp by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/330>

### **Full Changelog**: <https://github.com/AnthonyMichaelTDM/mecomp/compare/v0.5.1...v0.5.2>

## v0.5.1

### v0.5.1: What's Changed

v0.5.0 had a bug that caused some packages to fail to be deployed to crates.io, this release fixes that issue and another one I found on the way

* hotfix(storage): fix failing release by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/326>
* fix(storage): `merge_with_song` was skipping the album field by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/327>

### **Full Changelog**: <https://github.com/AnthonyMichaelTDM/mecomp/compare/v0.5.0...v0.5.1>

## v0.5.0

### v0.5.0: What's Changed

* refactor(backup): change file existence check by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/317>
* feat(rpc): use usize instead of i64 for search limit by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/319>
* feat(daemon): improve random read endpoints by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/321>
* feat: refactor state management to use LibraryBrief instead of LibraryFull in many places by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/322>
* refactor: simplify assertions and improve readability in tests by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/323>
* refactor(daemon): remove `analysis` feature flag by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/324>
* perf(analysis): improve cache locality in chroma_stft by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/325>

### **Full Changelog**: <https://github.com/AnthonyMichaelTDM/mecomp/compare/v0.4.1...v0.5.0>

## v0.4.1

### v0.4.1: What's Changed

* feat(surrealqlx): better support for custom queries, and split index definition into separate macro by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/314>

### **Full Changelog**: <https://github.com/AnthonyMichaelTDM/mecomp/compare/v0.4.0...v0.4.1>

## v0.4.0

### v0.4.0: What's Changed

* fix(daemon): bug where daemon would hang if it failed to start the tc… by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/297>
* refactor(core): overhaul duration tracking by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/298>
* refactor(storage/analysis): streamline song analysis queries by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/303>
* feat(daemon): improve graceful shutdown by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/304>
* fix(storage/tui): tui could crash if a playlist has duplicate songs by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/306>
* feat(daemon): optimize future spawning and improve event handling by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/307>
* Tokio console by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/309>
* refactor(audio): simplify AudioKernel structure and command handling by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/310>
* feat(storage): use computed fields in favor of calling repair all the time by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/311>
* feat(cli): implement tab-completion for ids by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/312>

### **Full Changelog**: <https://github.com/AnthonyMichaelTDM/mecomp/compare/v0.3.4...v0.4.0>

## v0.3.4

### v0.3.4: What's Changed

* feat(daemon): Add import/export functionality for playlists and dynamic playlists by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/295>

### **Full Changelog**: <https://github.com/AnthonyMichaelTDM/mecomp/compare/v0.3.3...v0.3.4>

## v0.3.3

### v0.3.3: What's Changed

* feat(analysis): expose more projection methods and add PCA by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/292>
* refactor(mock_playback loop): speed up audio-related tests by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/291>
* refactor(core/duration_watcher): use std threads instead of tokio by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/294>

### **Full Changelog**: <https://github.com/AnthonyMichaelTDM/mecomp/compare/v0.3.2...v0.3.3>

## v0.3.2

### v0.3.2: What's Changed

* feat(one-or-many): enhance deserialization by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/287>
* feat(core): add support for setting list of protected artist names by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/288>
* feat(tui): ensure selection remains in view when sort mode changed by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/289>
* Migrate to edition 2024 by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/290>

### **Full Changelog**: <https://github.com/AnthonyMichaelTDM/mecomp/compare/v0.3.1...v0.3.2>

## v0.3.1

### v0.3.1: What's Changed

* fix(daemon): add missing command attributes to cli `Flags` struct, enabling --version flag by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/282>
* feat(one-or-many): Implement the Extend trait by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/283>
* fix(radio): properly handle an empty input for radio endpoints by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/284>

### **Full Changelog**: <https://github.com/AnthonyMichaelTDM/mecomp/compare/v0.3.0...v0.3.1>

## v0.3.0

### v0.3.0: What's Changed

* refactor(logging): improve file path processing for logging by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/269>
* feat(daemon): implement graceful handling of shutdown signals by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/270>
* feat(binaries): shell completions by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/277>
* feat(cli): improve output formatting, completion hints, input validation, and implement support for a --quiet flag (for some commands) to print only the IDs (good for piping to a new playlist, radio, etc.) by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/279>
* fix(tests): fix flaky test by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/281>
* misc(deps): surrealdb v2.2.2 by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/280>

### **Full Changelog**: <https://github.com/AnthonyMichaelTDM/mecomp/compare/v0.2.3...v0.3.0>

## v0.2.3

### v0.2.3: What's Changed

* fix(storage): migrate to surrealdb 2.2.1 by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/256>
* feat: refactor main functions for improved performance by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/258>
* feat(storage): define tables for each relation we use by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/259>
* feat(analysis): improve decoder by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/260>
* Feat(cli): optionally override analysis by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/261>
* Reorganize benchmarks and expand benchmark suite by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/263>
* Analysis optimizations by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/268>

### **Full Changelog**: <https://github.com/AnthonyMichaelTDM/mecomp/compare/v0.2.2...v0.2.3>

## v0.2.2

### v0.2.2: What's Changed

* HOTFIX: pin surrealdb to v2.2.0 by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/254>

### **Full Changelog**: <https://github.com/AnthonyMichaelTDM/mecomp/compare/v0.2.1...v0.2.2>

## v0.2.1

### v0.2.1: What's Changed

This release is mostly bug fixes, there was an issue with my implementation of the gap statistic which has been fixed, and a regression with surrealDB that was addressed.
I also moved configuration management into mecomp-core, so that frontends like the tui can have access configs for their own uses

* feat(core,tui): Centralize settings management in mecomp_core and make radio size configurable for TUI by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/245>
* fix(analysis): correct error with gap calculation by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/248>
* refactor(tui): simplify logic and reduce some duplication by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/251>
* fix(storage): fix panic on defining analyzer by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/252>

### **Full Changelog**: <https://github.com/AnthonyMichaelTDM/mecomp/compare/v0.2.0...v0.2.1>

## v0.2.0

### v0.2.0: What's Changed

This release sees the addition of a new frontend (mecomp-mpris), an overhaul of how player state is communicated to clients (using UDP), and finally implements Dynamic Playlists, as well as various bug fixes.

This is a minor version bump because there have been breaking changes to both the RPC interface and how commands are handled by the audio kernel (in order to better align the daemon with the MPRIS2 specification).

* feat(tui): add random view and integrate into sidebar and state management by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/202>
* feat(testing): convert cli smoke tests to snapshot tests by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/203>
* chore: update dependencies by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/204>
* feat(daemon): Implement pub sub functionality to enable client notifications by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/206>
* refactor(daemon): use UDP for client notifications by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/207>
* feat(core/udp): improved performance and scalability by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/210>
* feat(tui): support nested popups by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/217>
* feat(storage) define internal representation of dynamic playlists and implement compilation/parsing by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/218>
* feat(daemon): expose endpoints for dynamic playlists  by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/219>
* chore(deps): bump codecov/codecov-action from 5.1.2 to 5.3.1 by @dependabot in <https://github.com/AnthonyMichaelTDM/mecomp/pull/209>
* feat(cli): integrate dynamic playlist endpoints to cli by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/222>
* feat(tui): implement query builder for dynamic playlists by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/225>
* bugfix(tui): clicking on an empty region of a checklist when an item is selected will open that item by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/227>
* feat(rpc): add method to get a song by its file path by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/235>
* fix(tui): bug where checked items weren't always cleared acros view transitions by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/238>
* feat(daemon): implement playback stop endpoint by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/239>
* feat(udp): publish state changes over UDP and use those in clients to maintain internal state by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/240>
* feat(mpris): Implement MPRIS compatibility layer with Root and Player interfaces supported  by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/241>
* feat(db): improve full text search analyzer with additional filters by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/242>
* feat: preparation for v0.2.0 release by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/243>

### **Full Changelog**: <https://github.com/AnthonyMichaelTDM/mecomp/compare/v0.1.3...v0.2.0>

## v0.1.3

### v0.1.3: What's Changed

* refactor(daemon): remove redundant and unused endpoints by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/190>
* feat(logging): migrate to envlogger 11.5 by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/192>
* feat: improve endpoint test coverage and correct config deserialization issue by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/198>
* chore(deps): bump codecov/codecov-action from 5.1.1 to 5.1.2 by @dependabot in <https://github.com/AnthonyMichaelTDM/mecomp/pull/195>
* chore(deps): bump actions/upload-artifact from 4.4.3 to 4.6.0 by @dependabot in <https://github.com/AnthonyMichaelTDM/mecomp/pull/197>

### **Full Changelog**: <https://github.com/AnthonyMichaelTDM/mecomp/compare/v0.1.2...v0.1.3>

## v0.1.2

### v0.1.2: What's Changed

* refactor(tui): reduce duplicated code related to checktrees and item views by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/164>
* feat(tui): right click to return to previous view by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/166>
* chore(deps): bump codecov/codecov-action from 4.6.0 to 5.0.6 by @dependabot in <https://github.com/AnthonyMichaelTDM/mecomp/pull/178>
* chore(deps): bump codecov/codecov-action from 5.0.6 to 5.0.7 by @dependabot in <https://github.com/AnthonyMichaelTDM/mecomp/pull/179>
* feat(tui): implement Undo redo navigation by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/180>
* feat(tui): enhance key event handling with media keys by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/182>
* fix(storage): cleanup orphans by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/183>
* chore: update dependencies and prep for v0.1.2 release by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/184>

### **Full Changelog**: <https://github.com/AnthonyMichaelTDM/mecomp/compare/v0.1.1...v0.1.2>

## v0.1.1

### v0.1.1: What's Changed

* chore(deps): bump actions/upload-artifact from 4.4.0 to 4.4.3 by @dependabot in <https://github.com/AnthonyMichaelTDM/mecomp/pull/142>
* chore(deps): bump tonic from 0.12.2 to 0.12.3 in the cargo group by @dependabot in <https://github.com/AnthonyMichaelTDM/mecomp/pull/133>
* chore(deps): bump codecov/codecov-action from 4.5.0 to 4.6.0 by @dependabot in <https://github.com/AnthonyMichaelTDM/mecomp/pull/131>
* fix(core): underflow in duration watcher when subtracting durations by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/146>
* feat(tui/SongView): display playlists and collections containing the song by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/153>
* chore(deps): bump softprops/action-gh-release from 1 to 2 by @dependabot in <https://github.com/AnthonyMichaelTDM/mecomp/pull/148>
* chore(deps): bump actions/download-artifact from 4.1.7 to 4.1.8 by @dependabot in <https://github.com/AnthonyMichaelTDM/mecomp/pull/149>
* fix(daemon): properly create data / config directories on first run by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/155>

### **Full Changelog**: <https://github.com/AnthonyMichaelTDM/mecomp/compare/v0.1.0...v0.1.1>

* Bump surrealdb from 1.1.0 to 1.2.0 by @dependabot in <https://github.com/AnthonyMichaelTDM/mecomp/pull/4>
* Bump shlex from 1.2.0 to 1.3.0 by @dependabot in <https://github.com/AnthonyMichaelTDM/mecomp/pull/2>
* Bump h2 from 0.3.23 to 0.3.24 by @dependabot in <https://github.com/AnthonyMichaelTDM/mecomp/pull/3>
* Bump mio from 0.8.10 to 0.8.11 by @dependabot in <https://github.com/AnthonyMichaelTDM/mecomp/pull/5>
* Improve SurrealDB integration by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/7>
* improve ci by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/13>
* chore(deps): bump codecov/codecov-action from 4.0.1 to 4.4.1 by @dependabot in <https://github.com/AnthonyMichaelTDM/mecomp/pull/17>
* Daemon improvements by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/19>
* feat(utils): migrate OneOrMany into it's own crate by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/20>
* feat: feature gate storage crate by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/23>
* Implement MECOMP CLI by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/24>
* feat(audio): gapless playback by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/25>
* feat: Search by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/30>
* feat(search): improve song searching by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/32>
* Implement seeking by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/34>
* feat: Audio Analysis and recommendations by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/37>
* feat(Tui): Implement TUI by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/45>
* feat: Add default config file if it doesn't exist by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/52>
* chore(deps): bump codecov/codecov-action from 4.4.1 to 4.5.0 by @dependabot in <https://github.com/AnthonyMichaelTDM/mecomp/pull/36>
* issue 49 by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/55>
* perf(analysis): add analysis functions with callbacks by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/56>
* feat: implement clustering by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/62>
* feat(tui): CheckTree by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/66>
* fix(audio): Use saturating_add and saturating_sub for seek calculations by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/69>
* test(tui): make a test suite for mecomp-tui by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/70>
* chore(deps): bump actions/upload-artifact from 4.3.3 to 4.3.4 by @dependabot in <https://github.com/AnthonyMichaelTDM/mecomp/pull/73>
* chore(deps): bump the cargo group with 2 updates by @dependabot in <https://github.com/AnthonyMichaelTDM/mecomp/pull/81>
* feat(daemon): test client that runs on channels instead of over tcp by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/112>
* feat(core): allow users to specify multiple artist separators. by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/114>
* Surrealdb 2.0 migration by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/120>
* chore(deps): bump actions/upload-artifact from 4.3.4 to 4.4.0 by @dependabot in <https://github.com/AnthonyMichaelTDM/mecomp/pull/108>
* Add piped input support for radio, and playlist add subcommands by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/123>
* feat: greatly improve clustering quality and performance by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/134>
* test(cli): improve test coverage by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/136>
* Tui improvements by @AnthonyMichaelTDM in <https://github.com/AnthonyMichaelTDM/mecomp/pull/143>

### v0.1.0 New Contributors

* @dependabot made their first contribution in <https://github.com/AnthonyMichaelTDM/mecomp/pull/4>
* @AnthonyMichaelTDM made their first contribution in <https://github.com/AnthonyMichaelTDM/mecomp/pull/7>

### **Full Changelog**: <https://github.com/AnthonyMichaelTDM/mecomp/commits/v0.1.0>
