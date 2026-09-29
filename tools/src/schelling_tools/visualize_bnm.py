#!/usr/bin/env python3
"""
visualize_bnm.py — Visualization script for the Schelling (1971) bounded-neighborhood model (BNM)

Usage:
    schelling-tools visualize-bnm [--results_dir RESULTS_DIR] [--output_dir OUTPUT_DIR]

If --results_dir is omitted, the target is the run returned by
`runvault path --experiment schelling-analytic --latest --subcommand bnm`
(use `--subcommand bnm-basin` for basin-of-attraction analysis).

Inputs (run directory; under runvault, CSV files are in artifacts/):
    config.json
    tolerance_a.csv / tolerance_b.csv          # CDF (R, F(R))
    reaction_curve_a.csv / reaction_curve_b.csv  # (own, max_other)
    equilibria.csv                              # (a, b, kind, stability)
    vector_field.csv                            # (a, b, da_sign, db_sign, region)
    trajectory.csv                              # (t, a, b)  (single bnm run only)
    basin.csv                                   # (a0, b0, ..., converged_kind)  (bnm-basin only)

Outputs (output_dir):
    tolerance_schedules.png
    reaction_curves.png
    phase_portrait.png
    trajectory.png        (if trajectory.csv exists)
    basin_of_attraction.png  (if basin.csv exists)
"""
from __future__ import annotations

import argparse
import os
import sys

import matplotlib as mpl
import matplotlib.patches as mpatches
import matplotlib.pyplot as plt
import numpy as np
import pandas as pd

from runvault.read import artifacts_dir, config_parameters, figures_dir, runvault_path

# Color settings
COLOR_W_CURVE = "#1f77b4"   # Group A reaction curve (blue)
COLOR_B_CURVE = "#d62728"   # Group B reaction curve (red)
COLOR_TRAJECTORY = "#2ca02c"  # Trajectory (green)
COLOR_INITIAL = "#ff7f0e"   # Initial point (orange)
COLOR_CAPACITY = "#7f7f7f"  # Capacity constraint line (gray)

EQUILIBRIUM_COLORS = {
    "all_a": COLOR_W_CURVE,
    "all_b": COLOR_B_CURVE,
    "mixed": "#9467bd",
    "empty": "#7f7f7f",
}

# Colormap for basins
BASIN_COLORS = {
    "all_a": COLOR_W_CURVE,
    "all_b": COLOR_B_CURVE,
    "mixed": "#9467bd",
    "empty": "#bcbd22",
    "none": "#cccccc",
}


# --------------------------------------------------------------------------- #
# I/O
# --------------------------------------------------------------------------- #

def _extract_phase(cfg: dict | None) -> dict | None:
    """Extract phase information from config.json. BNM stores it in cfg["phase"],
    while Tipping stores it in cfg["config"]["phase"]."""
    if cfg is None:
        return None
    if "phase" in cfg:
        return cfg["phase"]
    if "config" in cfg and isinstance(cfg["config"], dict) and "phase" in cfg["config"]:
        return cfg["config"]["phase"]
    return None


def load_artifacts(results_dir: str) -> dict:
    """Load each CSV / JSON file from the BNM output and return them as a dict.

    For a runvault run, config.json is an envelope, so unwrap `parameters` and
    read CSV files from `artifacts/`. In a legacy run, both are directly under the run.
    """
    out: dict = {}
    out["config"] = config_parameters(results_dir, required=False)
    out["phase"] = _extract_phase(out["config"])
    csv_dir = artifacts_dir(results_dir)

    for name in [
        "tolerance_a",
        "tolerance_b",
        "reaction_curve_a",
        "reaction_curve_b",
        "equilibria",
        "vector_field",
        "trajectory",
        "basin",
    ]:
        path = os.path.join(csv_dir, f"{name}.csv")
        out[name] = pd.read_csv(path) if os.path.exists(path) else None
    return out


# --------------------------------------------------------------------------- #
# Plotting
# --------------------------------------------------------------------------- #

