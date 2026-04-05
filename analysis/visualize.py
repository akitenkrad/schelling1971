#!/usr/bin/env python3
"""
visualize.py — Schelling (1971) 分離モデル 再現実験 可視化スクリプト

Usage:
    python analysis/visualize.py [--results_dir RESULTS_DIR] [--output_dir OUTPUT_DIR]
                                  [--fps FPS] [--no_animation]

Outputs:
    output_dir/
    ├── animation.gif          ← グリッド進化のアニメーション
    ├── final_state.png        ← 最終状態のグリッドヒートマップ
    ├── metrics_timeseries.png ← メトリクス時系列グラフ
    ├── initial_state.png      ← 初期状態のグリッドヒートマップ
    └── comparison.png         ← 初期・中間・最終状態の3ショット比較
"""

from __future__ import annotations

import argparse
import glob
import os
import sys

import matplotlib as mpl
import matplotlib.animation as animation
import matplotlib.patches as mpatches
import matplotlib.pyplot as plt
import numpy as np
import pandas as pd

# --------------------------------------------------------------------------- #
# 日本語フォント設定
# --------------------------------------------------------------------------- #
plt.rcParams["font.family"] = "Hiragino Sans"

# --------------------------------------------------------------------------- #
# カラー設定
# --------------------------------------------------------------------------- #
COLOR_EMPTY  = "#F5F5F0"   # 空きセル (アイボリー)
COLOR_A      = "#2196F3"   # 集団A (青)
COLOR_B      = "#F44336"   # 集団B (赤)
COLOR_BG     = "#FAFAF8"   # 背景

CMAP = mpl.colors.ListedColormap([COLOR_EMPTY, COLOR_A, COLOR_B])
NORM = mpl.colors.BoundaryNorm([0, 0.5, 1.5, 2.5], CMAP.N)

LEGEND_PATCHES = [
    mpatches.Patch(color=COLOR_A,     label="集団 A"),
    mpatches.Patch(color=COLOR_B,     label="集団 B"),
    mpatches.Patch(color=COLOR_EMPTY, label="空き",
                   edgecolor="#CCCCCC", linewidth=0.5),
]

# --------------------------------------------------------------------------- #
# ユーティリティ
# --------------------------------------------------------------------------- #

def load_snapshot(path: str, rows: int, cols: int) -> np.ndarray:
    """CSVスナップショットをグリッド行列 (rows×cols) に変換する"""
    df = pd.read_csv(path)
    mat = np.zeros((rows, cols), dtype=np.int8)
    for _, row in df.iterrows():
        mat[int(row["row"]), int(row["col"])] = int(row["cell"])
    return mat


def load_all_snapshots(snapshots_dir: str) -> tuple[list[np.ndarray], list[int]]:
    """全スナップショットを読み込み，(行列リスト, ステップリスト) を返す"""
    paths = sorted(glob.glob(os.path.join(snapshots_dir, "step_*.csv")))
    if not paths:
        raise FileNotFoundError(f"スナップショットが見つかりません: {snapshots_dir}")

    # グリッドサイズをファイル名一覧から推定
    sample = pd.read_csv(paths[0])
    rows = int(sample["row"].max()) + 1
    cols = int(sample["col"].max()) + 1

    matrices, steps = [], []
    for p in paths:
        step = int(os.path.basename(p).replace("step_", "").replace(".csv", ""))
        mat = load_snapshot(p, rows, cols)
        matrices.append(mat)
        steps.append(step)

    return matrices, steps


def load_metrics(metrics_path: str) -> pd.DataFrame:
    """metrics.csv を読み込む"""
    if not os.path.exists(metrics_path):
        raise FileNotFoundError(f"metrics.csv が見つかりません: {metrics_path}")
    return pd.read_csv(metrics_path)


# --------------------------------------------------------------------------- #
# 可視化関数
# --------------------------------------------------------------------------- #

