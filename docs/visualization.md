**English** | [日本語](visualization.ja.md)

# Visualization (Python)

The Python tools live in the `schelling-tools` package. Python dependencies are managed with [uv](https://docs.astral.sh/uv/). Running `uv sync` at the workspace root installs the `tools` package as an editable install and makes the `schelling-tools` command available.

```bash
# Install dependencies (run at the workspace root)
uv sync

# Run visualization (automatically references the most recent run)
uv run schelling-tools visualize

# Limit the number of animation frames for faster generation
uv run schelling-tools visualize --max_frames 30 --fps 8

# Skip the animation and generate static images only
uv run schelling-tools visualize --no_animation

# Visualize a specific run
uv run schelling-tools visualize --results_dir "$(runvault path --experiment schelling --latest --subcommand run)"
```

Omitting `--results_dir` targets whatever `runvault path --experiment schelling --latest --subcommand run` returns (`runvault` must be on PATH). `--subcommand run` keeps it from picking up a sweep parent.

**Output files (single run):**

Figures go *outside* the run directory, in `<results_root>/<experiment>/figures/<run_slug>/`. `manifest.csv` is fixed by `finish()`, so adding something produced after the run to `artifacts/` would leave it unhashed and out of step with the record.

```
<results_root>/<experiment>/figures/<run_slug>/
├── animation.gif          # animation of the grid evolution
├── initial_state.png      # grid at the initial state
├── final_state.png        # grid at the final state
├── comparison.png         # 3-shot comparison: initial / middle / final
└── metrics_timeseries.png # metrics time-series plots
```

## Sweep visualization

`schelling-tools visualize-sweep` takes a sweep's parent run, collects the child runs that point at it through `lineage.parent_run_uid`, rebuilds the one-row-per-condition table (no `sweep_summary.csv` is written any more) and visualizes the relationship between parameters and final metrics. It automatically distinguishes 1D sweeps (one parameter varies) from 2D sweeps (two parameters vary).

```bash
# Visualize the most recent sweep (via runvault path --latest --subcommand sweep)
uv run schelling-tools visualize-sweep

# Specify a particular sweep
uv run schelling-tools visualize-sweep --sweep_dir "$(runvault path --experiment schelling --latest --subcommand sweep)"
```

**Main options:**

| Option | Default | Description |
|--------|---------|-------------|
| `--sweep_dir` | `runvault path --latest --subcommand sweep` | The sweep's parent run directory |
| `--results_root` | `results` | runvault results root |
| `--output_dir` | `<experiment>/figures/<run_slug>/` | Where figures are saved |
| `--no_grid_animation` | off | Skip generating the per-combination grid animation |
| `--grid_seed` | first seed | Seed used for the grid animation |
| `--fps` | 5 | FPS of the grid animation |
| `--max_frames` | 0 (all frames) | Maximum number of frames for the grid animation |

**Output files (sweep):**

```
<results_root>/<experiment>/figures/<sweep parent's run_slug>/
├── sweep_avg_same_ratio.png  # average same-color neighbor ratio (1D: line + error bars / 2D: heatmap)
├── sweep_pct_no_opposite.png # fraction with no opposite-color neighbors
├── sweep_convergence.png     # number of convergence steps
├── sweep_overview.png        # overview panel of 4 metrics (2×2)
└── sweep_grid_animation.gif  # per-combination grid progression animation
                              # (only when sweep was run with --snapshot-interval N (N>0))
```

- **1D sweep**: line plot. With multiple seeds, plots the mean line + standard-deviation error bars + individual points.
- **2D sweep**: heatmap, with values annotated inside each cell.
- **Grid animation**: a composite GIF in which each cell replays the snapshots of a single (τ, vacant_rate) run. In a 2D sweep the layout is rows = vacant_rate, columns = threshold. In a 1D sweep it is folded into a single row or rectangle. Runs with different convergence-step counts hold their final frame to stay synchronized. Runs without snapshots (sweep run with `--snapshot-interval 0`) are shown as empty cells.

## Analytic model visualization (BNM / Tipping)

```bash
# BNM (reaction curves, phase portrait, trajectory, basin)
uv run schelling-tools visualize-bnm

# Tipping (the above + tipping classification annotations)
uv run schelling-tools visualize-tipping
```

## `show-experiment-settings`

`schelling-tools show-experiment-settings` displays either (1) the list of paper-reproduction experiment definitions, or (2) the settings used in an existing result directory.

```bash
# Show the paper-reproduction experiment definitions (Fig. 7–17) (preview before reproduce)
uv run schelling-tools show-experiment-settings

# Show only specific experiment keys (comma-separated)
uv run schelling-tools show-experiment-settings --only fig11_tau_one_third,fig16_congregationist_min_same_3

# Show the settings of an existing result (the most recent run / sweep)
uv run schelling-tools show-experiment-settings --results-dir "$(runvault path --experiment schelling --latest --subcommand run)"

# Specify a particular result (run / sweep auto-detected)
uv run schelling-tools show-experiment-settings --results-dir results/20260425_153000

# Output in JSON format
uv run schelling-tools show-experiment-settings --json
uv run schelling-tools show-experiment-settings --results-dir <run directory> --json
```

The condition lives under `parameters` in the run directory's `config.json` (runvault's envelope, which also carries `schema_version`, `run_uid` and `runvault`). Whether it was a `run` or a `sweep` is answered by `subcommand` in `run.json`, so either can be passed. Flat `config.json` / `sweep_config.json` files written before the runvault migration are still read.

