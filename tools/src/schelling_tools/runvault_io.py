#!/usr/bin/env python3
"""runvault_io.py — runvault の run ディレクトリを読むための共通部品．

runvault 移行後，Rust 側の出力は次の形になった:

    <results_root>/<experiment>/<run_slug>/
    ├── run.json          ← run のメタデータ (lineage / rng / research …)
    ├── config.json       ← 封筒．実験条件は ["parameters"] の下
    ├── metrics.csv       ← long 形式 (run_uid, step, step_unit, scope, name, value)
    ├── status.json
    ├── manifest.csv
    └── artifacts/        ← 実験コードが書いた成果物 (CSV / snapshots / figures)

このモジュールは «run をどう選ぶか» と «封筒をどう開けるか» を 1 箇所に集める．
移行前の legacy レイアウト (config.json が flat，CSV が run 直下，metrics.csv が
wide 形式) も読めるようにしてあり，既存の `results/` をそのまま可視化できる．
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess

import pandas as pd

__all__ = [
    "runvault_path",
    "config_parameters",
    "artifacts_dir",
    "figures_dir",
    "load_run_meta",
    "metrics_wide",
    "run_scope_metrics",
    "sweep_children",
    "sweep_summary_table",
    "legacy_run_dir_name",
]


# --------------------------------------------------------------------------- #
# run の選び方
# --------------------------------------------------------------------------- #

def runvault_path(
    experiment: str = "schelling",
    results_root: str = "results",
    subcommand: str | None = None,
) -> str:
    """`runvault path --latest` で直近の完了 run のパスを得る．

    `subcommand` を渡すと，その subcommand の run だけが対象になる．sweep の親と
    子は同じ experiment に居るため，これを付けないと `--latest` が指標を持たない
    親を返すことがある．
    """
    if shutil.which("runvault") is None:
        raise SystemExit(
            "エラー: `runvault` コマンドが PATH にありません．\n"
            "  rs-runvault をビルドして PATH に入れるか，run ディレクトリを"
            "明示的に指定してください．\n"
            "  例: cargo install --path <rs-runvault>/crates/runvault"
        )
    cmd = [
        "runvault", "path",
        "--results-root", str(results_root),
        "--experiment", experiment,
        "--latest",
    ]
    if subcommand is not None:
        cmd += ["--subcommand", subcommand]

    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        raise SystemExit(
            f"エラー: 完了した run が見つかりません ({' '.join(cmd)})\n"
            f"  {proc.stderr.strip()}"
        )
    path = proc.stdout.strip().splitlines()
    if not path or not path[0]:
        raise SystemExit(f"エラー: runvault が空のパスを返しました ({' '.join(cmd)})")
    return path[0]


# --------------------------------------------------------------------------- #
# 封筒を開ける
# --------------------------------------------------------------------------- #

def config_parameters(run_dir: str | os.PathLike) -> dict | None:
    """run ディレクトリの `config.json` から実験条件を取り出す．

    runvault の config.json は `{schema_version, run_uid, runvault, parameters}`
    という封筒で，条件は `parameters` の下にある．legacy の flat な config.json は
    そのまま返す．config.json が無ければ None．
    """
    path = os.path.join(str(run_dir), "config.json")
    if not os.path.exists(path):
        return None
    with open(path) as f:
        doc = json.load(f)
    if isinstance(doc, dict) and "parameters" in doc:
        return doc["parameters"]
    return doc


def load_run_meta(run_dir: str | os.PathLike) -> dict | None:
    """run.json を読む．legacy の run には無いので None を返しうる．"""
    path = os.path.join(str(run_dir), "run.json")
    if not os.path.exists(path):
        return None
    with open(path) as f:
        return json.load(f)


def artifacts_dir(run_dir: str | os.PathLike) -> str:
    """**実行中に**実験コードが書いた出力の置き場 (snapshots や解析 CSV)．

    runvault の run では `artifacts/` の下．`manifest.csv` は `finish()` が
    `artifacts/` と `logs/` を歩いて確定させるので，ここに入るのは run が
    終わるまでに書かれたものだけである．**後から作る図をここに置いてはいけない**
    (ハッシュが付かず，run の記録でもない)．作図の出力先は [`figures_dir`]．

    legacy の run には run.json が無いので run 直下を返す．
    """
    run_dir = str(run_dir)
    if is_runvault_run(run_dir):
        return os.path.join(run_dir, "artifacts")
    return run_dir


def is_runvault_run(run_dir: str | os.PathLike) -> bool:
    """`run_dir` が runvault の run ディレクトリか (legacy なら False)．"""
    return os.path.exists(os.path.join(str(run_dir), "run.json"))


def figures_dir(run_dir: str | os.PathLike) -> str:
    """作図の出力先．run が終わった後に作るものは run の記録ではない．

    `<results_root>/<experiment>/figures/<run_slug>/` を返す．run ディレクトリの
    外に置くので，`finish()` が確定させた `manifest.csv` と食い違わない．
    legacy の run には run ディレクトリの外に置く根拠が無いので run 直下 (従来
    どおり `<run>/figures`) を返す．
    """
    run_dir = os.path.abspath(str(run_dir))
    if not is_runvault_run(run_dir):
        return os.path.join(run_dir, "figures")
    experiment_dir = os.path.dirname(run_dir)
    return os.path.join(experiment_dir, "figures", os.path.basename(run_dir))


# --------------------------------------------------------------------------- #
# metrics.csv
# --------------------------------------------------------------------------- #

def _is_long(df: pd.DataFrame) -> bool:
    return {"name", "value", "step"}.issubset(df.columns)


def metrics_wide(metrics_path: str | os.PathLike) -> pd.DataFrame:
    """ステップごとの指標を 1 行 1 ステップの wide 形式で返す．

    long 形式のうち step を持つ行だけを使う．step の無い行 (`scope=run` の
    `converged` / `final_iteration`) は [`run_scope_metrics`] の担当．
    legacy の wide な metrics.csv はそのまま返す．
    """
    path = str(metrics_path)
    if not os.path.exists(path):
        raise FileNotFoundError(f"metrics.csv が見つかりません: {path}")
    df = pd.read_csv(path)
    if not _is_long(df):
        return df
    stepped = df[df["step"].notna()]
    return (
        stepped.pivot_table(index="step", columns="name", values="value", aggfunc="last")
        .reset_index()
        .rename_axis(None, axis=1)
        .astype({"step": int})
        .sort_values("step")
        .reset_index(drop=True)
    )


def run_scope_metrics(metrics_path: str | os.PathLike) -> dict[str, float]:
    """run 全体を 1 つの値で表す指標 (`converged` / `final_iteration` など)．

    long 形式の step を持たない行を集める．legacy の wide な metrics.csv からは
    最終行を使って同じ意味の値を組み立てる (`converged` は残っていないので欠落)．
    """
    path = str(metrics_path)
    if not os.path.exists(path):
        raise FileNotFoundError(f"metrics.csv が見つかりません: {path}")
    df = pd.read_csv(path)
    if not _is_long(df):
        if df.empty:
            return {}
        return {"final_iteration": float(df["step"].iloc[-1])}
    rows = df[df["step"].isna()]
    return {str(r["name"]): float(r["value"]) for _, r in rows.iterrows()}


# --------------------------------------------------------------------------- #
# sweep の系譜
# --------------------------------------------------------------------------- #

def sweep_children(parent_dir: str | os.PathLike) -> list[str]:
    """sweep 親 run の子 run を，`lineage.parent_run_uid` の一致で集める．

    子は親の下ではなく experiment ディレクトリの兄弟として並ぶので，親の隣を
    走査して系譜を照合する．返り値はディレクトリ名の昇順 (＝開始時刻の昇順)．
    """
    parent = os.path.abspath(str(parent_dir))
    meta = load_run_meta(parent)
    if meta is None:
        raise FileNotFoundError(f"run.json がありません (sweep 親ではない): {parent}")
    parent_uid = meta["run_uid"]

    experiment_dir = os.path.dirname(parent)
    children: list[str] = []
    for name in sorted(os.listdir(experiment_dir)):
        path = os.path.join(experiment_dir, name)
        if path == parent or not os.path.isdir(path) or os.path.islink(path):
            continue
        child = load_run_meta(path)
        if child is None:
            continue
        lineage = child.get("lineage") or {}
        if lineage.get("parent_run_uid") == parent_uid:
            children.append(path)
    return children


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
        params = config_parameters(child) or {}
        metrics_path = os.path.join(child, "metrics.csv")
        wide = metrics_wide(metrics_path)
        last = wide.iloc[-1]
        scoped = run_scope_metrics(metrics_path)
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