def plot_grid(
    ax: plt.Axes,
    mat: np.ndarray,
    step: int,
    title_prefix: str = "",
    show_legend: bool = True,
) -> None:
    """グリッドをヒートマップ形式でプロットする"""
    ax.imshow(mat, cmap=CMAP, norm=NORM, interpolation="nearest", aspect="equal")
    ax.set_xticks([])
    ax.set_yticks([])

    # グリッド線
    rows, cols = mat.shape
    for x in np.arange(-0.5, cols, 1):
        ax.axvline(x, color="#DDDDDD", linewidth=0.3)
    for y in np.arange(-0.5, rows, 1):
        ax.axhline(y, color="#DDDDDD", linewidth=0.3)

    title = f"{title_prefix}Step {step}" if title_prefix else f"Step {step}"
    ax.set_title(title, fontsize=10, pad=4)

    if show_legend:
        ax.legend(
            handles=LEGEND_PATCHES,
            loc="upper right",
            fontsize=7,
            framealpha=0.85,
            handlelength=1.0,
            handleheight=0.8,
        )


def save_single_grid(
    mat: np.ndarray,
    step: int,
    out_path: str,
    title: str = "",
) -> None:
    """単一グリッドを PNG として保存する"""
    fig, ax = plt.subplots(figsize=(6, 5), facecolor=COLOR_BG)
    ax.set_facecolor(COLOR_BG)
    plot_grid(ax, mat, step, title_prefix=title)
    fig.tight_layout()
    fig.savefig(out_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  保存: {out_path}")


def save_comparison(
    matrices: list[np.ndarray],
    steps: list[int],
    out_path: str,
) -> None:
    """初期・中間・最終の3ショット比較図を保存する"""
    n = len(matrices)
    indices = [0, n // 2, n - 1]
    titles  = ["初期状態", "中間状態", "最終状態"]

    fig, axes = plt.subplots(1, 3, figsize=(14, 5), facecolor=COLOR_BG)
    fig.suptitle("Schelling 分離モデル — グリッド状態の比較", fontsize=13, y=1.01)

    for ax, idx, title in zip(axes, indices, titles):
        ax.set_facecolor(COLOR_BG)
        plot_grid(ax, matrices[idx], steps[idx],
                  title_prefix=f"{title}\n", show_legend=(idx == indices[-1]))

    fig.tight_layout()
    fig.savefig(out_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  保存: {out_path}")


def save_metrics_timeseries(df: pd.DataFrame, out_path: str) -> None:
    """メトリクス時系列グラフを保存する"""
    fig, axes = plt.subplots(2, 2, figsize=(12, 8), facecolor=COLOR_BG)
    fig.suptitle("Schelling 分離モデル — メトリクス時系列", fontsize=13)

    step = df["step"]

    # (1) 平均同色近隣比率
    ax = axes[0, 0]
    ax.set_facecolor(COLOR_BG)
    ax.plot(step, df["avg_same_ratio"] * 100, color="#333333", lw=2, label="全体")
    ax.plot(step, df["avg_same_ratio_a"] * 100, color=COLOR_A, lw=1.5,
            linestyle="--", label="集団A")
    ax.plot(step, df["avg_same_ratio_b"] * 100, color=COLOR_B, lw=1.5,
            linestyle="--", label="集団B")
    ax.set_xlabel("Step")
    ax.set_ylabel("平均同色近隣比率 (%)")
    ax.set_title("平均同色近隣比率の推移")
    ax.legend(fontsize=8)
    ax.set_ylim(0, 105)
    ax.grid(True, alpha=0.3)

    # (2) 異色近隣なし割合
    ax = axes[0, 1]
    ax.set_facecolor(COLOR_BG)
    ax.plot(step, df["pct_no_opposite"], color="#9C27B0", lw=2)
    ax.set_xlabel("Step")
    ax.set_ylabel("割合 (%)")
    ax.set_title("異色近隣を持たないエージェントの割合")
    ax.set_ylim(0, 105)
    ax.grid(True, alpha=0.3)

    # (3) 不満足エージェント数 & 移動数
    ax = axes[1, 0]
    ax.set_facecolor(COLOR_BG)
    ax.bar(step, df["n_dissatisfied"], color="#FF9800", alpha=0.7, label="不満足数", width=0.8)
    ax.plot(step, df["n_moved"], color="#4CAF50", lw=2, label="移動数")
    ax.set_xlabel("Step")
    ax.set_ylabel("エージェント数")
    ax.set_title("不満足エージェント数と移動数")
    ax.legend(fontsize=8)
    ax.grid(True, alpha=0.3)

    # (4) 非類似性指数 (参考)
    ax = axes[1, 1]
    ax.set_facecolor(COLOR_BG)
    ax.plot(step, df["dissimilarity_index"], color="#607D8B", lw=2)
    ax.set_xlabel("Step")
    ax.set_ylabel("D")
    ax.set_title("非類似性指数 D (参考)")
    ax.set_ylim(0, 0.6)
    ax.grid(True, alpha=0.3)

    fig.tight_layout()
    fig.savefig(out_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  保存: {out_path}")


def save_animation(
    matrices: list[np.ndarray],
    steps: list[int],
    df: pd.DataFrame,
    out_path: str,
    fps: int = 5,
) -> None:
    """グリッド進化のアニメーションを GIF として保存する"""
    fig, axes = plt.subplots(
        1, 2, figsize=(12, 5),
        gridspec_kw={"width_ratios": [1, 1.3]},
        facecolor=COLOR_BG,
    )
    fig.suptitle("Schelling 分離モデル — シミュレーション進行", fontsize=12)

    ax_grid, ax_metrics = axes

    # --- 左: グリッド ---
    ax_grid.set_facecolor(COLOR_BG)
    im = ax_grid.imshow(
        matrices[0], cmap=CMAP, norm=NORM,
        interpolation="nearest", aspect="equal",
    )
    ax_grid.set_xticks([])
    ax_grid.set_yticks([])
    ax_grid.legend(
        handles=LEGEND_PATCHES, loc="upper right",
        fontsize=7, framealpha=0.85,
        handlelength=1.0, handleheight=0.8,
    )
    title_text = ax_grid.set_title(f"Step {steps[0]}", fontsize=10)

    # グリッド線
    rows, cols = matrices[0].shape
    for x in np.arange(-0.5, cols, 1):
        ax_grid.axvline(x, color="#DDDDDD", linewidth=0.3)
    for y in np.arange(-0.5, rows, 1):
        ax_grid.axhline(y, color="#DDDDDD", linewidth=0.3)

    # --- 右: メトリクス時系列 (追記型) ---
    ax_metrics.set_facecolor(COLOR_BG)
    ax_metrics.set_xlabel("Step")
    ax_metrics.set_ylabel("比率 (%)")
    ax_metrics.set_title("メトリクス推移")
    ax_metrics.set_xlim(df["step"].min(), df["step"].max())
    ax_metrics.set_ylim(0, 105)
    ax_metrics.grid(True, alpha=0.3)

    line_all, = ax_metrics.plot([], [], color="#333333", lw=2,   label="平均同色比率")
    line_a,   = ax_metrics.plot([], [], color=COLOR_A,   lw=1.5, linestyle="--", label="集団A")
    line_b,   = ax_metrics.plot([], [], color=COLOR_B,   lw=1.5, linestyle="--", label="集団B")
    line_noopp, = ax_metrics.plot([], [], color="#9C27B0", lw=1.5, linestyle=":",  label="異色近隣なし")
    vline = ax_metrics.axvline(0, color="#888888", linewidth=0.8, linestyle="--")
    ax_metrics.legend(fontsize=7, loc="lower right")

    # ステップ→DataFrameのインデックスのマッピング
    step_to_idx = {s: i for i, s in enumerate(df["step"].tolist())}

    def _init():
        im.set_data(matrices[0])
        line_all.set_data([], [])
        line_a.set_data([], [])
        line_b.set_data([], [])
        line_noopp.set_data([], [])
        return im, line_all, line_a, line_b, line_noopp, vline, title_text

    def _update(frame_idx: int):
        mat   = matrices[frame_idx]
        step  = steps[frame_idx]
        im.set_data(mat)
        title_text.set_text(f"Step {step}")

        # メトリクスを現在ステップまで描画
        df_upto = df[df["step"] <= step]
        xs = df_upto["step"].values
        line_all.set_data(xs, df_upto["avg_same_ratio"].values * 100)
        line_a.set_data(xs, df_upto["avg_same_ratio_a"].values * 100)
        line_b.set_data(xs, df_upto["avg_same_ratio_b"].values * 100)
        line_noopp.set_data(xs, df_upto["pct_no_opposite"].values)
        vline.set_xdata([step, step])

        return im, line_all, line_a, line_b, line_noopp, vline, title_text

    ani = animation.FuncAnimation(
        fig,
        _update,
        frames=len(matrices),
        init_func=_init,
        blit=True,
        interval=1000 // fps,
    )

    fig.tight_layout()
    ani.save(out_path, writer="pillow", fps=fps, dpi=120)
    plt.close(fig)
    print(f"  保存: {out_path}")


# --------------------------------------------------------------------------- #
# メイン
# --------------------------------------------------------------------------- #

def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description="Schelling 分離モデル 可視化スクリプト"
    )
    p.add_argument(
        "--results_dir", default="results/latest",
        help="Rustシミュレーションの出力ディレクトリ (default: results/latest)"
    )
    p.add_argument(
        "--output_dir", default=None,
        help="図の保存先ディレクトリ (default: {results_dir}/figures)"
    )
    p.add_argument(
        "--fps", type=int, default=5,
        help="アニメーションのFPS (default: 5)"
    )
    p.add_argument(
        "--no_animation", action="store_true",
        help="アニメーションの生成をスキップする"
    )
    p.add_argument(
        "--max_frames", type=int, default=0,
        help="アニメーションの最大フレーム数 (0=全フレーム)"
    )
    return p.parse_args()


def main() -> None:
    args = parse_args()

    snapshots_dir = os.path.join(args.results_dir, "snapshots")
    metrics_path  = os.path.join(args.results_dir, "metrics.csv")
    out_dir       = args.output_dir if args.output_dir else os.path.join(args.results_dir, "figures")

    os.makedirs(out_dir, exist_ok=True)

    print("=== Schelling 分離モデル 可視化 ===")
    print(f"スナップショット: {snapshots_dir}")
    print(f"メトリクス:       {metrics_path}")
    print(f"出力先:           {out_dir}")
    print("-----------------------------------")

    # データ読み込み
    print("[1/5] スナップショットを読み込み中 ...")
    matrices, steps = load_all_snapshots(snapshots_dir)
    print(f"      {len(matrices)} ステップ分 | グリッド {matrices[0].shape}")

    print("[2/5] メトリクスを読み込み中 ...")
    df = load_metrics(metrics_path)
    print(f"      {len(df)} 行")

    # 初期状態
    print("[3/5] 初期状態を保存中 ...")
    save_single_grid(matrices[0], steps[0],
                     os.path.join(out_dir, "initial_state.png"), title="初期状態 — ")

    # 最終状態
    save_single_grid(matrices[-1], steps[-1],
                     os.path.join(out_dir, "final_state.png"), title="最終状態 — ")

    # 比較図
    save_comparison(matrices, steps, os.path.join(out_dir, "comparison.png"))

    # メトリクス時系列
    print("[4/5] メトリクス時系列を保存中 ...")
    save_metrics_timeseries(df, os.path.join(out_dir, "metrics_timeseries.png"))

    # アニメーション
    if not args.no_animation:
        print("[5/5] アニメーションを生成中 (時間がかかる場合があります) ...")
        mats = matrices
        stps = steps
        if args.max_frames > 0 and len(mats) > args.max_frames:
            # 均等サンプリング
            idx = np.linspace(0, len(mats) - 1, args.max_frames, dtype=int)
            mats = [mats[i] for i in idx]
            stps = [stps[i] for i in idx]
        save_animation(mats, stps, df,
                       os.path.join(out_dir, "animation.gif"),
                       fps=args.fps)
    else:
        print("[5/5] アニメーションをスキップしました")

    print("-----------------------------------")
    print("完了．出力ファイル一覧:")
    for f in sorted(os.listdir(out_dir)):
        size_kb = os.path.getsize(os.path.join(out_dir, f)) / 1024
        print(f"  {f:35s} ({size_kb:6.1f} KB)")


if __name__ == "__main__":
    main()