def plot_tolerance_schedules(art: dict, output_path: str) -> None:
    """Plot the CDF of the tolerance schedule F(R)."""
    fig, ax = plt.subplots(figsize=(8, 5))
    if art["tolerance_a"] is not None:
        df = art["tolerance_a"]
        ax.plot(df["r"], df["f_r"], color=COLOR_W_CURVE, linewidth=2, label="A: $F_A(R)$")
    if art["tolerance_b"] is not None:
        df = art["tolerance_b"]
        ax.plot(df["r"], df["f_r"], color=COLOR_B_CURVE, linewidth=2, label="B: $F_B(R)$")
    ax.set_xlabel("Tolerance Ratio R (Opposite Color / Same Color)")
    ax.set_ylabel("F(R) — Number with Tolerance Limit at Most R")
    ax.set_title("Tolerance Schedule (CDF)")
    ax.grid(True, alpha=0.3)
    ax.legend()
    fig.tight_layout()
    fig.savefig(output_path, dpi=150)
    plt.close(fig)


def plot_reaction_curves(art: dict, output_path: str) -> None:
    """Plot the reaction curves on the (W, B) phase plane."""
    fig, ax = plt.subplots(figsize=(7, 7))
    if art["reaction_curve_a"] is not None:
        df = art["reaction_curve_a"]
        ax.plot(df["own"], df["max_other"], color=COLOR_W_CURVE, linewidth=2,
                label="$B_A(A)$ — Group A Reaction Curve")
    if art["reaction_curve_b"] is not None:
        df = art["reaction_curve_b"]
        # The Group B reaction curve is (own=B, max_other=A_B(B)); swap the A/B axes when plotting
        ax.plot(df["max_other"], df["own"], color=COLOR_B_CURVE, linewidth=2,
                label="$A_B(B)$ — Group B Reaction Curve")

    # Capacity constraint line
    phase = art.get("phase")
    if phase and phase.get("capacity") is not None:
        c = phase["capacity"]
        x = np.linspace(0, c, 100)
        ax.plot(x, c - x, color=COLOR_CAPACITY, linestyle="--", linewidth=1,
                label=f"Capacity Constraint W+B={c:.0f}")

    # Equilibria
    if art["equilibria"] is not None:
        for _, row in art["equilibria"].iterrows():
            color = EQUILIBRIUM_COLORS.get(row["kind"], "#000000")
            marker = "o" if row["stability"] == "stable" else "x"
            size = 200 if row["stability"] == "stable" else 150
            ax.scatter(row["a"], row["b"], c=color, marker=marker, s=size,
                       edgecolors="black", linewidths=1.2, zorder=5)

    ax.set_xlabel("A (Group A Population)")
    ax.set_ylabel("B (Group B Population)")
    ax.set_title("Reaction Curves and Equilibria")
    ax.set_aspect("equal", adjustable="box")
    ax.grid(True, alpha=0.3)

    # Legend (add equilibrium markers)
    handles, labels = ax.get_legend_handles_labels()
    handles.append(plt.Line2D([], [], marker="o", color="w", markeredgecolor="black",
                               markerfacecolor="gray", markersize=10, label="Stable Equilibrium"))
    handles.append(plt.Line2D([], [], marker="x", color="black", linestyle="None",
                               markersize=10, label="Unstable Equilibrium"))
    ax.legend(handles=handles, loc="upper right")

    fig.tight_layout()
    fig.savefig(output_path, dpi=150)
    plt.close(fig)


def _pop_maxes(art: dict) -> tuple[float, float]:
    """Return the upper bounds (A_max, B_max) of the population pools."""
    phase = art.get("phase")
    w_max = b_max = 100.0
    if phase:
        w_max = phase.get("w_schedule", {}).get("pop_max", 100.0)
        b_max = phase.get("b_schedule", {}).get("pop_max", 100.0)
    return w_max, b_max


def _plot_reaction_curves(ax, art: dict, linewidth: float, alpha: float = 1.0) -> None:
    """Plot the reaction curves. Draw regions within the population pools
    ($A \\le A_{max}$, $B \\le B_{max}$) as solid lines and unreachable regions
    beyond the pools as dashed lines."""
    w_max, b_max = _pop_maxes(art)
    # Group A reaction curve B_A(A): (own=A, max_other=B). B>B_max is unreachable.
    if art["reaction_curve_a"] is not None:
        df = art["reaction_curve_a"]
        x, y = df["own"], df["max_other"]
        feasible = y <= b_max
        ax.plot(x, y, color=COLOR_W_CURVE, linewidth=linewidth, alpha=alpha,
                linestyle="--")
        ax.plot(x.where(feasible), y.where(feasible), color=COLOR_W_CURVE,
                linewidth=linewidth, alpha=alpha, linestyle="-", label="$B_A(A)$")
    # Group B reaction curve A_B(B): (max_other=A, own=B). A>A_max is unreachable.
    if art["reaction_curve_b"] is not None:
        df = art["reaction_curve_b"]
        x, y = df["max_other"], df["own"]
        feasible = x <= w_max
        ax.plot(x, y, color=COLOR_B_CURVE, linewidth=linewidth, alpha=alpha,
                linestyle="--")
        ax.plot(x.where(feasible), y.where(feasible), color=COLOR_B_CURVE,
                linewidth=linewidth, alpha=alpha, linestyle="-", label="$A_B(B)$")


