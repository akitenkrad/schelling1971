#!/usr/bin/env python3
"""
visualize_bnm.py — Schelling (1971) 境界近隣モデル (BNM) 可視化スクリプト

Usage:
    schelling-tools visualize-bnm [--results_dir RESULTS_DIR] [--output_dir OUTPUT_DIR]

Inputs (results_dir 配下に置かれる Rust 出力):
    config.json
    tolerance_w.csv / tolerance_b.csv          # CDF (R, F(R))
    reaction_curve_w.csv / reaction_curve_b.csv  # (own, max_other)
    equilibria.csv                              # (w, b, kind, stability)
    vector_field.csv                            # (w, b, dw_sign, db_sign, region)
    trajectory.csv                              # (t, w, b)  (bnm 単発のみ)
    basin.csv                                   # (w0, b0, ..., converged_kind)  (bnm-basin のみ)

Outputs (output_dir):
    tolerance_schedules.png
    reaction_curves.png
    phase_portrait.png
    trajectory.png        (trajectory.csv がある場合)
    basin_of_attraction.png  (basin.csv がある場合)
"""
from __future__ import annotations

import argparse
import json
import os
import sys

import matplotlib as mpl
import matplotlib.patches as mpatches
import matplotlib.pyplot as plt
import numpy as np
import pandas as pd

# 日本語フォント
plt.rcParams["font.family"] = "Hiragino Sans"

# 色設定
COLOR_W_CURVE = "#1f77b4"   # 白人反応曲線 (青)
COLOR_B_CURVE = "#d62728"   # 黒人反応曲線 (赤)
COLOR_TRAJECTORY = "#2ca02c"  # 軌跡 (緑)
COLOR_INITIAL = "#ff7f0e"   # 初期点 (橙)
COLOR_CAPACITY = "#7f7f7f"  # 容量制約線 (灰)

EQUILIBRIUM_COLORS = {
    "all_white": COLOR_W_CURVE,
    "all_black": COLOR_B_CURVE,
    "mixed": "#9467bd",
    "empty": "#7f7f7f",
}

# basin 用カラーマップ
BASIN_COLORS = {
    "all_white": COLOR_W_CURVE,
    "all_black": COLOR_B_CURVE,
    "mixed": "#9467bd",
    "empty": "#bcbd22",
    "none": "#cccccc",
}


# --------------------------------------------------------------------------- #
# I/O
# --------------------------------------------------------------------------- #

def _extract_phase(cfg: dict | None) -> dict | None:
    """config.json から phase 情報を取り出す．BNM は cfg["phase"]，
    Tipping は cfg["config"]["phase"] にある．"""
    if cfg is None:
        return None
    if "phase" in cfg:
        return cfg["phase"]
    if "config" in cfg and isinstance(cfg["config"], dict) and "phase" in cfg["config"]:
        return cfg["config"]["phase"]
    return None


def load_artifacts(results_dir: str) -> dict:
    """BNM 出力の各 CSV / JSON を読み込んで dict で返す．"""
    out: dict = {}
    config_path = os.path.join(results_dir, "config.json")
    if os.path.exists(config_path):
        with open(config_path) as f:
            out["config"] = json.load(f)
    else:
        out["config"] = None
    out["phase"] = _extract_phase(out["config"])

    for name in [
        "tolerance_w",
        "tolerance_b",
        "reaction_curve_w",
        "reaction_curve_b",
        "equilibria",
        "vector_field",
        "trajectory",
        "basin",
    ]:
        path = os.path.join(results_dir, f"{name}.csv")
        out[name] = pd.read_csv(path) if os.path.exists(path) else None
    return out


# --------------------------------------------------------------------------- #
# プロット
# --------------------------------------------------------------------------- #

def plot_tolerance_schedules(art: dict, output_path: str) -> None:
    """許容限界スケジュール F(R) の CDF プロット．"""
    fig, ax = plt.subplots(figsize=(8, 5))
    if art["tolerance_w"] is not None:
        df = art["tolerance_w"]
        ax.plot(df["r"], df["f_r"], color=COLOR_W_CURVE, linewidth=2, label="W: $F_W(R)$")
    if art["tolerance_b"] is not None:
        df = art["tolerance_b"]
        ax.plot(df["r"], df["f_r"], color=COLOR_B_CURVE, linewidth=2, label="B: $F_B(R)$")
    ax.set_xlabel("許容比率 R (異色 / 自色)")
    ax.set_ylabel("F(R) — 許容限界がR以下の人数")
    ax.set_title("許容限界スケジュール (CDF)")
    ax.grid(True, alpha=0.3)
    ax.legend()
    fig.tight_layout()
    fig.savefig(output_path, dpi=150)
    plt.close(fig)


