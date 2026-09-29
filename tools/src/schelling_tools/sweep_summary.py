#!/usr/bin/env python3
"""A sweep table with one row per condition.

The mechanics of reading run directories are in `runvault.read`. Only the
Schelling model-specific parts remain here: the table columns (`threshold` /
`vacant_rate` / `dissimilarity_index` ...) and the child run directory names
used by sweeps before the runvault migration. Both concern the model in this
paper rather than how run directories are read, so they do not belong in the
shared components.
"""
from __future__ import annotations

import os

import pandas as pd
from runvault.read import (
    artifacts_dir,
    config_parameters,
    metrics_wide,
    scope_metrics_from_csv,
    sweep_children,
)

__all__ = ["legacy_run_dir_name", "sweep_summary_table"]


def legacy_run_dir_name(threshold: float, vacant_rate: float, seed: int) -> str:
    """Return the child run directory name used before the runvault migration."""
    return f"tau_{threshold:.3f}_vac_{vacant_rate:.3f}_seed_{seed}"


def sweep_summary_table(sweep_dir: str | os.PathLike) -> pd.DataFrame:
    """Build a summary table with one row per condition.

    In runvault, this table does not exist as a file. Collect the child runs of
    the sweep parent (where `lineage.parent_run_uid` is the parent's `run_uid`)
    and rebuild it from each child's `parameters` in `config.json` and final
    values in `metrics.csv`. Legacy sweeps have `sweep_summary.csv`, so read it.

    Both paths add a `snapshots_dir` column, so callers do not need to compose
    directory names from the conditions.
    """
    sweep_dir = str(sweep_dir)
    legacy = os.path.join(sweep_dir, "sweep_summary.csv")
    if os.path.exists(legacy):
        df = pd.read_csv(legacy)
        df["snapshots_dir"] = [
            os.path.join(
                sweep_dir,
                legacy_run_dir_name(r.threshold, r.vacant_rate, int(r.seed)),
                "snapshots",
            )
            for r in df.itertuples()
        ]
        return df

    children = sweep_children(sweep_dir)
    if not children:
        raise SystemExit(
            f"Error: no child runs associated with this sweep parent were found: {sweep_dir}\n"
            "  Child runs identify their parent through lineage.parent_run_uid."
            "Verify that the parent and child runs are under the same results root."
        )

    rows: list[dict] = []
    for child in children:
        params = config_parameters(child, required=False) or {}
        metrics_path = os.path.join(child, "metrics.csv")
        wide = metrics_wide(metrics_path)
        last = wide.iloc[-1]
        scoped = scope_metrics_from_csv(metrics_path)
        rows.append({
            "threshold": params.get("threshold"),
            "vacant_rate": params.get("vacant_rate"),
            "rows": params.get("rows"),
            "cols": params.get("cols"),
            "seed": params.get("seed"),
            "converged": bool(scoped.get("converged", 0.0)),
            "final_iteration": int(scoped.get("final_iteration", last["step"])),
            "avg_same_ratio": float(last["avg_same_ratio"]),
            "avg_same_ratio_a": float(last["avg_same_ratio_a"]),
            "avg_same_ratio_b": float(last["avg_same_ratio_b"]),
            "pct_no_opposite": float(last["pct_no_opposite"]),
            "dissimilarity_index": float(last["dissimilarity_index"]),
            "n_dissatisfied_final": int(last["n_dissatisfied"]),
            "n_moved_final": int(last["n_moved"]),
            "snapshots_dir": os.path.join(artifacts_dir(child), "snapshots"),
        })
    return (
        pd.DataFrame(rows)
        .sort_values(["threshold", "vacant_rate", "seed"])
        .reset_index(drop=True)
    )
