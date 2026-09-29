"""Reproduce Schelling's (1971) two-dimensional checkerboard model (Figure 7-14).

Run the paper's main experiments with the existing Rust binary
(`cargo run --release`), compare final metrics with the values reported in the
paper, and save the results as a table and JSON.

Usage:
    uv run python analysis/reproduce_paper.py
    uv run python analysis/reproduce_paper.py --seeds 42,123,456,789,2024
    uv run python analysis/reproduce_paper.py --skip-build  # Skip cargo build
    uv run python analysis/reproduce_paper.py --only fig11  # Run only a specific experiment

Reproduction targets:
    Fig. 11 : τ=1/3, equal numbers,   13×16, 30% vacant — mean same-color ratio ≈ 65-75%
    Fig. 9  : τ=1/2, equal numbers,   13×16, 30% vacant — mean same-color ratio ≈ 80-83%
    Fig. 8  : τ=1/2 (multiple seeds approximate strict operation) — 89-91%
    Fig. 12 : τ=1/3, unequal numbers 2:1, 13×16, 30% vacant — minority > 80%
    Fig. 14 : τ sensitivity analysis (0.10-0.60 in increments of 0.05)
    Fig. 16 : congregation preference (absolute same-color count ≥ 3) — mean same-color ratio ≈ 75%, no opposite-color neighbors ≈ 38%
    Fig. 17 : integration preference (absolute same-color count 3-6) — moderate segregation, but dead space forms
"""

from __future__ import annotations

import argparse
import csv
import json
import statistics
import subprocess
import sys
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path

from runvault.read import (
    artifacts_dir,
    metrics_wide,
    runvault_path,
    scope_metrics_from_csv,
)

from schelling_tools.sweep_summary import sweep_summary_table

# ---------------------------------------------------------------------------
# Experiment definitions
# ---------------------------------------------------------------------------


@dataclass
class Experiment:
    """Parameter settings and expected values for one paper experiment."""

    key: str
    figure: str
    description: str
    rows: int = 13
    cols: int = 16
    vacant_rate: float = 0.30
    # Satisfaction rule: when None, construct a ratio rule from threshold.
    # Examples: "ratio:0.333" / "min-same:3" / "bounded:3:6"
    rule: str | None = None
    threshold: float = 1.0 / 3.0
    # Movement mode: "standard" (lenient) / "strict" (strict, Fig.8)
    move_mode: str = "standard"
    # Destination selection strategy: "nearest" (existing) / "best-local" (Fig.12 cluster improvement)
    move_strategy: str = "nearest"
    # Agent counts (if 0, calculate equal counts automatically from vacant_rate).
    n_a: int = 0
    n_b: int = 0
    # Values reported in the paper (for reference and comparison displays).
    paper_avg_same_ratio: tuple[float, float] | None = None     # (min, max)
    paper_pct_no_opposite: tuple[float, float] | None = None
    paper_minority_avg_same: tuple[float, float] | None = None

    def cargo_args(self, seed: int, output_dir: str) -> list[str]:
        args = [
            "cargo", "run", "--release", "--quiet", "--",
            "run",
            "--rows", str(self.rows),
            "--cols", str(self.cols),
            "--vacant-rate", f"{self.vacant_rate:.6f}",
            "--seed", str(seed),
            "--snapshot-interval", "0",
            "--output-dir", output_dir,
        ]
        if self.rule is not None:
            args += ["--rule", self.rule]
        else:
            args += ["--threshold", f"{self.threshold:.6f}"]
        args += ["--move-mode", self.move_mode, "--move-strategy", self.move_strategy]
        if self.n_a > 0 and self.n_b > 0:
            args += ["--n-a", str(self.n_a), "--n-b", str(self.n_b)]
        return args

    def rule_label(self) -> str:
        if self.rule is not None:
            return self.rule
        return f"ratio:{self.threshold:.3f}"


