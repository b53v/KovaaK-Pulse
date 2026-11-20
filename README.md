# KovaaK Pulse

A lightweight, native Windows desktop application built with **Rust** and **iced 0.14 (`wgpu`)** for real-time monitoring and long-term performance analysis of **KovaaK's** aim training sessions.

## Features

- **Zero-Overhead Native Performance**: Single native `.exe` binary using DirectX 12 (`wgpu`) hardware acceleration; near-0% idle CPU usage in the background so it never interferes with in-game FPS.
- **Sub-Second Post-Run Analysis (< 0.5s)**: Monitors KovaaK's `stats/*.csv` and `performances/*.perf` directories via Windows `ReadDirectoryChangesW` (`notify`) with paired file debouncing and embedded SQLite (`rusqlite`, WAL mode) storage.
- **Single-Monitor Friendly**: Stays in the system tray and refreshes automatically without stealing focus from KovaaK's.
- **Rich Interactive Charts (Custom `iced::widget::canvas`)**:
  - Per-second dual-axis chart (KPS / Score bars + Accuracy % line)
  - Kill interval scatter plot (1-shot kills vs. multi-shot follow-ups) & first-half vs. second-half stability diagnostics
  - Long-term scenario trends with moving averages (MA5 / MA10 / MA20), PB step envelope, and `cm/360` sensitivity change attribution
  - Training calendar activity heatmap

## Architecture

- `src/config.rs`: Auto-detects Steam / KovaaK's paths via Windows Registry and `libraryfolders.vdf`, persists user settings.
- `src/parser/`: Pure-Rust CSV parser (`csv_parser.rs`) with `cm/360` conversion and zero-dependency Proto3 wire-format `.perf` decoder (`perf_parser.rs`).
- `src/model.rs` & `src/analytics.rs`: Core data models, single-run diagnostics, moving averages, PB envelope, and sensitivity grouping.
- `src/storage/db.rs`: Embedded SQLite database (`kovaak_pulse.db`) with deduplication and fast incremental sync.
- `src/watcher.rs`: Asynchronous `notify` + `tokio` file watcher with paired `.csv` / `.perf` debouncing.
- `src/ui/`: Dark esports theme UI, scenario sidebar, and interactive `Canvas` charts.

## Building & Running

Requires the Rust MSVC toolchain (`x86_64-pc-windows-msvc`) and Visual Studio 2022 C++ Build Tools.

```powershell
# Run tests
cargo test -- --nocapture

# Build optimized release binary
cargo build --release
```

The standalone executable will be output at `target\release\kovaak-pulse.exe`.
