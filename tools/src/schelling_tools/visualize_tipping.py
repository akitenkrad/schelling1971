#!/usr/bin/env python3
"""
visualize_tipping.py — Schelling (1971) ティッピングモデル可視化．

BNM 可視化に加え，tipping_classification.json を読み込んで
in-tipping/out-tipping の有無を図中に注釈表示する．
"""
from __future__ import annotations

import argparse
import json
import os

import matplotlib.pyplot as plt

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
    path = os.path.join(results_dir, "tipping_classification.json")
    if not os.path.exists(path):
        return None
    with open(path) as f:
        return json.load(f)


def annotate_classification(output_path: str, classification: dict) -> None:
    """既存の reaction_curves.png または phase_portrait.png にラベル領域を重畳しない代わりに，
    分類サマリを別ファイルとしてテキスト併載するシンプルな表示．
    """
    fig, ax = plt.subplots(figsize=(7, 4))
    ax.axis("off")
    label = classification.get("type", "(unknown)")
    aw = classification.get("all_white_stable", None)
    mx = classification.get("mixed_stable_exists", None)
    text = (
        f"ティッピング類型: {label}\n\n"
        f"  全W端点が安定: {aw}\n"
        f"  安定混合均衡が存在: {mx}\n\n"
        f"類型解釈:\n"
        f"  in_tipping_only   — B 反応曲線が全W点を覆う + 安定混合あり (B が流入し混合へ)\n"
        f"  out_tipping_only  — 全W安定 + 安定混合なし (B が閾値超過で W が連鎖退出)\n"
        f"  both              — 上記両方の経路が存在 (典型的ホワイトフライト)\n"
        f"  neither           — 端点も混合も全て安定 (頑健な多相安定)\n"
    )
    ax.text(0.02, 0.5, text, fontsize=11, family="Hiragino Sans", verticalalignment="center")
    fig.tight_layout()
    fig.savefig(output_path, dpi=150)
    plt.close(fig)


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(
        prog="schelling-tools visualize-tipping",
        description="ティッピングモデル可視化 (BNM 可視化 + 分類注釈)",
    )
    parser.add_argument("--results_dir", default=None)
    parser.add_argument("--output_dir", default=None)
    args = parser.parse_args(argv)

    results_dir = resolve_results_dir(args.results_dir)
    output_dir = args.output_dir or os.path.join(results_dir, "figures")
    os.makedirs(output_dir, exist_ok=True)

    print(f"[visualize-tipping] 入力: {results_dir}")
    print(f"[visualize-tipping] 出力: {output_dir}")

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

    print(f"[visualize-tipping] 生成完了: {len(figures)} 枚")
    for f in figures:
        print(f"  - {output_dir}/{f}")


if __name__ == "__main__":
    main()
