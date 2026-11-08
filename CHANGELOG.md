# Changelog

## 0.2.0 — 2026-11-08

### Workspace
- Project sidebar, searchable check families, selected-only view, and selection limited to the visible checks.
- Dark and light appearance, resizable output, progress, elapsed time, and keyboard shortcuts.
- Streamed stdout/stderr, bounded output history, and copyable logs.
- Check discovery from the installed clang-tidy, preserving matching selections.
- Include dotted Clang Static Analyzer names when discovering checks.
- Automatically refresh checks on startup and after changing clang-tidy, with debounced edits, a timeout, and labelled cached/bundled fallbacks.

### Reliability
- Use the selected compile_commands.json via its parent directory.
- Pass check selection as command arguments and preserve the project's .clang-tidy.
- Default to analysis, with explicit source-fix mode and configurable worker count.
- Report validation, process, configuration, and exit errors.
- Finish the active check before stopping; reset stop state for the next run.
- Store settings atomically in the user config directory and import legacy settings.
- Embed the starter check catalogue so installation does not depend on the launch directory.

### Repository
- Update eframe/egui to 0.37, migrate the panel and app APIs, and require Rust 1.95.
- Add crate metadata, a committed lockfile, regression tests, and a real LLVM smoke test.
- Add cross-platform CI, minimum-version and package checks, dependency updates, and draft binary releases.
- Expand installation, build database, contribution, and release documentation.

## 0.1.0

Initial prototype: configure paths, select bundled checks, and apply them sequentially.