def paper_experiments() -> list[Experiment]:
    # 13×16 = 208 cells, approximately 30% vacant = 62 vacancies, 146 agents.
    # Unequal numbers at 2:1 → 97:49 (146 total).
    return [
        Experiment(
            key="fig11_tau_one_third",
            figure="Fig. 11",
            description="τ=1/3, equal numbers, random initial placement (main experiment)",
            threshold=1.0 / 3.0,
            paper_avg_same_ratio=(0.65, 0.75),
            paper_pct_no_opposite=(35.0, 45.0),
        ),
        Experiment(
            key="fig09_tau_one_half_lenient",
            figure="Fig. 9",
            description="τ=1/2, equal numbers (lenient operation)",
            threshold=0.5,
            paper_avg_same_ratio=(0.80, 0.83),
            paper_pct_no_opposite=(38.0, 42.0),
        ),
        Experiment(
            key="fig08_tau_one_half_strict",
            figure="Fig. 8",
            description="τ=1/2, equal numbers (strict operation — satisfied agents also move speculatively)",
            threshold=0.5,
            move_mode="strict",
            paper_avg_same_ratio=(0.89, 0.91),
            paper_pct_no_opposite=(65.0, 70.0),
        ),
        Experiment(
            key="fig12_unequal_two_to_one",
            figure="Fig. 12",
            description="τ=1/3, unequal numbers 2:1 (A:97, B:49), best-local strategy improves the minority cluster",
            threshold=1.0 / 3.0,
            move_strategy="best-local",
            n_a=97,
            n_b=49,
            paper_avg_same_ratio=(0.70, 0.85),
            paper_minority_avg_same=(0.80, 1.00),
        ),
        Experiment(
            key="fig16_congregationist_min_same_3",
            figure="Fig. 16",
            description="Congregation preference: absolute same-color count ≥ 3 (regardless of ratio)",
            rule="min-same:3",
            paper_avg_same_ratio=(0.70, 0.80),
            paper_pct_no_opposite=(35.0, 42.0),
        ),
        Experiment(
            key="fig17_integrationist_bounded_3_6",
            figure="Fig. 17",
            description=(
                "Integration preference: absolute same-color count 3-6 (with lower and upper bounds) — "
                "the paper reports dead space formation and difficulty converging qualitatively, without quantitative values"
            ),
            rule="bounded:3:6",
        ),
    ]


def tau_sweep_taus() -> list[float]:
    # Equivalent to Fig. 14: τ=0.10, 0.15, ..., 0.60 (13 points).
    return [round(0.10 + 0.05 * i, 2) for i in range(11)]


# ---------------------------------------------------------------------------
# Analytic model (BNM + Tipping) experiment definitions
# ---------------------------------------------------------------------------


@dataclass
class AnalyticExperiment:
    """A single analytic model (BNM / Tipping) experiment."""

    key: str
    figure: str
    description: str
    model: str  # "bnm" / "tipping"
    preset: str
    init: tuple[float, float] | None = None
    # Expected values: list of equilibrium (type, stability) pairs and optional tipping type.
    expected_equilibria: list[tuple[str, str]] | None = None  # [("all_a","stable"), ...]
    expected_tipping_type: str | None = None  # "in_tipping_only", etc.
    # Expected convergence destination (trajectory).
    expected_converged_kind: str | None = None


