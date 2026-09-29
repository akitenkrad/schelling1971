#!/usr/bin/env python3
"""
visualize.py — Visualization script for the Schelling (1971) segregation model replication

Usage:
    python analysis/visualize.py [--results_dir RESULTS_DIR] [--output_dir OUTPUT_DIR]
                                  [--fps FPS] [--no_animation]

If --results_dir is omitted, the target is the run directory returned by
`runvault path --experiment schelling --latest --subcommand run`
(`runvault` must be on PATH).

Outputs:
    output_dir/
    ├── animation.gif          ← Animation of grid evolution
    ├── final_state.png        ← Grid heatmap of the final state
    ├── metrics_timeseries.png ← Metrics time-series plot
    ├── initial_state.png      ← Grid heatmap of the initial state
    └── comparison.png         ← Three-snapshot comparison of initial, intermediate, and final states
"""

from __future__ import annotations

import argparse
import glob
import os
import sys

import matplotlib as mpl
import matplotlib.animation as animation
import matplotlib.patches as mpatches
import matplotlib.pyplot as plt
import numpy as np
import pandas as pd

from runvault.read import artifacts_dir, figures_dir, metrics_wide, runvault_path

# --------------------------------------------------------------------------- #
# Color settings
# --------------------------------------------------------------------------- #
COLOR_EMPTY  = "#F5F5F0"   # Vacant cell (ivory)
COLOR_A      = "#2196F3"   # Group A (blue)
COLOR_B      = "#F44336"   # Group B (red)
COLOR_BG     = "#FAFAF8"   # Background

CMAP = mpl.colors.ListedColormap([COLOR_EMPTY, COLOR_A, COLOR_B])
NORM = mpl.colors.BoundaryNorm([0, 0.5, 1.5, 2.5], CMAP.N)

LEGEND_PATCHES = [
    mpatches.Patch(facecolor=COLOR_A,     edgecolor="black", linewidth=0.8, label="Group A"),
    mpatches.Patch(facecolor=COLOR_B,     edgecolor="black", linewidth=0.8, label="Group B"),
    mpatches.Patch(facecolor=COLOR_EMPTY, edgecolor="black", linewidth=0.8, label="Vacant"),
]

# --------------------------------------------------------------------------- #
# Utilities
# --------------------------------------------------------------------------- #

def snapshots_dir_of(results_dir: str) -> str:
    """Determine the snapshot location from the run directory."""
    return os.path.join(artifacts_dir(results_dir), "snapshots")


def load_snapshot(path: str, rows: int, cols: int) -> np.ndarray:
    """Convert a CSV snapshot into a grid matrix (rows×cols)."""
    df = pd.read_csv(path)
    mat = np.zeros((rows, cols), dtype=np.int8)
    for _, row in df.iterrows():
        mat[int(row["row"]), int(row["col"])] = int(row["cell"])
    return mat


def load_all_snapshots(snapshots_dir: str) -> tuple[list[np.ndarray], list[int]]:
    """Load all snapshots and return (matrix list, step list)."""
    paths = sorted(glob.glob(os.path.join(snapshots_dir, "step_*.csv")))
    if not paths:
        raise FileNotFoundError(f"Snapshots not found: {snapshots_dir}")

    # Infer the grid size from the list of filenames
    sample = pd.read_csv(paths[0])
    rows = int(sample["row"].max()) + 1
    cols = int(sample["col"].max()) + 1

    matrices, steps = [], []
    for p in paths:
        step = int(os.path.basename(p).replace("step_", "").replace(".csv", ""))
        mat = load_snapshot(p, rows, cols)
        matrices.append(mat)
        steps.append(step)

    return matrices, steps


def load_metrics(metrics_path: str) -> pd.DataFrame:
    """Read metrics.csv in wide format with one step per row."""
    return metrics_wide(metrics_path)


# --------------------------------------------------------------------------- #
# Visualization functions
# --------------------------------------------------------------------------- #

