# Contributing

Small, focused changes are welcome. Open an issue for larger workflow changes before implementing them.

## Build and check

Use Rust 1.88 or newer. Install the Linux desktop development packages listed in the README if applicable.
Python 3 must be on PATH: the runner tests launch small, temporary Python fixtures without needing LLVM.

```sh
cargo run --locked
cargo fmt --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --locked
cargo package --locked
```

The tests cover legacy settings, self-contained installation data, command arguments with spaces,
nonzero exits, stdout/stderr, stop/restart, and preservation of project configuration.
Catalogue tests cover automatic startup refresh, debounced toolchain changes, selection preservation,
failed-refresh fallbacks, large discovery output, and a timed-out executable.

For a real analysis-and-fix test, install clang-tidy and clang-apply-replacements on PATH,
then point to the matching run-clang-tidy script:

```sh
CLANG_TIDY_GUI_RUNNER=/path/to/run-clang-tidy cargo test installed_llvm -- --ignored --nocapture
```

PowerShell:

```powershell
$env:CLANG_TIDY_GUI_RUNNER = 'C:/path/to/run-clang-tidy'
cargo test installed_llvm -- --ignored --nocapture
```

This test uses a temporary C++ project. It checks that analysis preserves the source,
applying fixes produces nullptr, and both runs preserve .clang-tidy.

## Visual review

Review the default 1180 × 820 window and the minimum 900 × 640 window, dark and light appearance,
long paths, empty search results, selected-only filtering, output overflow, and a running/stopping queue.
Keep file dialogs and settings errors readable.

To capture the actual application window, enable the optional screenshot feature.
Discovery finishes before capture, then the process exits after eframe captures a settled frame:

```sh
EFRAME_SCREENSHOT_TO=doc/screenshot_main.png cargo run --locked --features screenshot
```

PowerShell:

```powershell
$env:EFRAME_SCREENSHOT_TO = "$PWD/doc/screenshot_main.png"
cargo run --locked --features screenshot
Remove-Item Env:EFRAME_SCREENSHOT_TO
```

Use a desktop session with OpenGL. Screenshots do not need a separate image renderer.

## Code layout

- `src/config.rs`: serializable settings, catalogue selection, and atomic persistence.
- `src/runner.rs`: tool discovery, validation, command construction, and sequential execution.
- `src/ui.rs`: workspace, worker events, bounded output history, and appearance.
- `src/main.rs`: startup, legacy migration, and the native window.

The worker never writes a project .clang-tidy. Keep process arguments separate and avoid shell interpolation.
Both output streams must be drained concurrently. A stop request must let the active check finish.
Keep the app's idle state quiet: no polling loop or settings writes every frame.

## Release

1. Update Cargo.toml and Cargo.lock together, and date the changelog.
2. Run CI on Windows, Linux, macOS, and the minimum Rust version.
3. Run the real LLVM smoke test and review the current screenshots.
4. Run `cargo package --locked` and inspect `cargo package --list --locked`.
5. With crates.io credentials configured, run `cargo publish --dry-run --locked`.
6. Publish deliberately with `cargo publish --locked`; the workflow never publishes to crates.io.
7. Push a version tag such as `v0.2.0`. The release workflow builds platform archives and creates a draft GitHub release.
8. Review and publish the draft, then update the README installation instructions.

Do not commit local settings, target directories, or credentials.