> **Note**: Result directories generated by older versions (before `config.json` output was supported) do not contain a settings file, so they cannot be displayed in `--results-dir` mode. In that case, please re-run.

## Output interpretation

### metrics.csv

A CSV file recording the segregation metrics at each step.

| Column | Description | Range | How to read |
|--------|-------------|-------|-------------|
| `step` | Simulation step number | 0– | — |
| `avg_same_ratio` | Average same-color neighbor ratio over all agents | 0.0–1.0 | Higher means more segregation. Random placement is close to the group ratio (≈0.5); at convergence it reaches ≈0.6–0.9 |
| `avg_same_ratio_a` | Average same-color neighbor ratio for group A | 0.0–1.0 | Used to check asymmetry of segregation between groups |
| `avg_same_ratio_b` | Average same-color neighbor ratio for group B | 0.0–1.0 | Same as above |
| `pct_no_opposite` | Fraction of agents with no opposite-color neighbors | 0–100 (%) | Higher means more agents surrounded only by the same color = stronger segregation |
| `dissimilarity_index` | Dissimilarity index D (simplified) | 0.0–0.5 | With the whole grid as one zone, D = 0.5 × \|a/A − b/B\|. ≈0 when group sizes are equal |
| `n_dissatisfied` | Number of dissatisfied agents | 0– | Number of agents judged dissatisfied by the rule. Convergence when this is 0 |
| `n_moved` | Number of agents that moved | 0– | Number of moves actually completed each step. Convergence when this is 0 |

### Visualization output (`<experiment>/figures/<run_slug>/`)

| File | Content | What to look at |
|------|---------|-----------------|
| `initial_state.png` | Grid of the initial placement | Confirm random placement. Blue = group A, red = group B, white = empty cell |
| `final_state.png` | Grid after convergence | Observe same-color cluster formation. The key Schelling insight is that pronounced segregation arises even at low τ |
| `comparison.png` | 3-shot comparison: initial / middle / final | Overview of the segregation process. See clusters grow from the initial random placement |
| `metrics_timeseries.png` | Metrics time-series (4 panels) | Top-left: rising curve of average same-color ratio; top-right: trend of no-opposite fraction; bottom-left: decay of dissatisfied/moved counts; bottom-right: dissimilarity index D |
| `animation.gif` | Animation of grid evolution | Left panel tracks agent movement, right panel tracks metric changes step by step |

### Sweep visualization output

| File | Content | What to look at |
|------|---------|-----------------|
| `sweep_avg_same_ratio.png` | Parameter vs average same-color neighbor ratio | Rising segregation curve with increasing τ. Group A/B differences are also visible |
| `sweep_pct_no_opposite.png` | Parameter vs no-opposite fraction | Increasing trend of agents surrounded by a fully same-color cluster |
| `sweep_convergence.png` | Parameter vs number of convergence steps | Convergence takes longer at moderate τ (0.4–0.6) and tends not to converge when too high |
| `sweep_overview.png` | Overview panel of 4 metrics | Grasp overall parameter sensitivity at a glance |
| `sweep_grid_animation.gif` | Per-combination grid progression animation | Each cell replays one parameter combination's run. Compare side by side how parameters affect segregation pattern formation |