def analytic_experiments() -> list[AnalyticExperiment]:
    return [
        AnalyticExperiment(
            key="fig18_linear_two_to_one",
            figure="Fig. 18",
            description="Linear schedule with a 1:2 ratio — two endpoint equilibria + an unstable mixed equilibrium.",
            model="bnm",
            preset="fig18",
            init=(50.0, 25.0),
            expected_equilibria=[
                ("all_a", "stable"),
                ("all_b", "stable"),
                ("mixed", "unstable"),
            ],
        ),
        AnalyticExperiment(
            key="fig19_steep_three_stable",
            figure="Fig. 19",
            description="Steep schedule (median=1.5) — three stable equilibria.",
            model="bnm",
            preset="fig19",
            init=(60.0, 60.0),
            expected_equilibria=[
                ("all_a", "stable"),
                ("all_b", "stable"),
                ("mixed", "stable"),
            ],
            expected_converged_kind="mixed",
        ),
        AnalyticExperiment(
            key="fig20_lenient_linear",
            figure="Fig. 20",
            description="Gradual linear schedule (R_max=3, symmetric) — increased tolerance raises the response-curve peak.",
            model="bnm",
            preset="fig20",
            init=(50.0, 50.0),
        ),
        AnalyticExperiment(
            key="fig21_steep_linear",
            figure="Fig. 21",
            description="Steep linear schedule (R_max=1, symmetric) — increased intolerance lowers the peak.",
            model="bnm",
            preset="fig21",
            init=(50.0, 50.0),
        ),
        AnalyticExperiment(
            key="fig22_unequal_no_intersection",
            figure="Fig. 22",
            description="Unequal numbers — nonintersecting response curves yield no mixed equilibrium.",
            model="bnm",
            preset="fig22",
            init=(60.0, 30.0),
        ),
        AnalyticExperiment(
            key="fig23_limiting_numbers",
            figure="Fig. 23",
            description="An upper admission quota creates a mixed equilibrium.",
            model="bnm",
            preset="fig23",
            init=(50.0, 15.0),
        ),
        AnalyticExperiment(
            key="fig24_asymmetric_tolerance",
            figure="Fig. 24",
            description="Asymmetric tolerance (A:R_max=2, B:R_max=1) — the mixed equilibrium is skewed.",
            model="bnm",
            preset="fig24",
            init=(50.0, 50.0),
        ),
        AnalyticExperiment(
            key="fig25_zero_tolerance_intercept",
            figure="Fig. 25",
            description="Includes zero-tolerance agents (intercept=10) — outflow at the endpoints increases.",
            model="bnm",
            preset="fig25",
            init=(60.0, 60.0),
        ),
        AnalyticExperiment(
            key="fig26_capacity_constraint",
            figure="Fig. 26",
            description="Capacity constraint C=120 — admission competition places the mixed equilibrium on the capacity line.",
            model="bnm",
            preset="fig26",
            init=(60.0, 50.0),
        ),
        AnalyticExperiment(
            key="fig27_piecewise_schedule",
            figure="Fig. 27",
            description="Piecewise-linear schedule (S-shaped CDF) — nonuniform tolerance distribution.",
            model="bnm",
            preset="fig27",
            init=(55.0, 55.0),
        ),
        AnalyticExperiment(
            key="fig28_unequal_tolerant_minority",
            figure="Fig. 28",
            description="Unequal numbers + tolerant minority (R_max=4) — mixing persists.",
            model="bnm",
            preset="fig28",
            init=(60.0, 25.0),
        ),
        AnalyticExperiment(
            key="fig29_strong_quota",
            figure="Fig. 29",
            description="Strong quota (B pop_max=20) — the mixed equilibrium is confined to the low-B region.",
            model="bnm",
            preset="fig29",
            init=(60.0, 10.0),
        ),
        AnalyticExperiment(
            key="fig30a_in_tipping_only",
            figure="Fig. 30a",
            description="In-tipping only. The B response curve covers the all-A point.",
            model="tipping",
            preset="fig30a",
            expected_tipping_type="in_tipping_only",
        ),
        AnalyticExperiment(
            key="fig30b_out_tipping_only",
            figure="Fig. 30b",
            description="Out-tipping only (same structure as Fig.18).",
            model="tipping",
            preset="fig30b",
            expected_tipping_type="out_tipping_only",
        ),
        AnalyticExperiment(
            key="fig31_both_tipping",
            figure="Fig. 31",
            description="In-tipping + out-tipping (typical white flight).",
            model="tipping",
            preset="fig31",
            init=(100.0, 15.0),
            expected_tipping_type="both",
            expected_converged_kind="all_b",
        ),
        AnalyticExperiment(
            key="fig32_neither_tipping",
            figure="Fig. 32",
            description="No tipping (robust multistability).",
            model="tipping",
            preset="fig32",
            init=(60.0, 60.0),
            expected_tipping_type="neither",
            expected_converged_kind="mixed",
        ),
    ]


# ---------------------------------------------------------------------------
# Rust binary invocation
# ---------------------------------------------------------------------------


# Locate the project root (workspace root).
# This module is at tools/src/schelling_tools/reproduce_paper.py, so
# `parents[3]` is the workspace root.
# The SCHELLING_PROJECT_ROOT environment variable can override it.
import os as _os
_env_root = _os.environ.get("SCHELLING_PROJECT_ROOT")
if _env_root:
    PROJECT_ROOT = Path(_env_root).resolve()
else:
    PROJECT_ROOT = Path(__file__).resolve().parents[3]


