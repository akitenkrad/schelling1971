#!/usr/bin/env python3
"""
visualize_sweep.py — Visualization script for Schelling (1971) segregation-model parameter sweep results

Usage:
    uv run python analysis/visualize_sweep.py
    uv run python analysis/visualize_sweep.py --sweep_dir results/schelling/sweep_...
    uv run python analysis/visualize_sweep.py --output_dir out

If --sweep_dir is omitted, the target is the parent sweep run returned by
`runvault path --experiment schelling --latest --subcommand sweep`
(`runvault` must be on PATH).
The one-row-per-condition table does not exist as a file; reconstruct it from
the child runs linked to the parent.

Outputs:
    output_dir/
    ├── sweep_avg_same_ratio.png  ← Mean same-color neighbor ratio (1D line or 2D heatmap)
    ├── sweep_pct_no_opposite.png ← Proportion with no opposite-color neighbors
    ├── sweep_convergence.png     ← Convergence speed (final iteration count)
    ├── sweep_overview.png        ← 2×2 panel overview
    └── animation.gif            ← Grid animation by parameter combination
                                    (only when the sweep was run with --snapshot-interval > 0)
"""

from __future__ import annotations

import argparse
import json
import os

import matplotlib.animation as animation
import matplotlib.patches as mpatches
import matplotlib.pyplot as plt
import numpy as np
import pandas as pd

from runvault.read import config_parameters, figures_dir, runvault_path

from schelling_tools.sweep_summary import (
    legacy_run_dir_name as _run_dir_name,
    sweep_summary_table as load_summary,
)
from schelling_tools.visualize import (
    CMAP,
    COLOR_EMPTY,
    NORM,
    load_all_snapshots,
)

# --------------------------------------------------------------------------- #
# Color settings
# --------------------------------------------------------------------------- #
COLOR_BG = "#FAFAF8"

COLOR_AVG_SAME = "#333333"
COLOR_A = "#2196F3"
COLOR_B = "#F44336"
COLOR_PCT_NO_OPP = "#9C27B0"
COLOR_ITERATION = "#FF9800"
COLOR_DISSIMILARITY = "#607D8B"

# --------------------------------------------------------------------------- #
# Utilities
# --------------------------------------------------------------------------- #


def detect_sweep_type(df: pd.DataFrame) -> tuple[str, list[str]]:
    """Detect the dimensionality of the sweep.

    Returns:
        ("1d", [varying_col]) or ("2d", ["threshold", "vacant_rate"])
    """
    n_threshold = df["threshold"].nunique()
    n_vacant = df["vacant_rate"].nunique()

    if n_threshold > 1 and n_vacant > 1:
        return "2d", ["threshold", "vacant_rate"]
    elif n_threshold > 1:
        return "1d", ["threshold"]
    elif n_vacant > 1:
        return "1d", ["vacant_rate"]
    else:
        # Single parameter (only seeds differ) → use threshold as a dummy axis
        return "1d", ["threshold"]


def load_sweep_config(sweep_dir: str) -> dict | None:
    """Read the sweep grid definition.

    For runvault, this is `parameters` in `config.json` for the parent sweep run.
    For legacy runs, it is `sweep_config.json` directly under the sweep directory.
    """
    # Legacy sweeps have sweep_config.json but no config.json,
    # so its absence is not an error.
    params = config_parameters(sweep_dir, required=False)
    if params is not None:
        return params
    path = os.path.join(sweep_dir, "sweep_config.json")
    if os.path.exists(path):
        with open(path) as f:
            return json.load(f)
    return None


def make_subtitle(config: dict | None, df: pd.DataFrame) -> str:
    """Generate a subtitle string from the configuration."""
    parts: list[str] = []

    if config:
        rows = config.get("rows", None)
        cols = config.get("cols", None)
        if rows and cols:
            parts.append(f"{rows}×{cols} grid")
    else:
        rows_vals = df["rows"].unique()
        cols_vals = df["cols"].unique()
        if len(rows_vals) == 1 and len(cols_vals) == 1:
            parts.append(f"{rows_vals[0]}×{cols_vals[0]} grid")

    n_seeds = df["seed"].nunique()
    parts.append(f"{n_seeds} seeds")

    return ", ".join(parts)