def plot_reaction_curves(art: dict, output_path: str) -> None:
    """位相平面 (W, B) 上の反応曲線．"""
    fig, ax = plt.subplots(figsize=(7, 7))
    if art["reaction_curve_w"] is not None:
        df = art["reaction_curve_w"]
        ax.plot(df["own"], df["max_other"], color=COLOR_W_CURVE, linewidth=2,
                label="$B_W(W)$ — W の反応曲線")
    if art["reaction_curve_b"] is not None:
        df = art["reaction_curve_b"]
        # B の反応曲線は (own=B, max_other=W_B(B))，W軸/B軸を入替えてプロット
        ax.plot(df["max_other"], df["own"], color=COLOR_B_CURVE, linewidth=2,
                label="$W_B(B)$ — B の反応曲線")

    # 容量制約線
    phase = art.get("phase")
    if phase and phase.get("capacity") is not None:
        c = phase["capacity"]
        x = np.linspace(0, c, 100)
        ax.plot(x, c - x, color=COLOR_CAPACITY, linestyle="--", linewidth=1,
                label=f"容量制約 W+B={c:.0f}")

    # 平衡点
    if art["equilibria"] is not None:
        for _, row in art["equilibria"].iterrows():
            color = EQUILIBRIUM_COLORS.get(row["kind"], "#000000")
            marker = "o" if row["stability"] == "stable" else "x"
            size = 200 if row["stability"] == "stable" else 150
            ax.scatter(row["w"], row["b"], c=color, marker=marker, s=size,
                       edgecolors="black", linewidths=1.2, zorder=5)

    ax.set_xlabel("W (白人数)")
    ax.set_ylabel("B (黒人数)")
    ax.set_title("反応曲線と平衡点")
    ax.set_aspect("equal", adjustable="box")
    ax.grid(True, alpha=0.3)

    # 凡例 (平衡点凡例を追加)
    handles, labels = ax.get_legend_handles_labels()
    handles.append(plt.Line2D([], [], marker="o", color="w", markeredgecolor="black",
                               markerfacecolor="gray", markersize=10, label="安定均衡"))
    handles.append(plt.Line2D([], [], marker="x", color="black", linestyle="None",
                               markersize=10, label="不安定均衡"))
    ax.legend(handles=handles, loc="upper right")

    fig.tight_layout()
    fig.savefig(output_path, dpi=150)
    plt.close(fig)


def plot_phase_portrait(art: dict, output_path: str) -> None:
    """ベクトル場 + 反応曲線 + 平衡点．"""
    fig, ax = plt.subplots(figsize=(8, 7))

    # ベクトル場 (符号のみ → 矢印)
    if art["vector_field"] is not None:
        vf = art["vector_field"]
        # 矢印の長さは符号 × スケール
        phase = art.get("phase")
        scale = 1.0
        if phase:
            w_max = phase.get("w_schedule", {}).get("pop_max", 100.0)
            b_max = phase.get("b_schedule", {}).get("pop_max", 100.0)
            scale = 0.04 * max(w_max, b_max)
        ax.quiver(
            vf["w"], vf["b"],
            vf["dw_sign"] * scale, vf["db_sign"] * scale,
            color="#888888", alpha=0.6, width=0.003, scale=1, scale_units="xy",
            angles="xy",
        )

    # 反応曲線
    if art["reaction_curve_w"] is not None:
        df = art["reaction_curve_w"]
        ax.plot(df["own"], df["max_other"], color=COLOR_W_CURVE, linewidth=2,
                label="$B_W(W)$")
    if art["reaction_curve_b"] is not None:
        df = art["reaction_curve_b"]
        ax.plot(df["max_other"], df["own"], color=COLOR_B_CURVE, linewidth=2,
                label="$W_B(B)$")

    # 平衡点
    if art["equilibria"] is not None:
        for _, row in art["equilibria"].iterrows():
            color = EQUILIBRIUM_COLORS.get(row["kind"], "#000000")
            marker = "o" if row["stability"] == "stable" else "x"
            size = 200 if row["stability"] == "stable" else 150
            ax.scatter(row["w"], row["b"], c=color, marker=marker, s=size,
                       edgecolors="black", linewidths=1.2, zorder=5)

    ax.set_xlabel("W (白人数)")
    ax.set_ylabel("B (黒人数)")
    ax.set_title("位相平面 (反応曲線 + ベクトル場 + 平衡点)")
    ax.grid(True, alpha=0.3)
    ax.legend(loc="upper right")
    fig.tight_layout()
    fig.savefig(output_path, dpi=150)
    plt.close(fig)


