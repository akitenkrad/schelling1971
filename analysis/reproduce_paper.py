"""Schelling (1971) の2次元チェッカーボードモデル（Figure 7-14）を再現するスクリプト．

論文の主要実験を既存のRustバイナリ (`cargo run --release`) に対して実行し，
最終メトリクスを論文報告値と比較して表とJSONに保存する．

Usage:
    uv run python analysis/reproduce_paper.py
    uv run python analysis/reproduce_paper.py --seeds 42,123,456,789,2024
    uv run python analysis/reproduce_paper.py --skip-build  # cargo buildをスキップ
    uv run python analysis/reproduce_paper.py --only fig11  # 特定実験のみ実行

再現対象:
    Fig. 11 : τ=1/3, 等数,   13×16, 30%空き — 平均同色比率 ≈ 65-75%
    Fig. 9  : τ=1/2, 等数,   13×16, 30%空き — 平均同色比率 ≈ 80-83%
    Fig. 8  : τ=1/2 (厳格運用の近似として複数シードで試行) — 89-91%
    Fig. 12 : τ=1/3, 不等数 2:1, 13×16, 30%空き — 少数派 > 80%
    Fig. 14 : τ感度解析 (0.10-0.60, 0.05刻み)
    Fig. 16 : 集会選好 (同色絶対数 ≥ 3) — 平均同色比率 ≈ 75%, 異色近隣なし ≈ 38%
    Fig. 17 : 統合選好 (同色絶対数 3-6) — 分離度は穏やかだが dead space が形成される
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

# ---------------------------------------------------------------------------
# 実験定義
# ---------------------------------------------------------------------------


@dataclass
class Experiment:
    """単一の論文実験に対応するパラメータ設定と期待値．"""

    key: str
    figure: str
    description: str
    rows: int = 13
    cols: int = 16
    vacant_rate: float = 0.30
    # 満足判定ルール: None のときは threshold から ratio ルールを構築する．
    # 例: "ratio:0.333" / "min-same:3" / "bounded:3:6"
    rule: str | None = None
    threshold: float = 1.0 / 3.0
    # エージェント数（0なら vacant_rate から自動計算で等数）
    n_a: int = 0
    n_b: int = 0
    # 論文報告値 (参照用，比較表示に使う)
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
        if self.n_a > 0 and self.n_b > 0:
            args += ["--n-a", str(self.n_a), "--n-b", str(self.n_b)]
        return args

    def rule_label(self) -> str:
        if self.rule is not None:
            return self.rule
        return f"ratio:{self.threshold:.3f}"


def paper_experiments() -> list[Experiment]:
    # 13×16 = 208 セル, 約30%空き = 62 空き, 146 エージェント
    # 2:1 不等数 → 97:49 (146合計)
    return [
        Experiment(
            key="fig11_tau_one_third",
            figure="Fig. 11",
            description="τ=1/3, 等数, ランダム初期配置 (主実験)",
            threshold=1.0 / 3.0,
            paper_avg_same_ratio=(0.65, 0.75),
            paper_pct_no_opposite=(35.0, 45.0),
        ),
        Experiment(
            key="fig09_tau_one_half_lenient",
            figure="Fig. 9",
            description="τ=1/2, 等数 (緩い運用)",
            threshold=0.5,
            paper_avg_same_ratio=(0.80, 0.83),
            paper_pct_no_opposite=(38.0, 42.0),
        ),
        Experiment(
            key="fig08_tau_one_half_strict",
            figure="Fig. 8",
            description="τ=1/2, 等数 (厳格運用の近似 — シード分散で上限を探索)",
            threshold=0.5,
            paper_avg_same_ratio=(0.89, 0.91),
            paper_pct_no_opposite=(65.0, 70.0),
        ),
        Experiment(
            key="fig12_unequal_two_to_one",
            figure="Fig. 12",
            description="τ=1/3, 不等数 2:1 (A:97, B:49)",
            threshold=1.0 / 3.0,
            n_a=97,
            n_b=49,
            paper_avg_same_ratio=(0.70, 0.85),
            paper_minority_avg_same=(0.80, 1.00),
        ),
        Experiment(
            key="fig16_congregationist_min_same_3",
            figure="Fig. 16",
            description="集会選好: 同色絶対数 ≥ 3 (比率不問)",
            rule="min-same:3",
            paper_avg_same_ratio=(0.70, 0.80),
            paper_pct_no_opposite=(35.0, 42.0),
        ),
        Experiment(
            key="fig17_integrationist_bounded_3_6",
            figure="Fig. 17",
            description=(
                "統合選好: 同色絶対数 3-6 (上下限あり) — "
                "論文は定量値を示さず「dead space 形成」「収束困難」を定性的に報告"
            ),
            rule="bounded:3:6",
        ),
    ]


def tau_sweep_taus() -> list[float]:
    # Fig. 14 相当: τ=0.10, 0.15, ..., 0.60 (13点)
    return [round(0.10 + 0.05 * i, 2) for i in range(11)]


# ---------------------------------------------------------------------------
# Rustバイナリ呼び出し
# ---------------------------------------------------------------------------


PROJECT_ROOT = Path(__file__).resolve().parent.parent


def ensure_build() -> None:
    print("=== cargo build --release ===")
    subprocess.run(
        ["cargo", "build", "--release"],
        cwd=PROJECT_ROOT,
        check=True,
    )


def run_cargo(args: list[str], cwd: Path) -> None:
    """cargoサブプロセスを起動し，失敗したら例外を投げる．"""
    subprocess.run(args, cwd=cwd, check=True, stdout=subprocess.DEVNULL)


def read_final_metrics(output_dir: Path) -> dict:
    """cargo run が作成したタイムスタンプ付きサブディレクトリの metrics.csv を読む．"""
    # output_dir 配下の一番新しい (タイムスタンプ名の) サブディレクトリを探す
    candidates = sorted(
        [p for p in output_dir.iterdir() if p.is_dir() and p.name != "latest"],
        key=lambda p: p.stat().st_mtime,
    )
    if not candidates:
        raise FileNotFoundError(f"metrics.csv が見つかりません: {output_dir}")
    run_dir = candidates[-1]
    metrics_path = run_dir / "metrics.csv"
    with metrics_path.open() as f:
        reader = csv.DictReader(f)
        rows = list(reader)
    if not rows:
        raise ValueError(f"空の metrics.csv: {metrics_path}")
    final = rows[-1]
    initial = rows[0]
    return {
        "run_dir": str(run_dir.relative_to(PROJECT_ROOT)),
        "final_step": int(final["step"]),
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
# 実験ランナー
# ---------------------------------------------------------------------------


def run_experiment(exp: Experiment, seeds: list[int], base_dir: Path) -> dict:
    """1つの実験設定を複数シードで実行し，集計結果を返す．"""
    exp_dir = base_dir / exp.key
    exp_dir.mkdir(parents=True, exist_ok=True)

    print(f"--- {exp.figure}: {exp.description} ---")
    print(f"    ルール={exp.rule_label()} | グリッド={exp.rows}×{exp.cols} "
          f"| A:B={'auto' if exp.n_a == 0 else f'{exp.n_a}:{exp.n_b}'} "
          f"| 空き率={exp.vacant_rate:.2f} | seeds={seeds}")

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

    # シード間の平均と標準偏差を集計
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


def run_tau_sweep(seeds: list[int], base_dir: Path) -> dict:
    """Fig. 14 相当: τ=0.10-0.60 でスイープを実行し，均衡同色比率の非線形性を再現する．"""
    sweep_dir = base_dir / "fig14_tau_sweep"
    sweep_dir.mkdir(parents=True, exist_ok=True)

    taus = tau_sweep_taus()
    print(f"--- Fig. 14: τ感度解析 (τ={taus[0]:.2f}-{taus[-1]:.2f}) ---")

    seeds_str = ",".join(str(s) for s in seeds)
    # start:stop:step形式で cargo sweep を呼ぶ
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

    # 生成された sweep_summary.csv を読む
    sweep_outputs = sorted(
        [p for p in sweep_dir.iterdir() if p.is_dir() and p.name.endswith("_sweep")],
        key=lambda p: p.stat().st_mtime,
    )
    summary_path = sweep_outputs[-1] / "sweep_summary.csv"
    with summary_path.open() as f:
        rows = list(csv.DictReader(f))

    # τごとに集計
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
        "description": "τ感度解析: 0.35-0.50 で均衡同色比率が急上昇",
        "taus": taus,
        "seeds": seeds,
        "table": table,
        "sweep_dir": str(sweep_outputs[-1].relative_to(PROJECT_ROOT)),
    }


# ---------------------------------------------------------------------------
# レポート生成
# ---------------------------------------------------------------------------


def _format_range(rng: tuple[float, float] | None, unit: str = "") -> str:
    """レンジを "min — max" 形式で整形する．値はそのまま表示する（単位変換しない）．"""
    if rng is None:
        return "-"
    if unit == "%":
        return f"{rng[0]:.1f}% — {rng[1]:.1f}%"
    return f"{rng[0]:.3f} — {rng[1]:.3f}"


def _in_range(value: float, rng: tuple[float, float] | None) -> str:
    if rng is None:
        return ""
    return "✓" if rng[0] <= value <= rng[1] else "✗"


def render_comparison(experiments: list[dict], tau_sweep: dict | None) -> str:
    lines = []
    lines.append("=" * 90)
    lines.append("Schelling (1971) 論文再現結果  vs  論文報告値")
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
        # 不等数の場合は少数派集団の平均同色比率も表示
        if ref["minority_avg_same"] is not None:
            params = exp["parameters"]
            if params["n_a"] > params["n_b"]:
                minority_key = "avg_same_ratio_b"
            else:
                minority_key = "avg_same_ratio_a"
            minority_mean = agg[minority_key]["mean"]
            in_range = _in_range(minority_mean, ref["minority_avg_same"])
            paper_minority = _format_range(ref["minority_avg_same"])
            lines.append(f"{'':10}少数派同色比率: {minority_mean:.3f} {in_range} "
                         f"(paper: {paper_minority})")

    if tau_sweep is not None:
        lines.append("-" * 90)
        lines.append("Fig. 14  τ感度解析（均衡時平均同色比率）")
        lines.append(f"{'τ':<8}{'avg_same':<16}{'pct_no_opposite':<16}")
        for row in tau_sweep["table"]:
            lines.append(f"{row['threshold']:<8.2f}"
                         f"{row['avg_same_ratio_mean']:.3f}±{row['avg_same_ratio_std']:.3f}    "
                         f"{row['pct_no_opposite_mean']:.1f}%")
    lines.append("=" * 90)
    return "\n".join(lines)


# ---------------------------------------------------------------------------
# メイン
# ---------------------------------------------------------------------------


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--seeds", default="42,123,456,789,2024",
                        help="カンマ区切りの乱数シード (デフォルト: 5個)")
    parser.add_argument("--output-dir", default="results/paper_reproduction",
                        help="結果出力先 (プロジェクトルートからの相対パス)")
    parser.add_argument("--skip-build", action="store_true",
                        help="cargo build --release をスキップ")
    parser.add_argument("--skip-sweep", action="store_true",
                        help="τ感度解析 (Fig. 14) をスキップ")
    parser.add_argument("--only", default=None,
                        help="指定したexperiment keyのみ実行 (カンマ区切り可)")
    args = parser.parse_args()

    seeds = [int(s.strip()) for s in args.seeds.split(",")]

    # 実行タイムスタンプ付きベースディレクトリ
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    base_dir = PROJECT_ROOT / args.output_dir / timestamp
    base_dir.mkdir(parents=True, exist_ok=True)

    print(f"=== Schelling (1971) 論文実験再現 ===")
    print(f"    出力先: {base_dir.relative_to(PROJECT_ROOT)}")
    print(f"    シード: {seeds}")
    print()

    if not args.skip_build:
        ensure_build()

    experiments = paper_experiments()
    if args.only:
        wanted = {s.strip() for s in args.only.split(",")}
        experiments = [e for e in experiments if e.key in wanted]
        if not experiments:
            print(f"エラー: --only で指定されたキーが見つかりません: {args.only}", file=sys.stderr)
            return 1

    results = []
    for exp in experiments:
        results.append(run_experiment(exp, seeds, base_dir))
        print()

    tau_sweep = None
    if not args.skip_sweep and not args.only:
        tau_sweep = run_tau_sweep(seeds, base_dir)
        print()

    # レポート出力
    report = render_comparison(results, tau_sweep)
    print(report)

    # サマリ保存
    summary = {
        "timestamp": timestamp,
        "seeds": seeds,
        "experiments": results,
        "tau_sweep": tau_sweep,
    }
    summary_path = base_dir / "reproduction_summary.json"
    with summary_path.open("w") as f:
        json.dump(summary, f, indent=2, ensure_ascii=False)
    report_path = base_dir / "reproduction_report.txt"
    with report_path.open("w") as f:
        f.write(report + "\n")

    # 結果CSV (主要実験のみ)
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
    print(f"サマリJSON → {summary_path.relative_to(PROJECT_ROOT)}")
    print(f"レポート   → {report_path.relative_to(PROJECT_ROOT)}")
    print(f"CSV        → {csv_path.relative_to(PROJECT_ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
