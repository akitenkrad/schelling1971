"""schelling-tools show-experiment-settings — 実験設定値の表示．

2 つの表示モードを持つ:

1. 論文再現実験定義の表示 (デフォルト)
   `reproduce_paper.py` の `paper_experiments()` で定義された Fig.7-17 の実験設定と
   論文報告値の参照範囲を表形式で表示する．`reproduce` 実行前のプレビュー用．

2. 既存実行結果の設定表示 (`--results-dir <path>`)
   results/{timestamp}/config.json (run) または
   results/{timestamp}_sweep/sweep_config.json (sweep) を読み，
   実行時に使われた全パラメータを表示する．`results/latest` も解決される．

Usage:
    schelling-tools show-experiment-settings
    schelling-tools show-experiment-settings --only fig11_tau_one_third
    schelling-tools show-experiment-settings --json
    schelling-tools show-experiment-settings --results-dir results/latest
    schelling-tools show-experiment-settings --results-dir results/20260425_153000
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from dataclasses import asdict
from pathlib import Path

from schelling_tools.reproduce_paper import (
    PROJECT_ROOT,
    Experiment,
    paper_experiments,
    tau_sweep_taus,
)


# ---------------------------------------------------------------------------
# 1. 論文実験定義の表示
# ---------------------------------------------------------------------------


def _format_range(rng: tuple[float, float] | None, unit: str = "") -> str:
    if rng is None:
        return "-"
    if unit == "%":
        return f"{rng[0]:.1f}–{rng[1]:.1f}%"
    return f"{rng[0]:.3f}–{rng[1]:.3f}"


def _agents_label(exp: Experiment) -> str:
    if exp.n_a > 0 and exp.n_b > 0:
        return f"A:{exp.n_a} / B:{exp.n_b}"
    total = exp.rows * exp.cols
    n_vacant = round(total * exp.vacant_rate)
    n_agents = total - n_vacant
    a = n_agents // 2
    b = n_agents - a
    return f"A:{a} / B:{b} (auto)"


def render_paper_experiments(exps: list[Experiment]) -> str:
    lines: list[str] = []
    lines.append("=" * 90)
    lines.append("Schelling (1971) 論文再現実験 — 設定値一覧")
    lines.append("=" * 90)
    for i, exp in enumerate(exps):
        if i > 0:
            lines.append("-" * 90)
        lines.append(f"[{exp.key}]  {exp.figure}")
        lines.append(f"    説明        : {exp.description}")
        lines.append(f"    ルール      : {exp.rule_label()}")
        lines.append(f"    グリッド    : {exp.rows}×{exp.cols} (空き率 {exp.vacant_rate:.2f})")
        lines.append(f"    エージェント: {_agents_label(exp)}")
        lines.append(f"    論文報告値:")
        lines.append(f"      avg_same_ratio   : {_format_range(exp.paper_avg_same_ratio)}")
        lines.append(f"      pct_no_opposite  : {_format_range(exp.paper_pct_no_opposite, unit='%')}")
        if exp.paper_minority_avg_same is not None:
            lines.append(f"      minority_avg_same: {_format_range(exp.paper_minority_avg_same)}")
    lines.append("-" * 90)
    taus = tau_sweep_taus()
    lines.append(
        f"[fig14_tau_sweep]  Fig. 14 — τ感度解析 ({taus[0]:.2f}–{taus[-1]:.2f}, 0.05 刻み, {len(taus)} 点)"
    )
    lines.append("=" * 90)
    return "\n".join(lines)


def experiments_as_dicts(exps: list[Experiment]) -> list[dict]:
    """JSON 出力用に Experiment を dict に変換する．"""
    out: list[dict] = []
    for exp in exps:
        d = asdict(exp)
        d["rule_label"] = exp.rule_label()
        d["agents"] = _agents_label(exp)
        out.append(d)
    return out


# ---------------------------------------------------------------------------
# 2. 実行結果ディレクトリの設定表示
# ---------------------------------------------------------------------------


def _resolve_results_dir(arg: str) -> Path:
    """ユーザ指定の results_dir を絶対パスに解決する．

    - 絶対パスならそのまま
    - 相対パスは PROJECT_ROOT 起点で解決し，存在しなければ CWD 起点でも試す
    - シンボリックリンク (results/latest) は os.path.realpath で実体を解決
    """
    p = Path(arg)
    if not p.is_absolute():
        candidates = [PROJECT_ROOT / arg, Path.cwd() / arg]
        for c in candidates:
            if c.exists():
                p = c
                break
        else:
            p = candidates[0]
    return Path(os.path.realpath(p))


def _find_config_file(results_dir: Path) -> tuple[Path, str]:
    """results_dir 配下から config.json (run) か sweep_config.json (sweep) を探す．"""
    run_cfg = results_dir / "config.json"
    sweep_cfg = results_dir / "sweep_config.json"
    if run_cfg.exists():
        return run_cfg, "run"
    if sweep_cfg.exists():
        return sweep_cfg, "sweep"
    raise FileNotFoundError(
        f"設定ファイルが見つかりません: {results_dir}\n"
        f"  期待されるファイル: config.json (run) または sweep_config.json (sweep)\n"
        f"  注: 旧バージョンで生成された結果には config.json が含まれていない場合があります．"
    )


def render_run_config(cfg: dict, source: Path) -> str:
    lines: list[str] = []
    lines.append("=" * 90)
    lines.append("実行設定 (run)")
    lines.append("=" * 90)
    lines.append(f"設定ファイル: {source}")
    lines.append("-" * 90)
    lines.append(f"ルール       : {cfg.get('rule', '-')}  (kind={cfg.get('rule_kind', '-')})")
    if cfg.get("threshold") is not None:
        lines.append(f"  threshold  : {cfg['threshold']}")
    if cfg.get("min_same") is not None:
        lines.append(f"  min_same   : {cfg['min_same']}")
    if cfg.get("max_same") is not None:
        lines.append(f"  max_same   : {cfg['max_same']}")
    rows = cfg.get("rows", "-")
    cols = cfg.get("cols", "-")
    n_vacant = cfg.get("n_vacant", "-")
    lines.append(f"グリッド     : {rows}×{cols} (空き {n_vacant} セル / 空き率 {cfg.get('vacant_rate', '-')})")
    lines.append(f"エージェント : A={cfg.get('n_a', '-')}  B={cfg.get('n_b', '-')}")
    lines.append(f"シード       : {cfg.get('seed', '-')}")
    lines.append(f"最大反復     : {cfg.get('max_iterations', '-')}")
    lines.append(f"snapshot間隔 : {cfg.get('snapshot_interval', '-')}")
    lines.append(f"出力先       : {cfg.get('output_dir', '-')}")
    lines.append("=" * 90)
    return "\n".join(lines)


def render_sweep_config(cfg: dict, source: Path) -> str:
    lines: list[str] = []
    lines.append("=" * 90)
    lines.append("実行設定 (sweep)")
    lines.append("=" * 90)
    lines.append(f"設定ファイル: {source}")
    lines.append("-" * 90)

    def fmt_range(v) -> str:
        if isinstance(v, dict) and {"start", "stop", "step"} <= v.keys():
            return f"{v['start']}:{v['stop']}:{v['step']}  (range)"
        return f"{v}  (single)"

    lines.append(f"threshold    : {fmt_range(cfg.get('threshold'))}")
    lines.append(f"vacant_rate  : {fmt_range(cfg.get('vacant_rate'))}")
    lines.append(f"グリッド     : {cfg.get('rows', '-')}×{cfg.get('cols', '-')}")
    lines.append(f"シード       : {cfg.get('seeds', '-')}")
    lines.append(f"最大反復     : {cfg.get('max_iterations', '-')}")
    lines.append(f"snapshot間隔 : {cfg.get('snapshot_interval', '-')}")
    lines.append("=" * 90)
    return "\n".join(lines)


# ---------------------------------------------------------------------------
# メイン
# ---------------------------------------------------------------------------


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="schelling-tools show-experiment-settings",
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "--results-dir", "--results_dir",
        default=None,
        help=(
            "実行結果ディレクトリを指定し，その config.json / sweep_config.json を表示する．"
            "未指定時は論文再現実験定義の一覧を表示．"
        ),
    )
    parser.add_argument(
        "--only",
        default=None,
        help="論文再現実験のうち指定キーのみ表示 (カンマ区切り可)．--results-dir 指定時は無視．",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="表ではなく JSON 形式で出力する．",
    )
    args = parser.parse_args(argv)

    if args.results_dir is not None:
        results_dir = _resolve_results_dir(args.results_dir)
        if not results_dir.exists():
            print(f"エラー: ディレクトリが存在しません: {results_dir}", file=sys.stderr)
            return 1
        cfg_path, kind = _find_config_file(results_dir)
        with cfg_path.open() as f:
            cfg = json.load(f)
        if args.json:
            payload = {"source": str(cfg_path), "kind": kind, "config": cfg}
            print(json.dumps(payload, indent=2, ensure_ascii=False))
        else:
            if kind == "run":
                print(render_run_config(cfg, cfg_path))
            else:
                print(render_sweep_config(cfg, cfg_path))
        return 0

    exps = paper_experiments()
    if args.only:
        wanted = {s.strip() for s in args.only.split(",")}
        exps = [e for e in exps if e.key in wanted]
        if not exps:
            print(f"エラー: --only で指定されたキーが見つかりません: {args.only}", file=sys.stderr)
            return 1

    if args.json:
        payload = {
            "experiments": experiments_as_dicts(exps),
            "tau_sweep": {
                "key": "fig14_tau_sweep",
                "figure": "Fig. 14",
                "taus": tau_sweep_taus(),
            },
        }
        print(json.dumps(payload, indent=2, ensure_ascii=False))
    else:
        print(render_paper_experiments(exps))
    return 0


if __name__ == "__main__":
    sys.exit(main())