### Analytic model visualization output (BNM / Tipping)

| File | Content | What to look at |
|------|---------|-----------------|
| `tolerance_schedules.png` | Tolerance CDFs $F_W(R), F_B(R)$ | Confirm schedule shape (linear / steep / with intercept) |
| `reaction_curves.png` | Reaction curves + equilibria on the phase plane $(W, B)$ | The parabola intersections and endpoint stability (● stable, × unstable) |
| `phase_portrait.png` | Reaction curves + vector field (quiver) + equilibria | Flow direction in each region (inflow/outflow) and the location of equilibria |
| `trajectory.png` | Trajectory on the phase plane | Transition path from the initial point (★) to the endpoint (●). Whether it converges to a mixed equilibrium or is drawn to an endpoint |
| `basin_of_attraction.png` | Color-coded by which equilibrium the initial condition converges to (bnm-basin only) | The size of each stable equilibrium's basin and the location of its boundary |
| `tipping_classification.png` | Tipping classification annotation (tipping only) | The judgment of `in_tipping_only` / `out_tipping_only` / `both` / `neither` |

### How to read typical results

- **τ=1/3 (default)**: each agent is satisfied under the lenient condition that at least one third of its neighbors are the same color, yet avg_same_ratio rises from the initial ≈0.50 to ≈0.65 or higher, and pronounced clusters form at the macro level. This is the core of Schelling's claim that "mild individual preferences produce macro-level segregation."
- **Non-linearity in the τ sensitivity analysis**: in the Fig. 14 sweep, avg_same rises gently up to about τ=0.35, then jumps sharply from 0.80→0.90 around τ=0.45–0.55. This is the core evidence for the "mismatch between micro preferences and macro outcomes" that Schelling emphasized in the paper.
- **Indistinguishability of congregationist vs separationist**: `--rule min-same:3` and `--rule ratio:0.4` show almost the same equilibrium same-color ratio (≈0.78). This corresponds to the key finding of the paper's Fig. 16 that "congregationist and separationist orientations produce equivalent segregation at the macro level."
- **Hard convergence of integrationist preferences**: with `--rule bounded:3:6`, some seeds converge slowly (15+ steps), consistent with the "dead-space formation" noted in the paper.
- **Convergence criterion**: the simulation ends when `n_dissatisfied=0` (everyone satisfied) or `n_moved=0` (no destination found).

#### Typical analytic-model (BNM / Tipping) results

- **Fig. 18 (linear, 1:2 ratio)**: only the endpoints `(W_max, 0)` and `(0, B_max)` are stable. The mixed equilibrium `(21.7, 34.0)` is unstable. This corresponds to Schelling's core proposition that "a mix is statically possible but cannot be maintained dynamically."
- **Fig. 19 (steep slope, median = 1.5)**: in addition to the two endpoints, the symmetric mixed equilibrium `(60, 60)` is stable. The asymmetric mixes `(27.6, 72.4)` and `(72.4, 27.6)` are saddle points. A trajectory smoothly converging from the initial `(30, 30)` to `(60, 60)` is observed.
- **Fig. 31 (both tipping)**: $W$ intolerant + $B$ tolerant makes the apex of the B reaction curve exceed $W_{\max}$. From the initial `(100, 15)`, a strongly curved trajectory toward all_black `(0, 50)` (white-flight dynamics) is observed.
- **Fig. 32 (no tipping)**: a stable mixed equilibrium exists and the endpoints are also stable. Essentially any initial condition converges to the nearest stable equilibrium — robust multi-phase stability.
- **Reading the basin analysis**: the `basin_of_attraction.png` produced by `bnm-basin` has color-region area ratios that directly correspond to "the probability of falling into each equilibrium under random initialization." For Fig. 19, the mixed equilibrium has the largest basin (≈50%).