def ensure_build() -> None:
    print("=== cargo build --release ===")
    subprocess.run(
        ["cargo", "build", "--release"],
        cwd=PROJECT_ROOT,
        check=True,
    )


def run_cargo(args: list[str], cwd: Path) -> None:
    """Start a cargo subprocess and raise an exception if it fails."""
    subprocess.run(args, cwd=cwd, check=True, stdout=subprocess.DEVNULL)


def latest_run(output_dir: Path, subcommand: str, experiment: str = "schelling") -> Path:
    """Ask runvault for the directory of a run launched with `--output-dir output_dir`.

    Output is written to `<output_dir>/<experiment>/<run_slug>/`, so do not infer
    the directory layout here.
    """
    return Path(runvault_path(experiment, str(output_dir), subcommand=subcommand))


def read_final_metrics(output_dir: Path) -> dict:
    """Read final values from metrics.csv for the most recently executed run."""
    run_dir = latest_run(output_dir, "run")
    metrics_path = run_dir / "metrics.csv"
    df = metrics_wide(metrics_path)
    if df.empty:
        raise ValueError(f"Empty metrics.csv: {metrics_path}")
    final = df.iloc[-1]
    initial = df.iloc[0]
    scoped = scope_metrics_from_csv(metrics_path)
    return {
        "run_dir": str(run_dir.relative_to(PROJECT_ROOT)),
        "final_step": int(scoped.get("final_iteration", final["step"])),
        "converged": bool(scoped.get("converged", 0.0)),
        "initial_avg_same_ratio": float(initial["avg_same_ratio"]),
        "avg_same_ratio": float(final["avg_same_ratio"]),
        "avg_same_ratio_a": float(final["avg_same_ratio_a"]),
        "avg_same_ratio_b": float(final["avg_same_ratio_b"]),
        "pct_no_opposite": float(final["pct_no_opposite"]),
        "dissimilarity_index": float(final["dissimilarity_index"]),
        "n_dissatisfied": int(final["n_dissatisfied"]),
        "n_moved": int(final["n_moved"]),
    }


# ---------------------------------------------------------------------------
# Experiment runners
# ---------------------------------------------------------------------------


def run_experiment(exp: Experiment, seeds: list[int], base_dir: Path) -> dict:
    """Run one experiment setting with multiple seeds and return aggregated results."""
    exp_dir = base_dir / exp.key
    exp_dir.mkdir(parents=True, exist_ok=True)

    print(f"--- {exp.figure}: {exp.description} ---")
    print(f"    rule={exp.rule_label()} | grid={exp.rows}×{exp.cols} "
          f"| A:B={'auto' if exp.n_a == 0 else f'{exp.n_a}:{exp.n_b}'} "
          f"| vacant rate={exp.vacant_rate:.2f} | seeds={seeds}")

    per_seed: list[dict] = []
    for seed in seeds:
        seed_dir = exp_dir / f"seed_{seed}"
        seed_dir.mkdir(parents=True, exist_ok=True)
        args = exp.cargo_args(seed, str(seed_dir.relative_to(PROJECT_ROOT)))
        run_cargo(args, cwd=PROJECT_ROOT)
        m = read_final_metrics(seed_dir)
        m["seed"] = seed
        per_seed.append(m)
        print(f"    seed={seed}: step={m['final_step']:>3} "
              f"avg_same={m['avg_same_ratio']:.3f} "
              f"(A={m['avg_same_ratio_a']:.3f}, B={m['avg_same_ratio_b']:.3f}) "
              f"no_opp={m['pct_no_opposite']:.1f}%")

    # Aggregate the mean and standard deviation across seeds.
    def agg(key: str) -> dict:
        xs = [r[key] for r in per_seed]
        return {
            "mean": statistics.mean(xs),
            "std": statistics.pstdev(xs) if len(xs) > 1 else 0.0,
            "min": min(xs),
            "max": max(xs),
        }

    return {
        "experiment": exp.key,
        "figure": exp.figure,
        "description": exp.description,
        "parameters": {
            "rows": exp.rows,
            "cols": exp.cols,
            "rule": exp.rule_label(),
            "threshold": exp.threshold,
            "vacant_rate": exp.vacant_rate,
            "n_a": exp.n_a,
            "n_b": exp.n_b,
        },
        "paper_reference": {
            "avg_same_ratio": exp.paper_avg_same_ratio,
            "pct_no_opposite": exp.paper_pct_no_opposite,
            "minority_avg_same": exp.paper_minority_avg_same,
        },
        "seeds": seeds,
        "per_seed": per_seed,
        "aggregates": {
            "initial_avg_same_ratio": agg("initial_avg_same_ratio"),
            "avg_same_ratio": agg("avg_same_ratio"),
            "avg_same_ratio_a": agg("avg_same_ratio_a"),
            "avg_same_ratio_b": agg("avg_same_ratio_b"),
            "pct_no_opposite": agg("pct_no_opposite"),
            "final_step": agg("final_step"),
        },
    }


