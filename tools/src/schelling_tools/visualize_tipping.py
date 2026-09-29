#!/usr/bin/env python3
"""
visualize_tipping.py — Visualization of the Schelling (1971) tipping model.

In addition to BNM visualization, read tipping_classification.json and annotate
the figure to indicate the presence or absence of in-tipping/out-tipping.

If --results_dir is omitted, the target is the run returned by:
`runvault path --experiment schelling-analytic --latest --subcommand tipping`
"""
from __future__ import annotations

import argparse
import json
import os

import matplotlib.pyplot as plt

from runvault.read import artifacts_dir, figures_dir
from schelling_tools.visualize_bnm import (
    load_artifacts,
    plot_basin_of_attraction,
    plot_phase_portrait,
    plot_reaction_curves,
    plot_tolerance_schedules,
    plot_trajectory,
    resolve_results_dir,
)


def load_classification(results_dir: str) -> dict | None:
    path = os.path.join(artifacts_dir(results_dir), "tipping_classification.json")
    if not os.path.exists(path):
        return None
    with open(path) as f:
        return json.load(f)


def annotate_classification(output_path: str, classification: dict) -> None:
    """Create a simple display that places the classification summary as text
    in a separate file instead of overlaying a label area on the existing
    reaction_curves.png or phase_portrait.png.
    """
    fig, ax = plt.subplots(figsize=(7, 4))
    ax.axis("off")
    label = classification.get("type", "(unknown)")
    aw = classification.get("all_a_stable", None)
    mx = classification.get("mixed_stable_exists", None)
    text = (
        f"Tipping type: {label}\n\n"
        f"  All-A endpoint is stable: {aw}\n"
        f"  Stable mixed equilibrium exists: {mx}\n\n"
        f"Type interpretation:\n"
        f"  in_tipping_only   — B reaction curve covers the all-A point + stable mixed equilibrium exists (B enters, leading to mixing)\n"
        f"  out_tipping_only  — all-A is stable + no stable mixed equilibrium (A exits in a cascade once B exceeds the threshold)\n"
        f"  both              — both paths above exist (typical white flight)\n"
        f"  neither           — endpoints and mixed equilibria are all stable (robust multistability)\n"
    )
    ax.text(0.02, 0.5, text, fontsize=11, verticalalignment="center")
    fig.tight_layout()
    fig.savefig(output_path, dpi=150)
    plt.close(fig)


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(
        prog="schelling-tools visualize-tipping",
        description="Visualize the tipping model (BNM visualization + classification annotation)",
    )
    parser.add_argument("--results_dir", default=None,
                        help="Tipping run directory (default: runvault path --latest --subcommand tipping)")
    parser.add_argument("--results_root", "--results-root", default="results",
                        help="runvault results root (default: results)")
    parser.add_argument("--output_dir", default=None,
                        help="Figure output directory (default: <experiment>/figures/<run_slug>/)")
    args = parser.parse_args(argv)

    results_dir = resolve_results_dir(
        args.results_dir, results_root=args.results_root, subcommand="tipping",
    )
    output_dir = args.output_dir or figures_dir(results_dir)
    os.makedirs(output_dir, exist_ok=True)

    print(f"[visualize-tipping] Input: {results_dir}")
    print(f"[visualize-tipping] Output: {output_dir}")

    art = load_artifacts(results_dir)
    cls = load_classification(results_dir)

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

    if cls:
        annotate_classification(os.path.join(output_dir, "tipping_classification.png"), cls)
        figures.append("tipping_classification.png")

    print(f"[visualize-tipping] Generated {len(figures)} figures")
    for f in figures:
        print(f"  - {output_dir}/{f}")


if __name__ == "__main__":
    main()