def plot_phase_portrait(art: dict, output_path: str) -> None:
    """Plot the vector field, reaction curves, and equilibria."""
    fig, ax = plt.subplots(figsize=(8, 7))

    # Vector field (signs only → arrows)
    if art["vector_field"] is not None:
        vf = art["vector_field"]
        # Arrow length is sign × scale
        w_max, b_max = _pop_maxes(art)
        scale = 0.04 * max(w_max, b_max)
        ax.quiver(
            vf["a"], vf["b"],
            vf["da_sign"] * scale, vf["db_sign"] * scale,
            color="#888888", alpha=0.6, width=0.003, scale=1, scale_units="xy",
            angles="xy",
        )

    # Reaction curves (solid within the pools; dashed in unreachable regions)
    _plot_reaction_curves(ax, art, linewidth=2, alpha=1.0)

    # Equilibria
    if art["equilibria"] is not None:
        for _, row in art["equilibria"].iterrows():
            color = EQUILIBRIUM_COLORS.get(row["kind"], "#000000")
            marker = "o" if row["stability"] == "stable" else "x"
            size = 200 if row["stability"] == "stable" else 150
            ax.scatter(row["a"], row["b"], c=color, marker=marker, s=size,
                       edgecolors="black", linewidths=1.2, zorder=5)

    ax.set_xlabel("A (Group A Population)")
    ax.set_ylabel("B (Group B Population)")
    ax.set_title("Phase Plane (Reaction Curves + Vector Field + Equilibria)")
    ax.grid(True, alpha=0.3)
    ax.legend(loc="upper right")
    fig.tight_layout()
    fig.savefig(output_path, dpi=150)
    plt.close(fig)


def plot_trajectory(art: dict, output_path: str) -> None:
    """Overlay the trajectory on the reaction curves."""
    if art["trajectory"] is None:
        return
    fig, ax = plt.subplots(figsize=(8, 7))

    # Reaction curves (solid within the pools; dashed in unreachable regions)
    _plot_reaction_curves(ax, art, linewidth=1.5, alpha=0.6)

    # Trajectory
    traj = art["trajectory"]
    ax.plot(traj["a"], traj["b"], color=COLOR_TRAJECTORY, linewidth=2, label="Trajectory")
    ax.scatter([traj["a"].iloc[0]], [traj["b"].iloc[0]], c=COLOR_INITIAL, s=120,
               marker="*", edgecolors="black", linewidths=1, zorder=5, label="Initial Point")
    ax.scatter([traj["a"].iloc[-1]], [traj["b"].iloc[-1]], c=COLOR_TRAJECTORY, s=150,
               marker="o", edgecolors="black", linewidths=1.2, zorder=5, label="Endpoint")

    # Mark equilibria as well
    if art["equilibria"] is not None:
        for _, row in art["equilibria"].iterrows():
            color = EQUILIBRIUM_COLORS.get(row["kind"], "#000000")
            marker = "o" if row["stability"] == "stable" else "x"
            ax.scatter(row["a"], row["b"], c=color, marker=marker, s=80,
                       edgecolors="black", linewidths=0.8, zorder=4, alpha=0.7)

    ax.set_xlabel("A")
    ax.set_ylabel("B")
    ax.set_title("Dynamic Trajectory (Phase Plane)")
    ax.grid(True, alpha=0.3)
    ax.legend(loc="upper right")
    fig.tight_layout()
    fig.savefig(output_path, dpi=150)
    plt.close(fig)