def run_analytic_experiment(exp: AnalyticExperiment, base_dir: Path) -> dict:
    """Run one analytic model experiment (BNM or Tipping)."""
    exp_dir = base_dir / exp.key
    exp_dir.mkdir(parents=True, exist_ok=True)

    print(f"--- {exp.figure}: {exp.description} ---")
    print(f"    model={exp.model} | preset={exp.preset} | init={exp.init}")

    args = [
        "cargo", "run", "--release", "--quiet", "--",
        exp.model,
        "--preset", exp.preset,
        "--output-dir", str(exp_dir.relative_to(PROJECT_ROOT)),
    ]
    if exp.init is not None:
        args += ["--init", f"{exp.init[0]},{exp.init[1]}"]
    run_cargo(args, cwd=PROJECT_ROOT)

    # Parse output: analytic subcommands use experiment=schelling-analytic.
    run_dir = latest_run(exp_dir, exp.model, experiment="schelling-analytic")
    csv_dir = Path(artifacts_dir(run_dir))

    # equilibria.csv
    eq_path = csv_dir / "equilibria.csv"
    equilibria = []
    if eq_path.exists():
        with eq_path.open() as f:
            for row in csv.DictReader(f):
                equilibria.append({
                    "a": float(row["a"]),
                    "b": float(row["b"]),
                    "kind": row["kind"],
                    "stability": row["stability"],
                })

    # trajectory.csv (endpoint)
    traj_path = csv_dir / "trajectory.csv"
    traj_final = None
    if traj_path.exists():
        with traj_path.open() as f:
            rows = list(csv.DictReader(f))
        if rows:
            last = rows[-1]
            traj_final = {
                "t": float(last["t"]),
                "a": float(last["a"]),
                "b": float(last["b"]),
                "n_steps": len(rows) - 1,
            }

    # tipping_classification.json
    cls_path = csv_dir / "tipping_classification.json"
    classification = None
    if cls_path.exists():
        with cls_path.open() as f:
            classification = json.load(f)

    # Compare with expected values.
    eq_kinds_observed = {(e["kind"], e["stability"]) for e in equilibria}
    eq_match = None
    if exp.expected_equilibria is not None:
        eq_match = all(tuple(p) in eq_kinds_observed for p in exp.expected_equilibria)
    tipping_match = None
    if exp.expected_tipping_type is not None and classification is not None:
        tipping_match = classification.get("type") == exp.expected_tipping_type
    converged_match = None
    if exp.expected_converged_kind is not None and traj_final is not None:
        # Check whether the endpoint is near the expected equilibrium (5% threshold).
        target = next(
            (e for e in equilibria if e["kind"] == exp.expected_converged_kind
             and e["stability"] == "stable"),
            None,
        )
        if target is not None:
            scale = max(50.0, max(e["a"] + e["b"] for e in equilibria))
            d = ((target["a"] - traj_final["a"]) ** 2 +
                 (target["b"] - traj_final["b"]) ** 2) ** 0.5
            converged_match = d < 0.05 * scale

    print(f"    equilibria: {len(equilibria)} total")
    if classification is not None:
        print(f"    tipping type: {classification.get('type')}")
    if traj_final is not None:
        print(f"    trajectory endpoint: ({traj_final['a']:.2f}, {traj_final['b']:.2f}) "
              f"({traj_final['n_steps']} steps)")

    return {
        "experiment": exp.key,
        "figure": exp.figure,
        "description": exp.description,
        "model": exp.model,
        "preset": exp.preset,
        "init": exp.init,
        "equilibria": equilibria,
        "trajectory_final": traj_final,
        "classification": classification,
        "expected_equilibria": exp.expected_equilibria,
        "expected_tipping_type": exp.expected_tipping_type,
        "expected_converged_kind": exp.expected_converged_kind,
        "match_equilibria": eq_match,
        "match_tipping_type": tipping_match,
        "match_converged_kind": converged_match,
        "run_dir": str(run_dir.relative_to(PROJECT_ROOT)),
    }