def plot_trajectory(art: dict, output_path: str) -> None:
    """軌跡を反応曲線に重畳．"""
    if art["trajectory"] is None:
        return
    fig, ax = plt.subplots(figsize=(8, 7))

    # 反応曲線
    if art["reaction_curve_w"] is not None:
        df = art["reaction_curve_w"]
        ax.plot(df["own"], df["max_other"], color=COLOR_W_CURVE, linewidth=1.5,
                alpha=0.6, label="$B_W(W)$")
    if art["reaction_curve_b"] is not None:
        df = art["reaction_curve_b"]
        ax.plot(df["max_other"], df["own"], color=COLOR_B_CURVE, linewidth=1.5,
                alpha=0.6, label="$W_B(B)$")

    # 軌跡
    traj = art["trajectory"]
    ax.plot(traj["w"], traj["b"], color=COLOR_TRAJECTORY, linewidth=2, label="軌跡")
    ax.scatter([traj["w"].iloc[0]], [traj["b"].iloc[0]], c=COLOR_INITIAL, s=120,
               marker="*", edgecolors="black", linewidths=1, zorder=5, label="初期点")
    ax.scatter([traj["w"].iloc[-1]], [traj["b"].iloc[-1]], c=COLOR_TRAJECTORY, s=150,
               marker="o", edgecolors="black", linewidths=1.2, zorder=5, label="終点")

    # 平衡点もマーク
    if art["equilibria"] is not None:
        for _, row in art["equilibria"].iterrows():
            color = EQUILIBRIUM_COLORS.get(row["kind"], "#000000")
            marker = "o" if row["stability"] == "stable" else "x"
            ax.scatter(row["w"], row["b"], c=color, marker=marker, s=80,
                       edgecolors="black", linewidths=0.8, zorder=4, alpha=0.7)

    ax.set_xlabel("W")
    ax.set_ylabel("B")
    ax.set_title("動学軌跡 (位相平面)")
    ax.grid(True, alpha=0.3)
    ax.legend(loc="upper right")
    fig.tight_layout()
    fig.savefig(output_path, dpi=150)
    plt.close(fig)


def plot_basin_of_attraction(art: dict, output_path: str) -> None:
    """吸引域マップ：初期条件 (w0, b0) を収束先で色分け．"""
    if art["basin"] is None:
        return
    df = art["basin"]
    fig, ax = plt.subplots(figsize=(8, 7))

    # 散布図 (収束先カテゴリで色分け)
    for kind in df["converged_kind"].unique():
        mask = df["converged_kind"] == kind
        color = BASIN_COLORS.get(kind, "#000000")
        ax.scatter(df.loc[mask, "w0"], df.loc[mask, "b0"],
                   c=color, s=50, alpha=0.7, label=kind, edgecolors="none")

    # 反応曲線オーバーレイ
    if art["reaction_curve_w"] is not None:
        rc = art["reaction_curve_w"]
        ax.plot(rc["own"], rc["max_other"], color=COLOR_W_CURVE, linewidth=1.5,
                linestyle="--", alpha=0.5)
    if art["reaction_curve_b"] is not None:
        rc = art["reaction_curve_b"]
        ax.plot(rc["max_other"], rc["own"], color=COLOR_B_CURVE, linewidth=1.5,
                linestyle="--", alpha=0.5)

    # 平衡点
    if art["equilibria"] is not None:
        for _, row in art["equilibria"].iterrows():
            color = EQUILIBRIUM_COLORS.get(row["kind"], "#000000")
            marker = "o" if row["stability"] == "stable" else "x"
            size = 200 if row["stability"] == "stable" else 120
            ax.scatter(row["w"], row["b"], c=color, marker=marker, s=size,
                       edgecolors="black", linewidths=1.2, zorder=5)

    ax.set_xlabel("初期 $W_0$")
    ax.set_ylabel("初期 $B_0$")
    ax.set_title("吸引域 (初期条件 → 収束先)")
    ax.grid(True, alpha=0.3)
    ax.legend(loc="upper right", title="収束先")
    fig.tight_layout()
    fig.savefig(output_path, dpi=150)
    plt.close(fig)


# --------------------------------------------------------------------------- #
# CLI
# --------------------------------------------------------------------------- #

def resolve_results_dir(arg: str | None) -> str:
    """--results_dir 解決．省略時は results/latest を参照．"""
    if arg:
        return arg
    base = os.path.join(os.getcwd(), "results")
    latest = os.path.join(base, "latest")
    if os.path.exists(latest):
        return latest
    raise FileNotFoundError(f"results/latest が存在しません．--results_dir で指定してください．")


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(
        prog="schelling-tools visualize-bnm",
        description="境界近隣モデル (BNM) 解析結果の可視化",
    )
    parser.add_argument("--results_dir", default=None,
                        help="BNM 出力ディレクトリ (省略時は results/latest)")
    parser.add_argument("--output_dir", default=None,
                        help="図の出力ディレクトリ (省略時は results_dir/figures)")
    args = parser.parse_args(argv)

    results_dir = resolve_results_dir(args.results_dir)
    output_dir = args.output_dir or os.path.join(results_dir, "figures")
    os.makedirs(output_dir, exist_ok=True)

    print(f"[visualize-bnm] 入力: {results_dir}")
    print(f"[visualize-bnm] 出力: {output_dir}")

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

    print(f"[visualize-bnm] 生成完了: {len(figures)} 枚")
    for f in figures:
        print(f"  - {output_dir}/{f}")


if __name__ == "__main__":
    main()
