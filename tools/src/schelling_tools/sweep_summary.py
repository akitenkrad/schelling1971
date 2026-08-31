#!/usr/bin/env python3
"""スイープの «1 行 1 条件» の表．

run ディレクトリの読み方そのものは `runvault.read` にある．ここに残るのは
Schelling モデル固有の部分だけ — どの列を持つ表なのか (`threshold` /
`vacant_rate` / `dissimilarity_index` …) と，runvault 移行前のスイープが使って
いた子 run のディレクトリ名である．どちらもこの論文のモデルの話であって，
run ディレクトリの読み方ではないので，共通部品には置かない．
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
    """runvault 移行前の sweep が使っていた子 run のディレクトリ名．"""
    return f"tau_{threshold:.3f}_vac_{vacant_rate:.3f}_seed_{seed}"


def sweep_summary_table(sweep_dir: str | os.PathLike) -> pd.DataFrame:
    """1 行 1 条件のサマリ表を用意する．

    runvault ではこの表はファイルとして存在しない．sweep 親の子 run
    (`lineage.parent_run_uid` が親の `run_uid`) を集め，各子の `config.json` の
    `parameters` と `metrics.csv` の最終値から組み直す．legacy のスイープには
    `sweep_summary.csv` があるのでそれを読む．

    どちらの経路でも `snapshots_dir` 列を付けるので，呼び出し側は条件から
    ディレクトリ名を composing しなくてよい．
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
            f"エラー: この sweep 親に紐づく子 run が見つかりません: {sweep_dir}\n"
            "  子 run は lineage.parent_run_uid で親を指します．"
            "親と子が同じ results ルートにあるか確認してください．"
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