def run_tau_sweep(seeds: list[int], base_dir: Path) -> dict:
    """Reproduce Fig. 14 by sweeping τ=0.10-0.60 and its nonlinear equilibrium same-color ratio."""
    sweep_dir = base_dir / "fig14_tau_sweep"
    sweep_dir.mkdir(parents=True, exist_ok=True)

    taus = tau_sweep_taus()
    print(f"--- Fig. 14: τ sensitivity analysis (τ={taus[0]:.2f}-{taus[-1]:.2f}) ---")

    seeds_str = ",".join(str(s) for s in seeds)
    # Invoke cargo sweep in start:stop:step format.
    tau_range = f"{taus[0]:.2f}:{taus[-1]:.2f}:0.05"
    args = [
        "cargo", "run", "--release", "--quiet", "--",
        "sweep",
        "--threshold", tau_range,
        "--vacant-rate", "0.30",
        "--rows", "13", "--cols", "16",
        "--seeds", seeds_str,
        "--snapshot-interval", "0",
        "--output-dir", str(sweep_dir.relative_to(PROJECT_ROOT)),
    ]
    run_cargo(args, cwd=PROJECT_ROOT)

    # Rebuild the one-row-per-condition table from child runs
    # (sweep_summary.csv is no longer written).
    parent_dir = latest_run(sweep_dir, "sweep")
    summary = sweep_summary_table(parent_dir)
    rows = summary.to_dict("records")

    # Aggregate by τ.
    by_tau: dict[float, list[dict]] = {}
    for row in rows:
        tau = round(float(row["threshold"]), 3)
        by_tau.setdefault(tau, []).append(row)

    table = []
    for tau in sorted(by_tau.keys()):
        xs = [float(r["avg_same_ratio"]) for r in by_tau[tau]]
        no_opps = [float(r["pct_no_opposite"]) for r in by_tau[tau]]
        table.append({
            "threshold": tau,
            "avg_same_ratio_mean": statistics.mean(xs),
            "avg_same_ratio_std": statistics.pstdev(xs) if len(xs) > 1 else 0.0,
            "pct_no_opposite_mean": statistics.mean(no_opps),
            "n_seeds": len(xs),
        })

    print(f"    {'τ':>6} | {'avg_same':>10} | {'no_opp':>8}")
    for row in table:
        print(f"    {row['threshold']:>6.2f} | "
              f"{row['avg_same_ratio_mean']:>6.3f}±{row['avg_same_ratio_std']:<3.3f} | "
              f"{row['pct_no_opposite_mean']:>6.1f}%")

    return {
        "experiment": "fig14_tau_sweep",
        "figure": "Fig. 14",
        "description": "τ sensitivity analysis: equilibrium same-color ratio rises sharply from 0.35 to 0.50",
        "taus": taus,
        "seeds": seeds,
        "table": table,
        "sweep_dir": str(parent_dir.relative_to(PROJECT_ROOT)),
    }


# ---------------------------------------------------------------------------
# Report generation
# ---------------------------------------------------------------------------


def _format_range(rng: tuple[float, float] | None, unit: str = "") -> str:
    """Format a range as "min — max", displaying values without unit conversion."""
    if rng is None:
        return "-"
    if unit == "%":
        return f"{rng[0]:.1f}% — {rng[1]:.1f}%"
    return f"{rng[0]:.3f} — {rng[1]:.3f}"


def _in_range(value: float, rng: tuple[float, float] | None) -> str:
    if rng is None:
        return ""
    return "✓" if rng[0] <= value <= rng[1] else "✗"