# --------------------------------------------------------------------------- #
# 1D plotting functions
# --------------------------------------------------------------------------- #


def _plot_1d_line(
    ax: plt.Axes,
    df: pd.DataFrame,
    x_col: str,
    y_col: str,
    color: str,
    label: str,
    ylabel: str,
    title: str,
    *,
    y_percent: bool = False,
    ylim: tuple[float, float] | None = None,
    extra_lines: list[tuple[str, str, str]] | None = None,
    hline: float | None = None,
) -> None:
    """Plot a line/scatter plot for a 1D sweep."""
    ax.set_facecolor(COLOR_BG)

    grouped = df.groupby(x_col)
    xs = sorted(df[x_col].unique())
    n_seeds = df["seed"].nunique()
    scale = 100.0 if y_percent else 1.0

    # Mean and standard deviation
    means = [grouped.get_group(x)[y_col].mean() * scale for x in xs]
    stds = [grouped.get_group(x)[y_col].std() * scale for x in xs]

    # Plot individual points when there are multiple seeds
    if n_seeds > 1:
        for x in xs:
            vals = grouped.get_group(x)[y_col].values * scale
            ax.scatter(
                [x] * len(vals), vals,
                color=color, alpha=0.25, s=20, zorder=2,
            )
        ax.errorbar(
            xs, means, yerr=stds,
            color=color, lw=2, capsize=3, label=label, zorder=3,
        )
    else:
        ax.plot(xs, means, color=color, lw=2, marker="o", markersize=4, label=label)

    # Additional lines (avg_same_ratio_a, avg_same_ratio_b, etc.)
    if extra_lines:
        for ecol, ecolor, elabel in extra_lines:
            emeans = [grouped.get_group(x)[ecol].mean() * scale for x in xs]
            ax.plot(
                xs, emeans,
                color=ecolor, lw=1.5, linestyle="--", label=elabel,
            )

    # Horizontal reference line
    if hline is not None:
        ax.axhline(hline, color="#AAAAAA", linestyle=":", linewidth=1, label=f"{hline:.0f}% Reference Line")

    x_labels = {
        "threshold": "Threshold τ",
        "vacant_rate": "Vacancy Rate",
    }
    ax.set_xlabel(x_labels.get(x_col, x_col))
    ax.set_ylabel(ylabel)
    ax.set_title(title)
    if ylim:
        ax.set_ylim(*ylim)
    ax.legend(fontsize=7)
    ax.grid(True, alpha=0.3)


def _plot_1d_bar(
    ax: plt.Axes,
    df: pd.DataFrame,
    x_col: str,
    y_col: str,
    color: str,
    ylabel: str,
    title: str,
) -> None:
    """Plot a bar chart for a 1D sweep (e.g., number of convergence steps)."""
    ax.set_facecolor(COLOR_BG)

    grouped = df.groupby(x_col)
    xs = sorted(df[x_col].unique())
    n_seeds = df["seed"].nunique()

    means = [grouped.get_group(x)[y_col].mean() for x in xs]
    stds = [grouped.get_group(x)[y_col].std() for x in xs]

    if n_seeds > 1:
        ax.bar(
            range(len(xs)), means, yerr=stds,
            color=color, alpha=0.7, capsize=3, width=0.6,
        )
    else:
        ax.bar(range(len(xs)), means, color=color, alpha=0.7, width=0.6)

    ax.set_xticks(range(len(xs)))
    ax.set_xticklabels([f"{x:.3g}" for x in xs], fontsize=8)

    x_labels = {
        "threshold": "Threshold τ",
        "vacant_rate": "Vacancy Rate",
    }
    ax.set_xlabel(x_labels.get(x_col, x_col))
    ax.set_ylabel(ylabel)
    ax.set_title(title)
    ax.grid(True, alpha=0.3, axis="y")


# --------------------------------------------------------------------------- #
# 2D plotting functions
# --------------------------------------------------------------------------- #