def plot_grid(
    ax: plt.Axes,
    mat: np.ndarray,
    step: int,
    title_prefix: str = "",
    show_legend: bool = True,
) -> None:
    """Plot the grid as a heatmap."""
    ax.imshow(mat, cmap=CMAP, norm=NORM, interpolation="nearest", aspect="equal")
    ax.set_xticks([])
    ax.set_yticks([])

    # Grid lines
    rows, cols = mat.shape
    for x in np.arange(-0.5, cols, 1):
        ax.axvline(x, color="#DDDDDD", linewidth=0.3)
    for y in np.arange(-0.5, rows, 1):
        ax.axhline(y, color="#DDDDDD", linewidth=0.3)

    title = f"{title_prefix}Step {step}" if title_prefix else f"Step {step}"
    ax.set_title(title, fontsize=10, pad=4)

    if show_legend:
        ax.legend(
            handles=LEGEND_PATCHES,
            loc="upper right",
            fontsize=7,
            framealpha=0.85,
            handlelength=1.0,
            handleheight=0.8,
        )


def save_single_grid(
    mat: np.ndarray,
    step: int,
    out_path: str,
    title: str = "",
) -> None:
    """Save a single grid as a PNG."""
    fig, ax = plt.subplots(figsize=(6, 5), facecolor=COLOR_BG)
    ax.set_facecolor(COLOR_BG)
    plot_grid(ax, mat, step, title_prefix=title)
    fig.tight_layout()
    fig.savefig(out_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  Saved: {out_path}")


def save_comparison(
    matrices: list[np.ndarray],
    steps: list[int],
    out_path: str,
) -> None:
    """Save a three-snapshot comparison of the initial, intermediate, and final states."""
    n = len(matrices)
    indices = [0, n // 2, n - 1]
    titles  = ["Initial state", "Intermediate state", "Final state"]

    fig, axes = plt.subplots(1, 3, figsize=(14, 5), facecolor=COLOR_BG)
    fig.suptitle("Schelling Segregation Model — Comparison of Grid States", fontsize=13, y=1.01)

    for ax, idx, title in zip(axes, indices, titles):
        ax.set_facecolor(COLOR_BG)
        plot_grid(ax, matrices[idx], steps[idx],
                  title_prefix=f"{title}\n", show_legend=(idx == indices[-1]))

    fig.tight_layout()
    fig.savefig(out_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  Saved: {out_path}")


def save_metrics_timeseries(df: pd.DataFrame, out_path: str) -> None:
    """Save the metrics time-series plot."""
    fig, axes = plt.subplots(2, 2, figsize=(12, 8), facecolor=COLOR_BG)
    fig.suptitle("Schelling Segregation Model — Metrics Time Series", fontsize=13)

    step = df["step"]

    # (1) Mean same-color neighbor ratio
    ax = axes[0, 0]
    ax.set_facecolor(COLOR_BG)
    ax.plot(step, df["avg_same_ratio"] * 100, color="#333333", lw=2, label="Overall")
    ax.plot(step, df["avg_same_ratio_a"] * 100, color=COLOR_A, lw=1.5,
            linestyle="--", label="Group A")
    ax.plot(step, df["avg_same_ratio_b"] * 100, color=COLOR_B, lw=1.5,
            linestyle="--", label="Group B")
    ax.set_xlabel("Step")
    ax.set_ylabel("Mean Same-Color Neighbor Ratio (%)")
    ax.set_title("Mean Same-Color Neighbor Ratio over Time")
    ax.legend(fontsize=8)
    ax.set_ylim(0, 105)
    ax.grid(True, alpha=0.3)

    # (2) Proportion with no opposite-color neighbors
    ax = axes[0, 1]
    ax.set_facecolor(COLOR_BG)
    ax.plot(step, df["pct_no_opposite"], color="#9C27B0", lw=2)
    ax.set_xlabel("Step")
    ax.set_ylabel("Proportion (%)")
    ax.set_title("Proportion of Agents with No Opposite-Color Neighbors")
    ax.set_ylim(0, 105)
    ax.grid(True, alpha=0.3)

    # (3) Numbers of dissatisfied and moving agents
    ax = axes[1, 0]
    ax.set_facecolor(COLOR_BG)
    ax.bar(step, df["n_dissatisfied"], color="#FF9800", alpha=0.7, label="Dissatisfied", width=0.8)
    ax.plot(step, df["n_moved"], color="#4CAF50", lw=2, label="Moved")
    ax.set_xlabel("Step")
    ax.set_ylabel("Number of Agents")
    ax.set_title("Numbers of Dissatisfied and Moving Agents")
    ax.legend(fontsize=8)
    ax.grid(True, alpha=0.3)

    # (4) Dissimilarity index (reference)
    ax = axes[1, 1]
    ax.set_facecolor(COLOR_BG)
    ax.plot(step, df["dissimilarity_index"], color="#607D8B", lw=2)
    ax.set_xlabel("Step")
    ax.set_ylabel("D")
    ax.set_title("Dissimilarity Index D (Reference)")
    ax.set_ylim(0, 0.6)
    ax.grid(True, alpha=0.3)

    fig.tight_layout()
    fig.savefig(out_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  Saved: {out_path}")


def save_animation(
    matrices: list[np.ndarray],
    steps: list[int],
    df: pd.DataFrame,
    out_path: str,
    fps: int = 5,
) -> None:
    """Save an animation of grid evolution as a GIF."""
    fig, axes = plt.subplots(
        1, 2, figsize=(12, 5),
        gridspec_kw={"width_ratios": [1, 1.3]},
        facecolor=COLOR_BG,
    )
    # Do not display a title (suptitle)

    ax_grid, ax_metrics = axes

    # --- Left: grid ---
    ax_grid.set_facecolor(COLOR_BG)
    im = ax_grid.imshow(
        matrices[0], cmap=CMAP, norm=NORM,
        interpolation="nearest", aspect="equal",
    )
    ax_grid.set_xticks([])
    ax_grid.set_yticks([])
    # GIF only: outline cell-color swatches in black
    anim_legend_patches = [
        mpatches.Patch(facecolor=COLOR_A, edgecolor="black", linewidth=0.8,
                       label="Group A"),
        mpatches.Patch(facecolor=COLOR_B, edgecolor="black", linewidth=0.8,
                       label="Group B"),
        mpatches.Patch(facecolor=COLOR_EMPTY, edgecolor="black", linewidth=0.8,
                       label="Vacant"),
    ]
    ax_grid.legend(
        handles=anim_legend_patches, loc="upper right",
        fontsize=7, framealpha=0.85,
        handlelength=1.0, handleheight=0.8,
    )
    title_text = ax_grid.set_title(f"Step {steps[0]}", fontsize=10)

    # Grid lines
    rows, cols = matrices[0].shape
    for x in np.arange(-0.5, cols, 1):
        ax_grid.axvline(x, color="#DDDDDD", linewidth=0.3)
    for y in np.arange(-0.5, rows, 1):
        ax_grid.axhline(y, color="#DDDDDD", linewidth=0.3)

    # --- Right: incrementally drawn metrics time series ---
    ax_metrics.set_facecolor(COLOR_BG)
    ax_metrics.set_xlabel("Step")
    ax_metrics.set_ylabel("Ratio (%)")
    ax_metrics.set_title("Metrics over Time")
    ax_metrics.set_xlim(df["step"].min(), df["step"].max())
    ax_metrics.set_ylim(0, 105)
    ax_metrics.grid(True, alpha=0.3)

    line_all, = ax_metrics.plot([], [], color="#333333", lw=2,   label="Mean Same-Color Ratio")
    line_a,   = ax_metrics.plot([], [], color=COLOR_A,   lw=1.5, linestyle="--", label="Group A")
    line_b,   = ax_metrics.plot([], [], color=COLOR_B,   lw=1.5, linestyle="--", label="Group B")
    line_noopp, = ax_metrics.plot([], [], color="#9C27B0", lw=1.5, linestyle=":",  label="No Opposite-Color Neighbors")
    vline = ax_metrics.axvline(0, color="#888888", linewidth=0.8, linestyle="--")
    ax_metrics.legend(fontsize=7, loc="lower right")

    # Map steps to DataFrame indices
    step_to_idx = {s: i for i, s in enumerate(df["step"].tolist())}

    def _init():
        im.set_data(matrices[0])
        line_all.set_data([], [])
        line_a.set_data([], [])
        line_b.set_data([], [])
        line_noopp.set_data([], [])
        return im, line_all, line_a, line_b, line_noopp, vline, title_text

    def _update(frame_idx: int):
        mat   = matrices[frame_idx]
        step  = steps[frame_idx]
        im.set_data(mat)
        title_text.set_text(f"Step {step}")

        # Draw metrics through the current step
        df_upto = df[df["step"] <= step]
        xs = df_upto["step"].values
        line_all.set_data(xs, df_upto["avg_same_ratio"].values * 100)
        line_a.set_data(xs, df_upto["avg_same_ratio_a"].values * 100)
        line_b.set_data(xs, df_upto["avg_same_ratio_b"].values * 100)
        line_noopp.set_data(xs, df_upto["pct_no_opposite"].values)
        vline.set_xdata([step, step])

        return im, line_all, line_a, line_b, line_noopp, vline, title_text

    ani = animation.FuncAnimation(
        fig,
        _update,
        frames=len(matrices),
        init_func=_init,
        blit=True,
        interval=1000 // fps,
    )

    fig.tight_layout()
    ani.save(out_path, writer="pillow", fps=fps, dpi=120)
    plt.close(fig)
    print(f"  Saved: {out_path}")


# --------------------------------------------------------------------------- #
# Main
# --------------------------------------------------------------------------- #

def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    p = argparse.ArgumentParser(
        prog="schelling-tools visualize",
        description="Visualization script for the Schelling segregation model"
    )
    p.add_argument(
        "--results_dir", "--results-dir", default=None,
        help="Run directory (default: runvault path --latest --subcommand run)"
    )
    p.add_argument(
        "--results_root", "--results-root", default="results",
        help="runvault results root (default: results)"
    )
    p.add_argument(
        "--experiment", default="schelling",
        help="runvault experiment name (default: schelling)"
    )
    p.add_argument(
        "--output_dir", "--output-dir", default=None,
        help="Figure output directory (default: <experiment>/figures/<run_slug>/)"
    )
    p.add_argument(
        "--fps", type=int, default=5,
        help="Animation FPS (default: 5)"
    )
    p.add_argument(
        "--no_animation", "--no-animation", action="store_true",
        help="Skip animation generation"
    )
    p.add_argument(
        "--max_frames", "--max-frames", type=int, default=0,
        help="Maximum number of animation frames (0=all frames)"
    )
    return p.parse_args(argv)


def main(argv: list[str] | None = None) -> None:
    args = parse_args(argv)

    results_dir = args.results_dir
    if results_dir is None:
        results_dir = runvault_path(args.experiment, args.results_root, subcommand="run")

    snapshots_dir = snapshots_dir_of(results_dir)
    metrics_path  = os.path.join(results_dir, "metrics.csv")
    # Figures are generated after the run finishes, so place them outside the run directory
    # (finish() finalizes manifest.csv, so adding them later would cause a mismatch).
    out_dir       = args.output_dir or figures_dir(results_dir)

    os.makedirs(out_dir, exist_ok=True)

    print("=== Schelling Segregation Model Visualization ===")
    print(f"Snapshots: {snapshots_dir}")
    print(f"Metrics:   {metrics_path}")
    print(f"Output:    {out_dir}")
    print("-----------------------------------")

    # Load data
    print("[1/5] Loading snapshots ...")
    matrices, steps = load_all_snapshots(snapshots_dir)
    print(f"      {len(matrices)} steps | grid {matrices[0].shape}")

    print("[2/5] Loading metrics ...")
    df = load_metrics(metrics_path)
    print(f"      {len(df)} rows")

    # Initial state
    print("[3/5] Saving initial state ...")
    save_single_grid(matrices[0], steps[0],
                     os.path.join(out_dir, "initial_state.png"), title="Initial State — ")

    # Final state
    save_single_grid(matrices[-1], steps[-1],
                     os.path.join(out_dir, "final_state.png"), title="Final State — ")

    # Comparison figure
    save_comparison(matrices, steps, os.path.join(out_dir, "comparison.png"))

    # Metrics time series
    print("[4/5] Saving metrics time series ...")
    save_metrics_timeseries(df, os.path.join(out_dir, "metrics_timeseries.png"))

    # Animation
    if not args.no_animation:
        print("[5/5] Generating animation (this may take some time) ...")
        mats = matrices
        stps = steps
        if args.max_frames > 0 and len(mats) > args.max_frames:
            # Uniform sampling
            idx = np.linspace(0, len(mats) - 1, args.max_frames, dtype=int)
            mats = [mats[i] for i in idx]
            stps = [stps[i] for i in idx]
        save_animation(mats, stps, df,
                       os.path.join(out_dir, "animation.gif"),
                       fps=args.fps)
    else:
        print("[5/5] Skipped animation")

    print("-----------------------------------")
    print("Done. Output files:")
    for f in sorted(os.listdir(out_dir)):
        size_kb = os.path.getsize(os.path.join(out_dir, f)) / 1024
        print(f"  {f:35s} ({size_kb:6.1f} KB)")


if __name__ == "__main__":
    main()