def render_analytic_comparison(experiments: list[dict]) -> str:
    """Summarize analytic model (BNM + Tipping) reproduction results."""
    if not experiments:
        return ""
    lines = []
    lines.append("=" * 90)
    lines.append("Analytic model (BNM + Tipping) reproduction results")
    lines.append("=" * 90)
    lines.append(f"{'Figure':<10}{'Model':<10}{'Equilibria':<10}{'Type':<22}{'Trajectory':<12}")
    lines.append("-" * 90)

    for exp in experiments:
        n_eq = len(exp["equilibria"])
        cls = exp["classification"]
        cls_label = cls.get("type") if cls else "-"
        if exp["expected_tipping_type"] is not None:
            mark = "✓" if exp["match_tipping_type"] else "✗"
            cls_label = f"{cls_label} {mark}"

        eq_label = f"{n_eq} total"
        if exp["expected_equilibria"] is not None:
            mark = "✓" if exp["match_equilibria"] else "✗"
            eq_label = f"{eq_label} {mark}"

        traj_label = "-"
        if exp["match_converged_kind"] is not None:
            mark = "✓" if exp["match_converged_kind"] else "✗"
            traj_label = f"{exp['expected_converged_kind']} {mark}"

        lines.append(f"{exp['figure']:<10}{exp['model']:<10}"
                     f"{eq_label:<10}{cls_label:<22}{traj_label:<12}")

    lines.append("=" * 90)
    return "\n".join(lines)


def render_comparison(experiments: list[dict], tau_sweep: dict | None) -> str:
    lines = []
    lines.append("=" * 90)
    lines.append("Schelling (1971) reproduction results vs. values reported in the paper")
    lines.append("=" * 90)
    header = (f"{'Figure':<10}{'avg_same_ratio':<28}{'pct_no_opposite':<28}{'converged':<12}")
    lines.append(header)
    lines.append("-" * 90)

    for exp in experiments:
        agg = exp["aggregates"]
        ref = exp["paper_reference"]
        avg_mean = agg["avg_same_ratio"]["mean"]
        avg_std = agg["avg_same_ratio"]["std"]
        no_opp_mean = agg["pct_no_opposite"]["mean"]
        no_opp_std = agg["pct_no_opposite"]["std"]

        avg_cell = f"{avg_mean:.3f}±{avg_std:.3f} {_in_range(avg_mean, ref['avg_same_ratio'])}"
        paper_avg = _format_range(ref["avg_same_ratio"])
        no_opp_cell = f"{no_opp_mean:.1f}±{no_opp_std:.1f}% {_in_range(no_opp_mean, ref['pct_no_opposite'])}"
        paper_no_opp = _format_range(ref["pct_no_opposite"], unit="%")

        converged = sum(1 for r in exp["per_seed"] if r["n_dissatisfied"] == 0)
        conv_cell = f"{converged}/{len(exp['per_seed'])}"

        lines.append(f"{exp['figure']:<10}"
                     f"{avg_cell:<14}(paper:{paper_avg:<10}) "
                     f"{no_opp_cell:<14}(paper:{paper_no_opp:<10}) "
                     f"{conv_cell}")
        # For unequal numbers, also display the minority group's mean same-color ratio.
        if ref["minority_avg_same"] is not None:
            params = exp["parameters"]
            if params["n_a"] > params["n_b"]:
                minority_key = "avg_same_ratio_b"
            else:
                minority_key = "avg_same_ratio_a"
            minority_mean = agg[minority_key]["mean"]
            in_range = _in_range(minority_mean, ref["minority_avg_same"])
            paper_minority = _format_range(ref["minority_avg_same"])
            lines.append(f"{'':10}minority same-color ratio: {minority_mean:.3f} {in_range} "
                         f"(paper: {paper_minority})")

    if tau_sweep is not None:
        lines.append("-" * 90)
        lines.append("Fig. 14  τ sensitivity analysis (mean same-color ratio at equilibrium)")
        lines.append(f"{'τ':<8}{'avg_same':<16}{'pct_no_opposite':<16}")
        for row in tau_sweep["table"]:
            lines.append(f"{row['threshold']:<8.2f}"
                         f"{row['avg_same_ratio_mean']:.3f}±{row['avg_same_ratio_std']:.3f}    "
                         f"{row['pct_no_opposite_mean']:.1f}%")
    lines.append("=" * 90)
    return "\n".join(lines)