def _plot_2d_heatmap(
    ax: plt.Axes,
    df: pd.DataFrame,
    z_col: str,
    cmap: str,
    title: str,
    *,
    z_percent: bool = False,
    fmt: str = ".1f",
) -> None:
    """Plot a heatmap for a 2D sweep."""
    ax.set_facecolor(COLOR_BG)
    scale = 100.0 if z_percent else 1.0

    pivot = df.groupby(["vacant_rate", "threshold"])[z_col].mean().unstack()
    data = pivot.values * scale

    thresholds = pivot.columns.values
    vacant_rates = pivot.index.values

    im = ax.imshow(
        data, aspect="auto", origin="lower",
        cmap=cmap, interpolation="nearest",
    )
    plt.colorbar(im, ax=ax, fraction=0.046, pad=0.04)

    # Annotate cells
    for i in range(len(vacant_rates)):
        for j in range(len(thresholds)):
            val = data[i, j]
            text_color = "white" if val > (data.max() + data.min()) / 2 else "black"
            ax.text(
                j, i, f"{val:{fmt}}",
                ha="center", va="center", fontsize=7, color=text_color,
            )

    ax.set_xticks(range(len(thresholds)))
    ax.set_xticklabels([f"{t:.2g}" for t in thresholds], fontsize=8)
    ax.set_yticks(range(len(vacant_rates)))
    ax.set_yticklabels([f"{v:.2g}" for v in vacant_rates], fontsize=8)
    ax.set_xlabel("Threshold τ")
    ax.set_ylabel("Vacancy Rate")
    ax.set_title(title)


# --------------------------------------------------------------------------- #
# Figure generation
# --------------------------------------------------------------------------- #


