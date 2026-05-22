**English** | [日本語](usecases.ja.md)

# Use Cases

This project reimplements all three models from Schelling (1971), "Dynamic Models of Segregation." Below are the typical things you can do with it, with pointers to the detailed documentation for each.

## 1. Reproduce spatial segregation (Spatial Proximity Model)

Watch how mild individual preferences ("at least one third of my neighbors should be the same color") produce pronounced macro-level segregation on a 2D grid. This is the core finding of the paper.

- Run a single simulation: see [CLI — `run`](cli.md#run-spatial-proximity-model).
- Reproduce the paper figures (Fig. 7–32) and compare against reported values: see [Reproduction](reproduction.md).
- Visualize the grid evolution and segregation metrics: see [Visualization](visualization.md).

## 2. Parameter sensitivity (sweep)

Sweep the tolerance threshold τ and the vacancy rate to see how the equilibrium segregation level responds. The sweep reveals the strongly non-linear relationship between micro preferences and macro outcomes.

- Run a grid search: see [CLI — `sweep`](cli.md#sweep-parameter-sweep).
- Visualize 1D line plots and 2D heatmaps: see [Visualization — sweep](visualization.md#sweep-visualization).

## 3. Phase-plane analysis (Bounded-Neighborhood Model)

Reduce the state to aggregate populations $(W, B)$, derive reaction curves from the tolerance schedules, and analyze the dynamics on the phase plane: equilibria, stability, vector field, trajectories, and basins of attraction.

- Run the analytic model: see [CLI — `bnm` / `bnm-basin`](cli.md#bnm--bnm-basin-bounded-neighborhood-model).
- Visualize phase portraits and basins: see [Visualization — BNM](visualization.md#analytic-model-visualization-bnm--tipping).

## 4. Tipping dynamics (Tipping Model)

Apply the BNM to housing markets to study tipping: speculative moves, asymmetric in/out flows, and the classification of outcomes (in-tipping, out-tipping, both, neither — the classic white-flight pattern).

- Run the tipping model: see [CLI — `tipping`](cli.md#tipping-tipping-model).
- Visualize tipping classifications: see [Visualization — Tipping](visualization.md#analytic-model-visualization-bnm--tipping).

## Where to go next

- [CLI](cli.md) — every Rust CLI subcommand and option.
- [Reproduction](reproduction.md) — the one-shot paper reproduction workflow.
- [Visualization](visualization.md) — the Python `schelling-tools` and how to read the outputs.
- [Architecture](architecture.md) — repository structure and how the model is built on the socsim framework.