# ---------------------------------------------------------------------------
# Main entry point
# ---------------------------------------------------------------------------


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="schelling-tools reproduce",
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("--seeds", default="42,123,456,789,2024",
                        help="Comma-separated random seeds (default: 5 seeds)")
    parser.add_argument("--output-dir", default="results/paper_reproduction",
                        help="Results output directory (relative to the project root)")
    parser.add_argument("--skip-build", action="store_true",
                        help="Skip cargo build --release")
    parser.add_argument("--skip-sweep", action="store_true",
                        help="Skip the τ sensitivity analysis (Fig. 14)")
    parser.add_argument("--skip-analytic", action="store_true",
                        help="Skip analytic models (BNM + Tipping, Fig. 18-32)")
    parser.add_argument("--analytic-only", action="store_true",
                        help="Run only analytic models (skip the spatial model and τ sensitivity analysis)")
    parser.add_argument("--only", default=None,
                        help="Run only the specified experiment keys (comma-separated)")
    args = parser.parse_args(argv)

    seeds = [int(s.strip()) for s in args.seeds.split(",")]

    # Base directory with an execution timestamp.
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    base_dir = PROJECT_ROOT / args.output_dir / timestamp
    base_dir.mkdir(parents=True, exist_ok=True)

    print(f"=== Schelling (1971) paper experiment reproduction ===")
    print(f"    output: {base_dir.relative_to(PROJECT_ROOT)}")
    print(f"    seeds: {seeds}")
    print()

    if not args.skip_build:
        ensure_build()

    experiments = paper_experiments()
    analytic_exps = analytic_experiments()
    if args.only:
        wanted = {s.strip() for s in args.only.split(",")}
        experiments = [e for e in experiments if e.key in wanted]
        analytic_exps = [e for e in analytic_exps if e.key in wanted]
        if not experiments and not analytic_exps:
            print(f"Error: no keys specified by --only were found: {args.only}", file=sys.stderr)
            return 1

    if args.analytic_only:
        experiments = []

    results = []
    for exp in experiments:
        results.append(run_experiment(exp, seeds, base_dir))
        print()

    tau_sweep = None
    if not args.skip_sweep and not args.only and not args.analytic_only:
        tau_sweep = run_tau_sweep(seeds, base_dir)
        print()

    analytic_results: list[dict] = []
    if not args.skip_analytic:
        for aexp in analytic_exps:
            analytic_results.append(run_analytic_experiment(aexp, base_dir))
            print()

    # Write the report.
    report_parts = [render_comparison(results, tau_sweep)] if results or tau_sweep else []
    if analytic_results:
        report_parts.append(render_analytic_comparison(analytic_results))
    report = "\n\n".join(report_parts)
    print(report)

    # Save the summary.
    summary = {
        "timestamp": timestamp,
        "seeds": seeds,
        "experiments": results,
        "tau_sweep": tau_sweep,
        "analytic_experiments": analytic_results,
    }
    summary_path = base_dir / "reproduction_summary.json"
    with summary_path.open("w") as f:
        json.dump(summary, f, indent=2, ensure_ascii=False)
    report_path = base_dir / "reproduction_report.txt"
    with report_path.open("w") as f:
        f.write(report + "\n")

    # Results CSV (main experiments only).
    csv_path = base_dir / "reproduction_summary.csv"
    with csv_path.open("w", newline="") as f:
        writer = csv.writer(f)
        writer.writerow([
            "figure", "experiment", "rule", "n_a", "n_b", "seed",
            "final_step", "avg_same_ratio", "avg_same_ratio_a", "avg_same_ratio_b",
            "pct_no_opposite", "n_dissatisfied", "n_moved",
        ])
        for exp in results:
            for r in exp["per_seed"]:
                writer.writerow([
                    exp["figure"], exp["experiment"],
                    exp["parameters"]["rule"],
                    exp["parameters"]["n_a"], exp["parameters"]["n_b"],
                    r["seed"], r["final_step"],
                    f"{r['avg_same_ratio']:.4f}",
                    f"{r['avg_same_ratio_a']:.4f}",
                    f"{r['avg_same_ratio_b']:.4f}",
                    f"{r['pct_no_opposite']:.2f}",
                    r["n_dissatisfied"], r["n_moved"],
                ])

    print()
    print(f"Summary JSON → {summary_path.relative_to(PROJECT_ROOT)}")
    print(f"Report       → {report_path.relative_to(PROJECT_ROOT)}")
    print(f"CSV        → {csv_path.relative_to(PROJECT_ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
