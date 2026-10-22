# RFkit
For compiled executables, please go to [Releases](https://github.com/SLINGhub/RFkit/releases/latest) page. 

RFkit is a freely distributed software package for processing targeted metabolomics data from RapidFire-MS experiments.

## What it does

The tool takes the following as input: (i) mzML files containing the data for individual plates, (ii) transition information and (iii) batch log file containing sipping time information from the RapidFire system. The package automatically splices the data into individual injections and reports the total ion intensities of transitions integrated over the analysis time, with edges trimmed to yield expected peak shapes.


## Building from source

Once the repository is downloaded:

```bash
cd RFkit
cargo build --release
```

## Tests

Run from the repository root with Rust and Node.js 18 or newer installed:

```bash
cargo test
cargo test --locked --manifest-path gui/src-tauri/Cargo.toml
node --test gui/tests/frontend.test.cjs
cargo clippy --all-targets -- -D warnings
cargo clippy --locked --manifest-path gui/src-tauri/Cargo.toml --all-targets -- -D warnings
cargo fmt --check
cargo fmt --check --manifest-path gui/src-tauri/Cargo.toml
node --check gui/ui/app.js
```

The GUI Rust checks require the platform's Tauri build prerequisites and worker
sidecar (prepared by the platform build script in `gui/`). The worker tests use
temporary synthetic datasets and invoke the real CLI; they do not modify existing
datasets. Frontend tests exercise application functions with lightweight DOM and
desktop-bridge substitutes, so native WebView/IPC and canvas rendering still need
interactive checks. Reference-dataset and PDF comparisons are separate from these
regression suites.
