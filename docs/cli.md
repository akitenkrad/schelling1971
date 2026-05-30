**English** | [日本語](cli.ja.md)

# Rust CLI

The `schelling-simulation` crate exposes a CLI with the subcommands `run`, `sweep`, `bnm`, `bnm-basin`, and `tipping`. Build it once with `cargo build --release`; run from the workspace root with `cargo run --release -- <subcommand> ...`.

## `run` (Spatial Proximity Model)

Agent dynamics on a 2D grid.

```bash
# Build
cargo build --release

# Run with default settings (13×16 grid, τ=1/3, seed=42)
cargo run --release

# Run with explicit parameters
cargo run --release -- run \
    --rows 20 --cols 20 \
    --threshold 0.5 \
    --seed 42 \
    --output-dir results
```

**Main options:**

| Option | Default | Description |
|--------|---------|-------------|
| `--rows` | 13 | Grid rows |
| `--cols` | 16 | Grid columns |
| `--n-a`, `--n-b` | 0 (auto) | Number of agents per group (if 0, computed as equal counts from `--vacant-rate`) |
| `--threshold` | 0.333 | Tolerance threshold τ (used only when `--rule` is not given) |
| `--rule` | — | Satisfaction rule string (see below) |
| `--move-mode` | `standard` | Move operation mode (see below): `standard` or `strict` |
| `--move-strategy` | `nearest` | Move-target selection: `nearest` or `best-local` |
| `--vacant-rate` | 0.30 | Vacancy rate |
| `--seed` | — | Random seed |
| `--snapshot-interval` | 1 | Snapshot save interval (0 = do not save) |
| `--output-dir` | `results` | Output directory |

### Satisfaction rules (`--rule`)

The three preference forms from the paper are selectable via the `--rule` flag. When omitted, a `ratio` rule is built from `--threshold`.

| Rule | Form | Meaning | Paper figure |
|------|------|---------|--------------|
| Separationist | `ratio:X` | Same-color neighbor ratio ≥ X | Fig. 7–14 (default) |
| Congregationist | `min-same:N` | Absolute count of same-color neighbors ≥ N | Fig. 16 |
| Integrationist | `bounded:L:H` | Absolute count of same-color neighbors within range L–H | Fig. 17 |

```bash
# Separationist (equivalent to the default behavior)
cargo run --release -- run --rule ratio:0.333

# Congregationist (satisfied with at least 3 same-color neighbors)
cargo run --release -- run --rule min-same:3

# Integrationist (satisfied with 3–6 same-color neighbors; moves if too many)
cargo run --release -- run --rule bounded:3:6
```

### Move operation mode (`--move-mode`)

The paper distinguishes two operation forms for how agents relocate.

| Mode | Meaning | Paper figure |
|------|---------|--------------|
| `standard` | Lenient operation: only dissatisfied agents move (default) | Fig. 9–14 |
| `strict` | Strict operation: in addition to dissatisfied agents, satisfied agents also make speculative moves to any vacant cell that strictly increases their same-color ratio | Fig. 8 |

```bash
# Strict operation (Fig. 8): much sharper separation than lenient operation
cargo run --release -- run --threshold 0.5 --move-mode strict
```

Under strict operation, satisfied agents keep seeking more homogeneous neighborhoods, so the equilibrium is markedly more segregated than under lenient operation. The run stops once neither dissatisfied moves nor speculative improvements remain.

### Move-target strategy (`--move-strategy`)

Controls which satisfying vacant cell a moving agent chooses.

| Strategy | Meaning | Paper figure |
|----------|---------|--------------|
| `nearest` | The first satisfying vacant cell in ascending Chebyshev distance (default) | Fig. 7–14 |
| `best-local` | Among all satisfying vacant cells, the one that maximizes the post-move same-color ratio (ties broken by nearest distance, then row-major order) | Fig. 12 |

`best-local` makes minority members coalesce into the most homogeneous available block, which raises the minority-cluster ratio for the unequal-numbers case (Fig. 12) toward the paper's reported level (> 80%).

```bash
# Unequal numbers (Fig. 12), best-local strategy to tighten the minority cluster
cargo run --release -- run --n-a 97 --n-b 49 --threshold 0.333 --move-strategy best-local
```

`--move-mode` and `--move-strategy` are independent and may be combined. The `sweep` subcommand always uses `standard` / `nearest`.

**Output files:**

Each run is saved to a timestamped subdirectory. `results/latest` is a symbolic link to the most recent run.

```
results/
├── latest -> 20260405_153000       # symlink to the most recent run
├── 20260405_153000/
│   ├── metrics.csv                 # segregation metrics per step
│   └── snapshots/
│       ├── step_00000.csv          # initial state
│       ├── step_00001.csv
│       └── ...
└── 20260405_160000/
    └── ...
```