def plot_basin_of_attraction(art: dict, output_path: str) -> None:
    """Map basins of attraction by coloring initial conditions (w0, b0) by their limit."""
    if art["basin"] is None:
        return
    df = art["basin"]
    fig, ax = plt.subplots(figsize=(8, 7))

    # Scatter plot colored by convergence category
    for kind in df["converged_kind"].unique():
        mask = df["converged_kind"] == kind
        color = BASIN_COLORS.get(kind, "#000000")
        ax.scatter(df.loc[mask, "a0"], df.loc[mask, "b0"],
                   c=color, s=50, alpha=0.7, label=kind, edgecolors="none")

    # Reaction-curve overlay
    if art["reaction_curve_a"] is not None:
        rc = art["reaction_curve_a"]
        ax.plot(rc["own"], rc["max_other"], color=COLOR_W_CURVE, linewidth=1.5,
                linestyle="--", alpha=0.5)
    if art["reaction_curve_b"] is not None:
        rc = art["reaction_curve_b"]
        ax.plot(rc["max_other"], rc["own"], color=COLOR_B_CURVE, linewidth=1.5,
                linestyle="--", alpha=0.5)

    # Equilibria
    if art["equilibria"] is not None:
        for _, row in art["equilibria"].iterrows():
            color = EQUILIBRIUM_COLORS.get(row["kind"], "#000000")
            marker = "o" if row["stability"] == "stable" else "x"
            size = 200 if row["stability"] == "stable" else 120
            ax.scatter(row["a"], row["b"], c=color, marker=marker, s=size,
                       edgecolors="black", linewidths=1.2, zorder=5)

    ax.set_xlabel("Initial $A_0$")
    ax.set_ylabel("Initial $B_0$")
    ax.set_title("Basins of Attraction (Initial Conditions → Limit)")
    ax.grid(True, alpha=0.3)
    ax.legend(loc="upper right", title="Limit")
    fig.tight_layout()
    fig.savefig(output_path, dpi=150)
    plt.close(fig)


# --------------------------------------------------------------------------- #
# CLI
# --------------------------------------------------------------------------- #

def resolve_results_dir(
    arg: str | None,
    *,
    results_root: str = "results",
    subcommand: str = "bnm",
    experiment: str = "schelling-analytic",
) -> str:
    """Resolve --results_dir. If omitted, ask runvault for the latest completed run.

    Analysis subcommands are stored in a separate experiment named `schelling-analytic`,
    so they are not mixed with simulation runs.
    """
    if arg:
        return arg
    return runvault_path(experiment, results_root, subcommand=subcommand)


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(
        prog="schelling-tools visualize-bnm",
        description="Visualize bounded-neighborhood model (BNM) analysis results",
    )
    parser.add_argument("--results_dir", default=None,
                        help="BNM run directory (default: runvault path --latest --subcommand bnm)")
    parser.add_argument("--results_root", "--results-root", default="results",
                        help="runvault results root (default: results)")
    parser.add_argument("--subcommand", default="bnm",
                        help="Target subcommand (bnm / bnm-basin)")
    parser.add_argument("--output_dir", default=None,
                        help="Figure output directory (default: <experiment>/figures/<run_slug>/)")
    args = parser.parse_args(argv)

    results_dir = resolve_results_dir(
        args.results_dir, results_root=args.results_root, subcommand=args.subcommand,
    )
    output_dir = args.output_dir or figures_dir(results_dir)
    os.makedirs(output_dir, exist_ok=True)

    print(f"[visualize-bnm] Input: {results_dir}")
    print(f"[visualize-bnm] Output: {output_dir}")

    art = load_artifacts(results_dir)

    figures = []
    plot_tolerance_schedules(art, os.path.join(output_dir, "tolerance_schedules.png"))
    figures.append("tolerance_schedules.png")

    plot_reaction_curves(art, os.path.join(output_dir, "reaction_curves.png"))
    figures.append("reaction_curves.png")

    plot_phase_portrait(art, os.path.join(output_dir, "phase_portrait.png"))
    figures.append("phase_portrait.png")

    if art["trajectory"] is not None:
        plot_trajectory(art, os.path.join(output_dir, "trajectory.png"))
        figures.append("trajectory.png")

    if art["basin"] is not None:
        plot_basin_of_attraction(art, os.path.join(output_dir, "basin_of_attraction.png"))
        figures.append("basin_of_attraction.png")

    print(f"[visualize-bnm] Generated {len(figures)} figures")
    for f in figures:
        print(f"  - {output_dir}/{f}")


if __name__ == "__main__":
    main()