def save_avg_same_ratio(
    df: pd.DataFrame, sweep_type: str, sweep_cols: list[str],
    out_path: str, subtitle: str,
) -> None:
    """Save the mean same-color neighbor ratio plot."""
    fig, ax = plt.subplots(figsize=(8, 5), facecolor=COLOR_BG)
    fig.suptitle("Mean Same-Color Neighbor Ratio", fontsize=13)
    if subtitle:
        fig.text(0.5, 0.93, subtitle, ha="center", fontsize=9, color="#666666")

    if sweep_type == "1d":
        _plot_1d_line(
            ax, df, sweep_cols[0], "avg_same_ratio",
            COLOR_AVG_SAME, "Overall", "Mean Same-Color Neighbor Ratio (%)",
            "Mean Same-Color Neighbor Ratio",
            y_percent=True, ylim=(0, 105),
            extra_lines=[
                ("avg_same_ratio_a", COLOR_A, "Group A"),
                ("avg_same_ratio_b", COLOR_B, "Group B"),
            ],
            hline=50.0,
        )
    else:
        _plot_2d_heatmap(
            ax, df, "avg_same_ratio", "YlOrRd",
            "Mean Same-Color Neighbor Ratio (%)", z_percent=True, fmt=".1f",
        )

    fig.tight_layout(rect=[0, 0, 1, 0.92])
    fig.savefig(out_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  Saved: {out_path}")


def save_pct_no_opposite(
    df: pd.DataFrame, sweep_type: str, sweep_cols: list[str],
    out_path: str, subtitle: str,
) -> None:
    """Save the plot of the proportion with no opposite-color neighbors."""
    fig, ax = plt.subplots(figsize=(8, 5), facecolor=COLOR_BG)
    fig.suptitle("Proportion of Agents with No Opposite-Color Neighbors", fontsize=13)
    if subtitle:
        fig.text(0.5, 0.93, subtitle, ha="center", fontsize=9, color="#666666")

    if sweep_type == "1d":
        _plot_1d_line(
            ax, df, sweep_cols[0], "pct_no_opposite",
            COLOR_PCT_NO_OPP, "No Opposite-Color Neighbors", "Proportion (%)",
            "Proportion with No Opposite-Color Neighbors",
            ylim=(0, 105),
        )
    else:
        _plot_2d_heatmap(
            ax, df, "pct_no_opposite", "Purples",
            "Proportion with No Opposite-Color Neighbors (%)", fmt=".1f",
        )

    fig.tight_layout(rect=[0, 0, 1, 0.92])
    fig.savefig(out_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  Saved: {out_path}")


def save_convergence(
    df: pd.DataFrame, sweep_type: str, sweep_cols: list[str],
    out_path: str, subtitle: str,
) -> None:
    """Save the convergence-speed plot."""
    fig, ax = plt.subplots(figsize=(8, 5), facecolor=COLOR_BG)
    fig.suptitle("Number of Steps to Convergence", fontsize=13)
    if subtitle:
        fig.text(0.5, 0.93, subtitle, ha="center", fontsize=9, color="#666666")

    if sweep_type == "1d":
        _plot_1d_bar(
            ax, df, sweep_cols[0], "final_iteration",
            COLOR_ITERATION, "Number of Steps", "Number of Steps to Convergence",
        )
    else:
        _plot_2d_heatmap(
            ax, df, "final_iteration", "YlOrBr",
            "Number of Steps to Convergence", fmt=".0f",
        )

    fig.tight_layout(rect=[0, 0, 1, 0.92])
    fig.savefig(out_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  Saved: {out_path}")


def save_overview(
    df: pd.DataFrame, sweep_type: str, sweep_cols: list[str],
    out_path: str, subtitle: str,
) -> None:
    """Save the 2×2 panel overview figure."""
    fig, axes = plt.subplots(2, 2, figsize=(14, 10), facecolor=COLOR_BG)
    fig.suptitle("Schelling Segregation Model — Parameter Sweep Overview", fontsize=14)
    if subtitle:
        fig.text(0.5, 0.95, subtitle, ha="center", fontsize=9, color="#666666")

    if sweep_type == "1d":
        x_col = sweep_cols[0]

        # (1) Mean same-color neighbor ratio
        _plot_1d_line(
            axes[0, 0], df, x_col, "avg_same_ratio",
            COLOR_AVG_SAME, "Overall", "Mean Same-Color Neighbor Ratio (%)",
            "Mean Same-Color Neighbor Ratio",
            y_percent=True, ylim=(0, 105),
            extra_lines=[
                ("avg_same_ratio_a", COLOR_A, "Group A"),
                ("avg_same_ratio_b", COLOR_B, "Group B"),
            ],
            hline=50.0,
        )

        # (2) Proportion with no opposite-color neighbors
        _plot_1d_line(
            axes[0, 1], df, x_col, "pct_no_opposite",
            COLOR_PCT_NO_OPP, "No Opposite-Color Neighbors", "Proportion (%)",
            "Proportion with No Opposite-Color Neighbors",
            ylim=(0, 105),
        )

        # (3) Number of convergence steps
        _plot_1d_bar(
            axes[1, 0], df, x_col, "final_iteration",
            COLOR_ITERATION, "Number of Steps", "Number of Steps to Convergence",
        )

        # (4) Dissimilarity index
        _plot_1d_line(
            axes[1, 1], df, x_col, "dissimilarity_index",
            COLOR_DISSIMILARITY, "D", "Dissimilarity Index D",
            "Dissimilarity Index D",
            ylim=(0, 1.0),
        )
    else:
        # 2D heatmaps
        _plot_2d_heatmap(
            axes[0, 0], df, "avg_same_ratio", "YlOrRd",
            "Mean Same-Color Neighbor Ratio (%)", z_percent=True, fmt=".1f",
        )
        _plot_2d_heatmap(
            axes[0, 1], df, "pct_no_opposite", "Purples",
            "Proportion with No Opposite-Color Neighbors (%)", fmt=".1f",
        )
        _plot_2d_heatmap(
            axes[1, 0], df, "final_iteration", "YlOrBr",
            "Number of Steps to Convergence", fmt=".0f",
        )
        _plot_2d_heatmap(
            axes[1, 1], df, "dissimilarity_index", "Blues",
            "Dissimilarity Index D", fmt=".3f",
        )

    fig.tight_layout(rect=[0, 0, 1, 0.93])
    fig.savefig(out_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  Saved: {out_path}")


# --------------------------------------------------------------------------- #
# Grid animation by parameter combination
# --------------------------------------------------------------------------- #


def _snapshots_dir_for(
    df: pd.DataFrame, threshold: float, vacant_rate: float, seed: int, sweep_dir: str,
) -> str | None:
    """Look up the snapshot location of the child run corresponding to
    (τ, vacant_rate, seed) in the table.

    Child runvault runs are siblings of the parent sweep rather than children,
    and their names are hash-suffixed slugs, so the directory name cannot be
    composed from the conditions.
    """
    if "snapshots_dir" in df.columns:
        hit = df[
            np.isclose(df["threshold"].astype(float), threshold)
            & np.isclose(df["vacant_rate"].astype(float), vacant_rate)
            & (df["seed"].astype(int) == int(seed))
        ]
        if not hit.empty:
            return str(hit["snapshots_dir"].iloc[0])
        return None
    return os.path.join(sweep_dir, _run_dir_name(threshold, vacant_rate, seed), "snapshots")


def _enumerate_combos(
    df: pd.DataFrame, sweep_type: str, sweep_cols: list[str], seed: int,
) -> tuple[int, int, list[tuple[float, float]], list[str]]:
    """Enumerate the grid layout and (vacant_rate, threshold) for each cell.

    Returns:
        (n_rows, n_cols, combo_keys, combo_labels)
        combo_keys[i] = (vacant_rate, threshold) uniquely identifies the run for cell i.
    """
    if sweep_type == "2d":
        thresholds = sorted(df["threshold"].unique())
        vacant_rates = sorted(df["vacant_rate"].unique())
        n_rows = len(vacant_rates)
        n_cols = len(thresholds)
        # Rows: vacant_rate (increases top→bottom); columns: threshold (increases left→right)
        combo_keys = [(v, t) for v in vacant_rates for t in thresholds]
        combo_labels = [f"τ={t:.3g}, vac={v:.3g}" for v, t in combo_keys]
        return n_rows, n_cols, combo_keys, combo_labels

    # 1D: one parameter varies; obtain the fixed value of the other from df
    x_col = sweep_cols[0]
    xs = sorted(df[x_col].unique())
    n = len(xs)
    n_cols = min(n, 4)
    n_rows = (n + n_cols - 1) // n_cols

    if x_col == "threshold":
        fixed_vac = float(df["vacant_rate"].iloc[0])
        combo_keys = [(fixed_vac, t) for t in xs]
        combo_labels = [f"τ={t:.3g}" for t in xs]
    else:
        fixed_tau = float(df["threshold"].iloc[0])
        combo_keys = [(v, fixed_tau) for v in xs]
        combo_labels = [f"vac={v:.3g}" for v in xs]
    return n_rows, n_cols, combo_keys, combo_labels


def save_grid_animation(
    sweep_dir: str,
    df: pd.DataFrame,
    sweep_type: str,
    sweep_cols: list[str],
    out_path: str,
    *,
    seed: int | None = None,
    fps: float = 5,
    max_frames: int = 0,
    subtitle: str = "",
) -> bool:
    """Save a composite GIF that arranges grid-evolution animations for each
    parameter combination in a lattice.

    Each cell plays snapshots from the run for the selected seed. Runs with
    different convergence steps retain their final frame for synchronization.

    Returns:
        True: saved successfully / False: skipped because no snapshots are available
    """
    available_seeds = sorted(int(s) for s in df["seed"].unique())
    if not available_seeds:
        print("  Warning: sweep_summary.csv does not contain seed. Skipping.")
        return False
    if seed is None:
        seed = available_seeds[0]
    elif seed not in available_seeds:
        print(
            f"  Warning: specified seed={seed} is not in the sweep results {available_seeds}. "
            f"Using the first seed, seed={available_seeds[0]}."
        )
        seed = available_seeds[0]

    n_rows, n_cols, combo_keys, combo_labels = _enumerate_combos(
        df, sweep_type, sweep_cols, seed,
    )

    # Load snapshots for each cell
    cell_snapshots: list[tuple[list[np.ndarray], list[int]] | None] = []
    missing: list[str] = []
    for vac, tau in combo_keys:
        label = _run_dir_name(tau, vac, seed)
        snap_dir = _snapshots_dir_for(df, tau, vac, seed, sweep_dir)
        if snap_dir is None or not os.path.isdir(snap_dir):
            cell_snapshots.append(None)
            missing.append(label)
            continue
        try:
            matrices, steps = load_all_snapshots(snap_dir)
        except FileNotFoundError:
            cell_snapshots.append(None)
            missing.append(label)
            continue
        cell_snapshots.append((matrices, steps))

    valid = [s for s in cell_snapshots if s is not None]
    if not valid:
        print(
            "  Warning: no runs have snapshots. "
            "Rerun the sweep with `--snapshot-interval N` (N>0)."
        )
        return False

    if missing:
        print(f"  Note: {len(missing)} combinations have no snapshots (rendered as empty cells)")

    # Determine the common frame count for all cells (= step count of the longest run)
    n_frames = max(len(m) for m, _ in valid)

    # Thin frames to the maximum count (uniform sampling)
    if max_frames > 0 and n_frames > max_frames:
        sampled_idx = np.linspace(0, n_frames - 1, max_frames, dtype=int)
        n_frames = len(sampled_idx)
    else:
        sampled_idx = np.arange(n_frames)

    def cell_frame(cell_idx: int, frame_pos: int) -> tuple[np.ndarray, int]:
        data = cell_snapshots[cell_idx]
        assert data is not None
        matrices, steps = data
        # Derive the step position in the original run via sampled_idx and cap it at the run length
        original_pos = int(sampled_idx[frame_pos])
        clipped = min(original_pos, len(matrices) - 1)
        return matrices[clipped], steps[clipped]

    # Prepare the figure and axes
    cell_w = 2.6
    cell_h = 2.4
    fig_w = max(6.0, n_cols * cell_w)
    fig_h = max(4.0, n_rows * cell_h + 1.2)
    fig, axes = plt.subplots(
        n_rows, n_cols, figsize=(fig_w, fig_h), facecolor=COLOR_BG, squeeze=False,
    )
    # Do not display a title (suptitle)
    _ = (seed, subtitle)

    ims: list[plt.AxesImage | None] = []
    titles: list[plt.Text | None] = []
    n_combos = len(combo_keys)
    for idx in range(n_rows * n_cols):
        r, c = divmod(idx, n_cols)
        ax = axes[r, c]
        ax.set_facecolor(COLOR_BG)
        ax.set_xticks([])
        ax.set_yticks([])

        if idx >= n_combos:
            ax.axis("off")
            ims.append(None)
            titles.append(None)
            continue

        data = cell_snapshots[idx]
        label = combo_labels[idx]
        if data is None:
            ax.text(
                0.5, 0.5, "(no snapshots)", ha="center", va="center",
                transform=ax.transAxes, fontsize=8, color="#999999",
            )
            ax.set_title(label, fontsize=9)
            ims.append(None)
            titles.append(None)
            continue

        mat0, step0 = cell_frame(idx, 0)
        im = ax.imshow(mat0, cmap=CMAP, norm=NORM, interpolation="nearest", aspect="equal")
        title = ax.set_title(f"{label}\nstep {step0}", fontsize=8)
        ims.append(im)
        titles.append(title)

    # Draw the legend only once at the figure level
    # GIF only: outline cell-color swatches in black
    legend_patches = [
        mpatches.Patch(facecolor=COLOR_A, edgecolor="black", linewidth=0.8,
                       label="Group A"),
        mpatches.Patch(facecolor=COLOR_B, edgecolor="black", linewidth=0.8,
                       label="Group B"),
        mpatches.Patch(facecolor=COLOR_EMPTY, edgecolor="black", linewidth=0.8,
                       label="Vacant"),
    ]
    fig.legend(
        handles=legend_patches,
        loc="lower center",
        ncol=3,
        fontsize=8,
        frameon=False,
        bbox_to_anchor=(0.5, 0.0),
    )

    def _update(frame_pos: int):
        artists = []
        for i, data in enumerate(cell_snapshots):
            if data is None or ims[i] is None:
                continue
            mat, step = cell_frame(i, frame_pos)
            ims[i].set_data(mat)
            titles[i].set_text(f"{combo_labels[i]}\nstep {step}")
            artists.append(ims[i])
            artists.append(titles[i])
        return artists

    ani = animation.FuncAnimation(
        fig,
        _update,
        frames=n_frames,
        blit=False,
        interval=1000 // max(fps, 1),
    )

    # Reserve space for two title lines per row (use more of the top edge because there is no suptitle)
    fig.tight_layout(rect=[0, 0.05, 1, 0.98])
    fig.subplots_adjust(hspace=0.45, wspace=0.15)
    ani.save(out_path, writer="pillow", fps=fps, dpi=90)
    plt.close(fig)
    print(f"  Saved: {out_path}  ({n_frames} frames, {n_combos} cells, seed={seed})")
    return True


# --------------------------------------------------------------------------- #
# Main
# --------------------------------------------------------------------------- #


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    p = argparse.ArgumentParser(
        prog="schelling-tools visualize-sweep",
        description="Visualization script for Schelling segregation-model parameter sweeps"
    )
    p.add_argument(
        "--sweep_dir", "--sweep-dir", default=None,
        help="Parent sweep run directory (default: runvault path --latest --subcommand sweep)",
    )
    p.add_argument(
        "--results_root", "--results-root", default="results",
        help="runvault results root (default: results)",
    )
    p.add_argument(
        "--experiment", default="schelling",
        help="runvault experiment name (default: schelling)",
    )
    p.add_argument(
        "--output_dir", "--output-dir", default=None,
        help="Figure output directory (default: <experiment>/figures/<run_slug>/)",
    )
    p.add_argument(
        "--no_grid_animation", "--no-grid-animation", action="store_true",
        help="Skip grid animation generation by parameter combination",
    )
    p.add_argument(
        "--grid_seed", "--grid-seed", type=int, default=None,
        help="Seed used for the grid animation (default: first seed)",
    )
    p.add_argument(
        "--fps", type=float, default=5,
        help="Grid animation FPS (default: 5)",
    )
    p.add_argument(
        "--max_frames", "--max-frames", type=int, default=0,
        help="Maximum number of grid animation frames (0=all frames)",
    )
    return p.parse_args(argv)


def main(argv: list[str] | None = None) -> None:
    args = parse_args(argv)

    sweep_dir = args.sweep_dir
    if sweep_dir is None:
        sweep_dir = runvault_path(args.experiment, args.results_root, subcommand="sweep")

    # Figures are generated after the run finishes, so place them outside the run directory.
    out_dir = args.output_dir or figures_dir(sweep_dir)

    os.makedirs(out_dir, exist_ok=True)

    print("=== Schelling Segregation Model Parameter Sweep Visualization ===")
    print(f"Sweep results: {sweep_dir}")
    print(f"Output:        {out_dir}")
    print("---------------------------------------------------")

    # Load data (for runvault, reconstruct it from child runs)
    print("[1/6] Aggregating final values for each condition ...")
    df = load_summary(sweep_dir)
    print(f"      {len(df)} rows")

    # Load configuration
    print("[2/6] Checking sweep configuration ...")
    config = load_sweep_config(sweep_dir)
    sweep_type, sweep_cols = detect_sweep_type(df)
    subtitle = make_subtitle(config, df)
    print(f"      Sweep type: {sweep_type} ({', '.join(sweep_cols)})")
    print(f"      {subtitle}")

    # Generate figures
    print("[3/6] Saving mean same-color neighbor ratio ...")
    save_avg_same_ratio(
        df, sweep_type, sweep_cols,
        os.path.join(out_dir, "sweep_avg_same_ratio.png"), subtitle,
    )

    print("[4/6] Saving proportion with no opposite-color neighbors and convergence steps ...")
    save_pct_no_opposite(
        df, sweep_type, sweep_cols,
        os.path.join(out_dir, "sweep_pct_no_opposite.png"), subtitle,
    )
    save_convergence(
        df, sweep_type, sweep_cols,
        os.path.join(out_dir, "sweep_convergence.png"), subtitle,
    )

    print("[5/6] Saving overview panel ...")
    save_overview(
        df, sweep_type, sweep_cols,
        os.path.join(out_dir, "sweep_overview.png"), subtitle,
    )

    if args.no_grid_animation:
        print("[6/6] Skipped grid animation")
    else:
        print("[6/6] Generating grid animation by parameter combination ...")
        save_grid_animation(
            sweep_dir, df, sweep_type, sweep_cols,
            os.path.join(out_dir, "animation.gif"),
            seed=args.grid_seed,
            fps=args.fps,
            max_frames=args.max_frames,
            subtitle=subtitle,
        )

    print("---------------------------------------------------")
    print("Done. Output files:")
    for f in sorted(os.listdir(out_dir)):
        fpath = os.path.join(out_dir, f)
        if os.path.isfile(fpath):
            size_kb = os.path.getsize(fpath) / 1024
            print(f"  {f:40s} ({size_kb:6.1f} KB)")


if __name__ == "__main__":
    main()
