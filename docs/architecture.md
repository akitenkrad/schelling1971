**English** | [日本語](architecture.ja.md)

# Architecture

## Repository structure

A two-project layout: a Cargo workspace + a uv workspace.

```
schelling1971/
├── Cargo.toml                 # Cargo workspace root
├── pyproject.toml             # uv workspace root
├── simulation/                # Rust project (schelling-simulation)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs                     # CLI (run / sweep / bnm / bnm-basin / tipping)
│       ├── config.rs                   # spatial model configuration
│       ├── grid.rs                     # Cell type (the grid is delegated to socsim-grid)
│       ├── metrics.rs                  # segregation metrics
│       ├── world.rs                    # socsim WorldState impl (socsim-grid GridIndex + color map)
│       ├── mechanisms.rs               # socsim Mechanism impl (move rule + scratch/request_stop)
│       ├── simulation.rs               # spatial model dynamics (driven by the socsim engine)
│       └── analytic/                   # analytic models (BNM + Tipping)
│           ├── tolerance.rs            # tolerance schedules (CDF)
│           ├── reaction.rs             # reaction curve B_W(W)
│           ├── phase.rs                # equilibria, stability, vector field
│           ├── dynamics.rs             # dynamic integration + basin analysis
│           ├── tipping.rs              # speculation, asymmetry, classification
│           ├── preset.rs               # Fig. 18–32 presets
│           └── runner.rs               # CLI I/O orchestration
├── tools/                     # Python project (schelling-tools)
│   ├── pyproject.toml
│   └── src/schelling_tools/
│       ├── cli.py                       # unified CLI (schelling-tools)
│       ├── visualize.py                 # spatial model visualization
│       ├── visualize_sweep.py           # sweep visualization
│       ├── visualize_bnm.py             # BNM phase portraits, trajectories, basins
│       ├── visualize_tipping.py         # Tipping visualization + classification annotation
│       ├── reproduce_paper.py           # one-shot reproduction of paper Fig. 7–32
│       └── show_experiment_settings.py  # display experiment settings
└── results/                   # simulation output (gitignored)
```

- `cargo run` launches the `simulation` crate from the workspace root (`-p schelling-simulation` may be omitted since there is a single member).
- `uv run` invokes the `schelling-tools` command exposed by the `tools` member of the uv workspace.

## Spatial model on the socsim framework

The simulation engine for the spatial model (`run` / `sweep`) is built on top of the social-simulation framework [rs-social-simulation-tools](https://github.com/akitenkrad/rs-social-simulation-tools) (socsim) — a git dependency, with the commit pinned in `Cargo.lock`. The main APIs used are:

- `WorldState` / `Mechanism` / `Scheduler` / `SimRng` — the engine core.
- `socsim-grid`'s `Grid` / `GridIndex` — the lattice, Moore neighborhood, and empty-cell search. The hand-written neighbor computation of the old implementation (moore_neighbors / vacant_cells / chebyshev, etc.) is replaced by `socsim-grid`'s `Grid`/`GridIndex`; the spatial module only implements the Schelling-specific judgments (satisfaction, same-color ratio, destination search) as domain helpers.
- `StepContext::request_stop` / `Simulation::stop_requested` — early stop on convergence.
- `StepContext::scratch` — passing step results (number moved, dissatisfied count at step start, convergence flag) to the driver, read via `Simulation::scratch`.
- `derive_seed` — separation of the initialization RNG from the engine RNG.

The move mechanism (`SchellingMoveMechanism`) fires in the `Decision` phase and processes agents in the activation order given by `StepContext::agent_order` (i.e. the order shuffled by the scheduler). Only agents that were dissatisfied at the start of the step are candidates to move; an agent that became dissatisfied mid-step due to others' moves is not moved within that step, which keeps `n_moved <= n_dissatisfied(at step start)`. Destination selection uses no randomness (nearest-neighbor greedy, scanning empty cells in ascending Chebyshev distance); the only source of randomness is the activation-order shuffle (`RandomActivationScheduler`). On convergence (the step-start dissatisfied set is empty) or a stall (`n_moved == 0`), the mechanism requests the engine to stop via `StepContext::request_stop`.

Note: because of the move to the socsim engine (and the RNG-stream separation), the consumed random-number sequence differs from the old binary, so bit-for-bit trajectory reproduction is not guaranteed. Reproducibility for a given seed (i.e. determinism) is guaranteed, and the qualitative reproduction of the paper is preserved.

## The orthogonal analytic module

The analytic models (BNM / Tipping) describe continuous dynamics, so they are not placed on socsim and remain an independent implementation under `simulation/src/analytic/`. The state is reduced to aggregate populations $(W, B)$; reaction curves are derived from tolerance schedules (CDFs); and equilibria, stability, the vector field, trajectories, and basins of attraction are analyzed on the phase plane. The Tipping model adds speculation, asymmetric in/out flows, and outcome classification on top of the BNM.

## References

Schelling, T. C. (1971). Dynamic Models of Segregation.
*Journal of Mathematical Sociology*, 1(2), 143–186.
DOI: [10.1080/0022250X.1971.9989794](https://doi.org/10.1080/0022250X.1971.9989794)
