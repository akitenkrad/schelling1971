<p align="center"><img src="docs/assets/hero.svg" width="100%"></p>

**English** | [日本語](README.ja.md)

# Dynamic Models of Segregation — Schelling (1971)

A reimplementation of the three models from Schelling (1971), "Dynamic Models of Segregation": the Spatial Proximity Model (agent dynamics on a 2D grid), the Bounded-Neighborhood Model (phase-plane analysis of aggregate populations), and the Tipping Model (a housing-market application with speculation and asymmetry). The simulation is written in Rust and the visualization tools in Python.

## Install & Quick start

```bash
# Build the Rust simulation
cargo build --release

# Run with default settings (13×16 grid, τ=1/3, seed=42)
cargo run --release

# Install the Python visualization tools (at the workspace root)
uv sync

# Visualize the most recent run
uv run schelling-tools visualize
```

## Documentation

- [Use cases](docs/usecases.md) — what you can do with this project, with pointers to the rest of the docs.
- [CLI](docs/cli.md) — the Rust CLI: `run`, `sweep`, and the analytic `bnm` / `bnm-basin` / `tipping` subcommands.
- [Reproduction](docs/reproduction.md) — the one-shot paper-reproduction workflow (Fig. 7–32).
- [Visualization](docs/visualization.md) — the Python `schelling-tools` and how to interpret the outputs.
- [Architecture](docs/architecture.md) — repository structure, the socsim framework, and references.

## License

MIT