A `config.json` is also generated under `results/{timestamp}/`; see [`show-experiment-settings`](visualization.md#show-experiment-settings).

## `sweep` (Parameter Sweep)

Specify parameter ranges in `start:stop:step` form and run a grid search (sweep supports the `ratio` rule only).

```bash
# Sweep τ from 0.1 to 0.9 in steps of 0.1
cargo run --release -- sweep --threshold 0.1:0.9:0.1

# 2D sweep over τ and vacancy rate
cargo run --release -- sweep --threshold 0.1:0.5:0.1 --vacant-rate 0.2:0.4:0.1

# Multiple seeds to check statistical stability
cargo run --release -- sweep --threshold 0.1:0.9:0.1 --seeds 42,123,456

# Sweep with a different grid size
cargo run --release -- sweep --threshold 0.1:0.9:0.1 --rows 20 --cols 20
```

**Sweep options:**

| Option | Default | Description |
|--------|---------|-------------|
| `--threshold` | 0.333 | Range of τ (`start:stop:step`) or a single value |
| `--vacant-rate` | 0.30 | Range of vacancy rate (`start:stop:step`) or a single value |
| `--rows` | 13 | Grid rows |
| `--cols` | 16 | Grid columns |
| `--seeds` | 42 | Comma-separated random seeds |
| `--max-iterations` | 500 | Maximum number of iterations |
| `--snapshot-interval` | 0 | Snapshot save interval (0 = do not save) |
| `--output-dir` | `results` | Base output directory |

**Output files:**

```
results/{timestamp}_sweep/
├── sweep_summary.csv                # final metrics for every parameter combination
├── sweep_config.json                # sweep configuration (for reproduction)
├── tau_0.100_vac_0.300_seed_42/
│   └── metrics.csv
├── tau_0.200_vac_0.300_seed_42/
│   └── metrics.csv
└── ...
```

## Analytic Models — Bounded-Neighborhood Model (BNM) and Tipping Model

The analytic models from §3–§4 of the paper. The state is reduced to aggregate populations $(W, B)$, reaction curves are derived from the tolerance schedules, and the dynamics are analyzed on the phase plane. Spatial layout is not modeled.

### `bnm` / `bnm-basin` (Bounded-Neighborhood Model)

```bash
# Single run of the bounded-neighborhood model (phase portrait + trajectory)
cargo run --release -- bnm --preset fig18 --init 50,25
cargo run --release -- bnm --preset fig19 --init 60,60   # converges to a stable mix

# Basin analysis: sweep a grid of initial conditions to generate a basin map
cargo run --release -- bnm-basin --preset fig19 --init-grid 30x30
```

### `tipping` (Tipping Model)

```bash
# Tipping model (with speculation, asymmetry, and channeling)
cargo run --release -- tipping --preset fig31 --init 100,15
cargo run --release -- tipping --preset fig30a --speculation linear:alpha=0.3
cargo run --release -- tipping --preset fig31 --asymmetry "w_in=0.5:w_out=2.0:b_in=1.0:b_out=1.0"
```

### Presets

| Key | Figure | Structure | Expected equilibria |
|-----|--------|-----------|---------------------|
| `fig18` | Fig. 18 | Linear, 1:2 ratio | Two stable endpoints + unstable mix |
| `fig19` | Fig. 19 | Steep slope (median = 1.5) | Two endpoints + stable mix |
| `fig20` | Fig. 20 | Lenient linear (R_max = 3, symmetric) | Reaction-curve peak rises (wider mixing range) |
| `fig21` | Fig. 21 | Steep linear (R_max = 1, symmetric) | Reaction-curve peak falls (sharper separation) |
| `fig22` | Fig. 22 | Unequal numbers, curves do not intersect | No mixed equilibrium |
| `fig23` | Fig. 23 | Entry-cap quota | Quota produces a mix |
| `fig24` | Fig. 24 | Asymmetric tolerance (W R_max = 2, B R_max = 1) | Mixed equilibrium shifts off-center |
| `fig25` | Fig. 25 | Affine with zero-tolerance intercept | Stronger endpoint outflow |
| `fig26` | Fig. 26 | Capacity constraint C = 120 | Mixed equilibrium sits on the capacity line |
| `fig27` | Fig. 27 | Piecewise-linear (S-shaped CDF) | Non-uniform tolerance distribution |
| `fig28` | Fig. 28 | Unequal numbers + tolerant minority (R_max = 4) | Mixed equilibrium survives |
| `fig29` | Fig. 29 | Strong quota (B pop_max = 20) | Mixed equilibrium pinned to low B |
| `fig30a` | Fig. 30a | B extremely tolerant | In-tipping only |
| `fig30b` | Fig. 30b | Same structure as Fig. 18 | Out-tipping only |
| `fig31` | Fig. 31 | W intolerant + B tolerant | Both tipping (classic white flight) |
| `fig32` | Fig. 32 | Same structure as Fig. 19 | No tipping |

### Manual schedule specification

```bash
# Linear: F(R) = (R/r_max)*pop_max
cargo run --release -- bnm \
  --w-tolerance "linear:r_max=2.0:pop_max=100" \
  --b-tolerance "linear:r_max=2.0:pop_max=50" \
  --init 50,25

# Affine: F(R) = clamp(intercept_pop + slope*R, 0, pop_max)
cargo run --release -- bnm \
  --w-tolerance "affine:intercept_pop=20:slope=20:pop_max=100" \
  --b-tolerance "affine:intercept_pop=20:slope=20:pop_max=100" \
  --init 60,60
```

### Output files (BNM)

```
results/{timestamp}_bnm/
├── config.json                  # phase / dynamics / init / preset
├── tolerance_w.csv              # CDF F_W(R)
├── tolerance_b.csv
├── reaction_curve_w.csv         # (W, B_W(W))
├── reaction_curve_b.csv         # (B, W_B(B))
├── equilibria.csv               # (w, b, kind, stability)
├── vector_field.csv             # (w, b, dw_sign, db_sign, region)
└── trajectory.csv               # (t, w, b)
```

For basin analysis, a `basin.csv` is added; for tipping, a `tipping_classification.json` is added.

### Visualization

```bash
# BNM (reaction curves, phase portrait, trajectory, basin)
uv run schelling-tools visualize-bnm

# Tipping (the above + tipping classification annotations)
uv run schelling-tools visualize-tipping
```

See [Visualization](visualization.md) for details and output interpretation.
