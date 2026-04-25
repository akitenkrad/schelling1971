#!/usr/bin/env python3
"""
visualize_sweep.py — Schelling (1971) 分離モデル パラメータスイープ結果 可視化スクリプト

Usage:
    uv run python analysis/visualize_sweep.py
    uv run python analysis/visualize_sweep.py --sweep_dir results/20260405_160827_sweep
    uv run python analysis/visualize_sweep.py --sweep_dir results/latest --output_dir out

Outputs:
    output_dir/
    ├── sweep_avg_same_ratio.png  ← 平均同色近隣比率 (1Dライン or 2Dヒートマップ)
    ├── sweep_pct_no_opposite.png ← 異色近隣なし割合
    ├── sweep_convergence.png     ← 収束速度 (最終イテレーション数)
    ├── sweep_overview.png        ← 2×2 パネル概要図
    └── sweep_grid_animation.gif  ← パラメータ組み合わせ別のグリッドアニメーション
                                    (sweep を --snapshot-interval > 0 で実行した場合のみ)
"""

from __future__ import annotations

import argparse
import json
import os
import sys

import matplotlib.animation as animation
import matplotlib.pyplot as plt
import numpy as np
import pandas as pd

from schelling_tools.visualize import (
    CMAP,
    LEGEND_PATCHES,
    NORM,
    load_all_snapshots,
)

# --------------------------------------------------------------------------- #
# 日本語フォント設定
# --------------------------------------------------------------------------- #
plt.rcParams["font.family"] = "Hiragino Sans"

# --------------------------------------------------------------------------- #
# カラー設定
# --------------------------------------------------------------------------- #
COLOR_BG = "#FAFAF8"

COLOR_AVG_SAME = "#333333"
COLOR_A = "#2196F3"
COLOR_B = "#F44336"
COLOR_PCT_NO_OPP = "#9C27B0"
COLOR_ITERATION = "#FF9800"
COLOR_DISSIMILARITY = "#607D8B"

# --------------------------------------------------------------------------- #
# ユーティリティ
# --------------------------------------------------------------------------- #


def detect_sweep_type(df: pd.DataFrame) -> tuple[str, list[str]]:
    """スイープの次元を検出する．

    Returns:
        ("1d", [varying_col]) or ("2d", ["threshold", "vacant_rate"])
    """
    n_threshold = df["threshold"].nunique()
    n_vacant = df["vacant_rate"].nunique()

    if n_threshold > 1 and n_vacant > 1:
        return "2d", ["threshold", "vacant_rate"]
    elif n_threshold > 1:
        return "1d", ["threshold"]
    elif n_vacant > 1:
        return "1d", ["vacant_rate"]
    else:
        # 単一パラメータ（シード違いのみ）→ threshold をダミー軸とする
        return "1d", ["threshold"]


def load_sweep_config(sweep_dir: str) -> dict | None:
    """sweep_config.json を読み込む（存在しなければ None）"""
    path = os.path.join(sweep_dir, "sweep_config.json")
    if os.path.exists(path):
        with open(path) as f:
            return json.load(f)
    return None


def make_subtitle(config: dict | None, df: pd.DataFrame) -> str:
    """設定情報からサブタイトル文字列を生成する"""
    parts: list[str] = []

    if config:
        rows = config.get("rows", None)
        cols = config.get("cols", None)
        if rows and cols:
            parts.append(f"{rows}×{cols} グリッド")
    else:
        rows_vals = df["rows"].unique()
        cols_vals = df["cols"].unique()
        if len(rows_vals) == 1 and len(cols_vals) == 1:
            parts.append(f"{rows_vals[0]}×{cols_vals[0]} グリッド")

    n_seeds = df["seed"].nunique()
    parts.append(f"{n_seeds} シード")

    return "，".join(parts)


# --------------------------------------------------------------------------- #
# 1D プロット関数
# --------------------------------------------------------------------------- #


def _plot_1d_line(
    ax: plt.Axes,
    df: pd.DataFrame,
    x_col: str,
    y_col: str,
    color: str,
    label: str,
    ylabel: str,
    title: str,
    *,
    y_percent: bool = False,
    ylim: tuple[float, float] | None = None,
    extra_lines: list[tuple[str, str, str]] | None = None,
    hline: float | None = None,
) -> None:
    """1Dスイープ用の折れ線／散布図をプロットする"""
    ax.set_facecolor(COLOR_BG)

    grouped = df.groupby(x_col)
    xs = sorted(df[x_col].unique())
    n_seeds = df["seed"].nunique()
    scale = 100.0 if y_percent else 1.0

    # 平均と標準偏差
    means = [grouped.get_group(x)[y_col].mean() * scale for x in xs]
    stds = [grouped.get_group(x)[y_col].std() * scale for x in xs]

    # 複数シードなら個別点をプロット
    if n_seeds > 1:
        for x in xs:
            vals = grouped.get_group(x)[y_col].values * scale
            ax.scatter(
                [x] * len(vals), vals,
                color=color, alpha=0.25, s=20, zorder=2,
            )
        ax.errorbar(
            xs, means, yerr=stds,
            color=color, lw=2, capsize=3, label=label, zorder=3,
        )
    else:
        ax.plot(xs, means, color=color, lw=2, marker="o", markersize=4, label=label)

    # 追加線（avg_same_ratio_a, avg_same_ratio_b など）
    if extra_lines:
        for ecol, ecolor, elabel in extra_lines:
            emeans = [grouped.get_group(x)[ecol].mean() * scale for x in xs]
            ax.plot(
                xs, emeans,
                color=ecolor, lw=1.5, linestyle="--", label=elabel,
            )

    # 水平参照線
    if hline is not None:
        ax.axhline(hline, color="#AAAAAA", linestyle=":", linewidth=1, label=f"{hline:.0f}% 基準線")

    x_labels = {
        "threshold": "閾値 τ",
        "vacant_rate": "空き地率",
    }
    ax.set_xlabel(x_labels.get(x_col, x_col))
    ax.set_ylabel(ylabel)
    ax.set_title(title)
    if ylim:
        ax.set_ylim(*ylim)
    ax.legend(fontsize=7)
    ax.grid(True, alpha=0.3)


def _plot_1d_bar(
    ax: plt.Axes,
    df: pd.DataFrame,
    x_col: str,
    y_col: str,
    color: str,
    ylabel: str,
    title: str,
) -> None:
    """1Dスイープ用の棒グラフ（収束ステップ数など）をプロットする"""
    ax.set_facecolor(COLOR_BG)

    grouped = df.groupby(x_col)
    xs = sorted(df[x_col].unique())
    n_seeds = df["seed"].nunique()

    means = [grouped.get_group(x)[y_col].mean() for x in xs]
    stds = [grouped.get_group(x)[y_col].std() for x in xs]

    if n_seeds > 1:
        ax.bar(
            range(len(xs)), means, yerr=stds,
            color=color, alpha=0.7, capsize=3, width=0.6,
        )
    else:
        ax.bar(range(len(xs)), means, color=color, alpha=0.7, width=0.6)

    ax.set_xticks(range(len(xs)))
    ax.set_xticklabels([f"{x:.3g}" for x in xs], fontsize=8)

    x_labels = {
        "threshold": "閾値 τ",
        "vacant_rate": "空き地率",
    }
    ax.set_xlabel(x_labels.get(x_col, x_col))
    ax.set_ylabel(ylabel)
    ax.set_title(title)
    ax.grid(True, alpha=0.3, axis="y")


# --------------------------------------------------------------------------- #
# 2D プロット関数
# --------------------------------------------------------------------------- #


def _plot_2d_heatmap(
    ax: plt.Axes,
    df: pd.DataFrame,
    z_col: str,
    cmap: str,
    title: str,
    *,
    z_percent: bool = False,
    fmt: str = ".1f",
) -> None:
    """2Dスイープ用のヒートマップをプロットする"""
    ax.set_facecolor(COLOR_BG)
    scale = 100.0 if z_percent else 1.0

    pivot = df.groupby(["vacant_rate", "threshold"])[z_col].mean().unstack()
    data = pivot.values * scale

    thresholds = pivot.columns.values
    vacant_rates = pivot.index.values

    im = ax.imshow(
        data, aspect="auto", origin="lower",
        cmap=cmap, interpolation="nearest",
    )
    plt.colorbar(im, ax=ax, fraction=0.046, pad=0.04)

    # セルにアノテーション
    for i in range(len(vacant_rates)):
        for j in range(len(thresholds)):
            val = data[i, j]
            text_color = "white" if val > (data.max() + data.min()) / 2 else "black"
            ax.text(
                j, i, f"{val:{fmt}}",
                ha="center", va="center", fontsize=7, color=text_color,
            )

    ax.set_xticks(range(len(thresholds)))
    ax.set_xticklabels([f"{t:.2g}" for t in thresholds], fontsize=8)
    ax.set_yticks(range(len(vacant_rates)))
    ax.set_yticklabels([f"{v:.2g}" for v in vacant_rates], fontsize=8)
    ax.set_xlabel("閾値 τ")
    ax.set_ylabel("空き地率")
    ax.set_title(title)


# --------------------------------------------------------------------------- #
# 図の生成
# --------------------------------------------------------------------------- #


def save_avg_same_ratio(
    df: pd.DataFrame, sweep_type: str, sweep_cols: list[str],
    out_path: str, subtitle: str,
) -> None:
    """平均同色近隣比率のプロットを保存する"""
    fig, ax = plt.subplots(figsize=(8, 5), facecolor=COLOR_BG)
    fig.suptitle("平均同色近隣比率", fontsize=13)
    if subtitle:
        fig.text(0.5, 0.93, subtitle, ha="center", fontsize=9, color="#666666")

    if sweep_type == "1d":
        _plot_1d_line(
            ax, df, sweep_cols[0], "avg_same_ratio",
            COLOR_AVG_SAME, "全体", "平均同色近隣比率 (%)",
            "平均同色近隣比率",
            y_percent=True, ylim=(0, 105),
            extra_lines=[
                ("avg_same_ratio_a", COLOR_A, "集団A"),
                ("avg_same_ratio_b", COLOR_B, "集団B"),
            ],
            hline=50.0,
        )
    else:
        _plot_2d_heatmap(
            ax, df, "avg_same_ratio", "YlOrRd",
            "平均同色近隣比率 (%)", z_percent=True, fmt=".1f",
        )

    fig.tight_layout(rect=[0, 0, 1, 0.92])
    fig.savefig(out_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  保存: {out_path}")


def save_pct_no_opposite(
    df: pd.DataFrame, sweep_type: str, sweep_cols: list[str],
    out_path: str, subtitle: str,
) -> None:
    """異色近隣なし割合のプロットを保存する"""
    fig, ax = plt.subplots(figsize=(8, 5), facecolor=COLOR_BG)
    fig.suptitle("異色近隣を持たないエージェントの割合", fontsize=13)
    if subtitle:
        fig.text(0.5, 0.93, subtitle, ha="center", fontsize=9, color="#666666")

    if sweep_type == "1d":
        _plot_1d_line(
            ax, df, sweep_cols[0], "pct_no_opposite",
            COLOR_PCT_NO_OPP, "異色近隣なし", "割合 (%)",
            "異色近隣なし割合",
            ylim=(0, 105),
        )
    else:
        _plot_2d_heatmap(
            ax, df, "pct_no_opposite", "Purples",
            "異色近隣なし割合 (%)", fmt=".1f",
        )

    fig.tight_layout(rect=[0, 0, 1, 0.92])
    fig.savefig(out_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  保存: {out_path}")


def save_convergence(
    df: pd.DataFrame, sweep_type: str, sweep_cols: list[str],
    out_path: str, subtitle: str,
) -> None:
    """収束速度のプロットを保存する"""
    fig, ax = plt.subplots(figsize=(8, 5), facecolor=COLOR_BG)
    fig.suptitle("収束ステップ数", fontsize=13)
    if subtitle:
        fig.text(0.5, 0.93, subtitle, ha="center", fontsize=9, color="#666666")

    if sweep_type == "1d":
        _plot_1d_bar(
            ax, df, sweep_cols[0], "final_iteration",
            COLOR_ITERATION, "ステップ数", "収束ステップ数",
        )
    else:
        _plot_2d_heatmap(
            ax, df, "final_iteration", "YlOrBr",
            "収束ステップ数", fmt=".0f",
        )

    fig.tight_layout(rect=[0, 0, 1, 0.92])
    fig.savefig(out_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  保存: {out_path}")


def save_overview(
    df: pd.DataFrame, sweep_type: str, sweep_cols: list[str],
    out_path: str, subtitle: str,
) -> None:
    """2×2 パネル概要図を保存する"""
    fig, axes = plt.subplots(2, 2, figsize=(14, 10), facecolor=COLOR_BG)
    fig.suptitle("Schelling 分離モデル — パラメータスイープ概要", fontsize=14)
    if subtitle:
        fig.text(0.5, 0.95, subtitle, ha="center", fontsize=9, color="#666666")

    if sweep_type == "1d":
        x_col = sweep_cols[0]

        # (1) 平均同色近隣比率
        _plot_1d_line(
            axes[0, 0], df, x_col, "avg_same_ratio",
            COLOR_AVG_SAME, "全体", "平均同色近隣比率 (%)",
            "平均同色近隣比率",
            y_percent=True, ylim=(0, 105),
            extra_lines=[
                ("avg_same_ratio_a", COLOR_A, "集団A"),
                ("avg_same_ratio_b", COLOR_B, "集団B"),
            ],
            hline=50.0,
        )

        # (2) 異色近隣なし割合
        _plot_1d_line(
            axes[0, 1], df, x_col, "pct_no_opposite",
            COLOR_PCT_NO_OPP, "異色近隣なし", "割合 (%)",
            "異色近隣なし割合",
            ylim=(0, 105),
        )

        # (3) 収束ステップ数
        _plot_1d_bar(
            axes[1, 0], df, x_col, "final_iteration",
            COLOR_ITERATION, "ステップ数", "収束ステップ数",
        )

        # (4) 非類似性指数
        _plot_1d_line(
            axes[1, 1], df, x_col, "dissimilarity_index",
            COLOR_DISSIMILARITY, "D", "非類似性指数 D",
            "非類似性指数 D",
            ylim=(0, 1.0),
        )
    else:
        # 2D ヒートマップ
        _plot_2d_heatmap(
            axes[0, 0], df, "avg_same_ratio", "YlOrRd",
            "平均同色近隣比率 (%)", z_percent=True, fmt=".1f",
        )
        _plot_2d_heatmap(
            axes[0, 1], df, "pct_no_opposite", "Purples",
            "異色近隣なし割合 (%)", fmt=".1f",
        )
        _plot_2d_heatmap(
            axes[1, 0], df, "final_iteration", "YlOrBr",
            "収束ステップ数", fmt=".0f",
        )
        _plot_2d_heatmap(
            axes[1, 1], df, "dissimilarity_index", "Blues",
            "非類似性指数 D", fmt=".3f",
        )

    fig.tight_layout(rect=[0, 0, 1, 0.93])
    fig.savefig(out_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  保存: {out_path}")


# --------------------------------------------------------------------------- #
# パラメータ組み合わせ別グリッドアニメーション
# --------------------------------------------------------------------------- #


def _run_dir_name(threshold: float, vacant_rate: float, seed: int) -> str:
    """Rust 側 (cmd_sweep) と整合する run ディレクトリ名を生成する."""
    return f"tau_{threshold:.3f}_vac_{vacant_rate:.3f}_seed_{seed}"


def _enumerate_combos(
    df: pd.DataFrame, sweep_type: str, sweep_cols: list[str], seed: int,
) -> tuple[int, int, list[tuple[float, float]], list[str]]:
    """グリッドレイアウトと各セルの (vacant_rate, threshold) を列挙する.

    Returns:
        (n_rows, n_cols, combo_keys, combo_labels)
        combo_keys[i] = (vacant_rate, threshold) でセル i に対応する run を一意に特定する.
    """
    if sweep_type == "2d":
        thresholds = sorted(df["threshold"].unique())
        vacant_rates = sorted(df["vacant_rate"].unique())
        n_rows = len(vacant_rates)
        n_cols = len(thresholds)
        # 行: vacant_rate (上→下で増加), 列: threshold (左→右で増加)
        combo_keys = [(v, t) for v in vacant_rates for t in thresholds]
        combo_labels = [f"τ={t:.3g}, vac={v:.3g}" for v, t in combo_keys]
        return n_rows, n_cols, combo_keys, combo_labels

    # 1D: 単一パラメータが変化．他方は df から固定値を取得
    x_col = sweep_cols[0]
    xs = sorted(df[x_col].unique())
    n = len(xs)
    n_cols = min(n, 4)
    n_rows = (n + n_cols - 1) // n_cols

    if x_col == "threshold":
        fixed_vac = float(df["vacant_rate"].iloc[0])
        combo_keys = [(fixed_vac, t) for t in xs]
        combo_labels = [f"τ={t:.3g}" for t in xs]
    else:
        fixed_tau = float(df["threshold"].iloc[0])
        combo_keys = [(v, fixed_tau) for v in xs]
        combo_labels = [f"vac={v:.3g}" for v in xs]
    return n_rows, n_cols, combo_keys, combo_labels


def save_grid_animation(
    sweep_dir: str,
    df: pd.DataFrame,
    sweep_type: str,
    sweep_cols: list[str],
    out_path: str,
    *,
    seed: int | None = None,
    fps: int = 5,
    max_frames: int = 0,
    subtitle: str = "",
) -> bool:
    """各パラメータ組み合わせのグリッド進行アニメーションを格子状に並べた合成 GIF を保存する.

    各セルは選択された seed の run のスナップショットを再生する．収束ステップの異なる
    run はそれぞれの最終フレームを保持して同期する．

    Returns:
        True: 保存成功 / False: 利用可能なスナップショットが無くスキップ
    """
    available_seeds = sorted(int(s) for s in df["seed"].unique())
    if not available_seeds:
        print("  警告: sweep_summary.csv に seed が含まれていません．スキップ．")
        return False
    if seed is None:
        seed = available_seeds[0]
    elif seed not in available_seeds:
        print(
            f"  警告: 指定 seed={seed} はスイープ結果にありません {available_seeds}．"
            f"先頭シード seed={available_seeds[0]} を使用．"
        )
        seed = available_seeds[0]

    n_rows, n_cols, combo_keys, combo_labels = _enumerate_combos(
        df, sweep_type, sweep_cols, seed,
    )

    # 各セルのスナップショットを読み込む
    cell_snapshots: list[tuple[list[np.ndarray], list[int]] | None] = []
    missing: list[str] = []
    for vac, tau in combo_keys:
        snap_dir = os.path.join(sweep_dir, _run_dir_name(tau, vac, seed), "snapshots")
        if not os.path.isdir(snap_dir):
            cell_snapshots.append(None)
            missing.append(_run_dir_name(tau, vac, seed))
            continue
        try:
            matrices, steps = load_all_snapshots(snap_dir)
        except FileNotFoundError:
            cell_snapshots.append(None)
            missing.append(_run_dir_name(tau, vac, seed))
            continue
        cell_snapshots.append((matrices, steps))

    valid = [s for s in cell_snapshots if s is not None]
    if not valid:
        print(
            "  警告: スナップショットを持つ run が一つもありません．"
            "sweep を `--snapshot-interval N` (N>0) 付きで再実行してください．"
        )
        return False

    if missing:
        print(f"  注意: {len(missing)} 件の組み合わせにスナップショットがありません (空セルとして描画)")

    # 全セルで共通のフレーム数 (= 最長 run のステップ数) を決定
    n_frames = max(len(m) for m, _ in valid)

    # フレーム数を上限で間引き (均等サンプリング)
    if max_frames > 0 and n_frames > max_frames:
        sampled_idx = np.linspace(0, n_frames - 1, max_frames, dtype=int)
        n_frames = len(sampled_idx)
    else:
        sampled_idx = np.arange(n_frames)

    def cell_frame(cell_idx: int, frame_pos: int) -> tuple[np.ndarray, int]:
        data = cell_snapshots[cell_idx]
        assert data is not None
        matrices, steps = data
        # 元 run 上のステップ位置を sampled_idx 経由で算出し，run の長さで頭打ちする
        original_pos = int(sampled_idx[frame_pos])
        clipped = min(original_pos, len(matrices) - 1)
        return matrices[clipped], steps[clipped]

    # 図と軸を準備
    cell_w = 2.6
    cell_h = 2.4
    fig_w = max(6.0, n_cols * cell_w)
    fig_h = max(4.0, n_rows * cell_h + 1.2)
    fig, axes = plt.subplots(
        n_rows, n_cols, figsize=(fig_w, fig_h), facecolor=COLOR_BG, squeeze=False,
    )
    header = f"seed={seed}"
    if subtitle:
        header = f"{subtitle}，{header}"
    # 副題込みで suptitle を 2 行表示する (overlap 回避のため fig.text を使わない)
    fig.suptitle(
        f"Schelling 分離モデル — パラメータ組み合わせ別アニメーション\n{header}",
        fontsize=12,
    )

    ims: list[plt.AxesImage | None] = []
    titles: list[plt.Text | None] = []
    n_combos = len(combo_keys)
    for idx in range(n_rows * n_cols):
        r, c = divmod(idx, n_cols)
        ax = axes[r, c]
        ax.set_facecolor(COLOR_BG)
        ax.set_xticks([])
        ax.set_yticks([])

        if idx >= n_combos:
            ax.axis("off")
            ims.append(None)
            titles.append(None)
            continue

        data = cell_snapshots[idx]
        label = combo_labels[idx]
        if data is None:
            ax.text(
                0.5, 0.5, "(no snapshots)", ha="center", va="center",
                transform=ax.transAxes, fontsize=8, color="#999999",
            )
            ax.set_title(label, fontsize=9)
            ims.append(None)
            titles.append(None)
            continue

        mat0, step0 = cell_frame(idx, 0)
        im = ax.imshow(mat0, cmap=CMAP, norm=NORM, interpolation="nearest", aspect="equal")
        title = ax.set_title(f"{label}\nstep {step0}", fontsize=8)
        ims.append(im)
        titles.append(title)

    # 凡例は figure レベルで一度だけ描画
    fig.legend(
        handles=LEGEND_PATCHES,
        loc="lower center",
        ncol=3,
        fontsize=8,
        frameon=False,
        bbox_to_anchor=(0.5, 0.0),
    )

    def _update(frame_pos: int):
        artists = []
        for i, data in enumerate(cell_snapshots):
            if data is None or ims[i] is None:
                continue
            mat, step = cell_frame(i, frame_pos)
            ims[i].set_data(mat)
            titles[i].set_text(f"{combo_labels[i]}\nstep {step}")
            artists.append(ims[i])
            artists.append(titles[i])
        return artists

    ani = animation.FuncAnimation(
        fig,
        _update,
        frames=n_frames,
        blit=False,
        interval=1000 // max(fps, 1),
    )

    # 行ごとの 2 行タイトル分の余白を確保
    fig.tight_layout(rect=[0, 0.05, 1, 0.92])
    fig.subplots_adjust(hspace=0.45, wspace=0.15)
    ani.save(out_path, writer="pillow", fps=fps, dpi=90)
    plt.close(fig)
    print(f"  保存: {out_path}  ({n_frames} フレーム, {n_combos} セル, seed={seed})")
    return True


# --------------------------------------------------------------------------- #
# メイン
# --------------------------------------------------------------------------- #


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    p = argparse.ArgumentParser(
        prog="schelling-tools visualize-sweep",
        description="Schelling 分離モデル パラメータスイープ 可視化スクリプト"
    )
    p.add_argument(
        "--sweep_dir", "--sweep-dir", default="results/latest",
        help="スイープ結果のディレクトリ (default: results/latest)",
    )
    p.add_argument(
        "--output_dir", "--output-dir", default=None,
        help="図の保存先ディレクトリ (default: {sweep_dir}/figures)",
    )
    p.add_argument(
        "--no_grid_animation", "--no-grid-animation", action="store_true",
        help="パラメータ組み合わせ別グリッドアニメーションの生成をスキップする",
    )
    p.add_argument(
        "--grid_seed", "--grid-seed", type=int, default=None,
        help="グリッドアニメーションで使用する seed (default: 先頭シード)",
    )
    p.add_argument(
        "--fps", type=int, default=5,
        help="グリッドアニメーションの FPS (default: 5)",
    )
    p.add_argument(
        "--max_frames", "--max-frames", type=int, default=0,
        help="グリッドアニメーションの最大フレーム数 (0=全フレーム)",
    )
    return p.parse_args(argv)


def main(argv: list[str] | None = None) -> None:
    args = parse_args(argv)

    sweep_dir = args.sweep_dir
    out_dir = args.output_dir if args.output_dir else os.path.join(sweep_dir, "figures")

    summary_path = os.path.join(sweep_dir, "sweep_summary.csv")

    if not os.path.exists(summary_path):
        print(f"エラー: sweep_summary.csv が見つかりません: {summary_path}", file=sys.stderr)
        sys.exit(1)

    os.makedirs(out_dir, exist_ok=True)

    print("=== Schelling 分離モデル パラメータスイープ 可視化 ===")
    print(f"スイープ結果: {sweep_dir}")
    print(f"出力先:       {out_dir}")
    print("---------------------------------------------------")

    # データ読み込み
    print("[1/6] sweep_summary.csv を読み込み中 ...")
    df = pd.read_csv(summary_path)
    print(f"      {len(df)} 行")

    # 設定読み込み
    print("[2/6] スイープ設定を確認中 ...")
    config = load_sweep_config(sweep_dir)
    sweep_type, sweep_cols = detect_sweep_type(df)
    subtitle = make_subtitle(config, df)
    print(f"      スイープ種別: {sweep_type} ({', '.join(sweep_cols)})")
    print(f"      {subtitle}")

    # 図の生成
    print("[3/6] 平均同色近隣比率を保存中 ...")
    save_avg_same_ratio(
        df, sweep_type, sweep_cols,
        os.path.join(out_dir, "sweep_avg_same_ratio.png"), subtitle,
    )

    print("[4/6] 異色近隣なし割合・収束ステップ数を保存中 ...")
    save_pct_no_opposite(
        df, sweep_type, sweep_cols,
        os.path.join(out_dir, "sweep_pct_no_opposite.png"), subtitle,
    )
    save_convergence(
        df, sweep_type, sweep_cols,
        os.path.join(out_dir, "sweep_convergence.png"), subtitle,
    )

    print("[5/6] 概要パネルを保存中 ...")
    save_overview(
        df, sweep_type, sweep_cols,
        os.path.join(out_dir, "sweep_overview.png"), subtitle,
    )

    if args.no_grid_animation:
        print("[6/6] グリッドアニメーションをスキップしました")
    else:
        print("[6/6] パラメータ組み合わせ別グリッドアニメーションを生成中 ...")
        save_grid_animation(
            sweep_dir, df, sweep_type, sweep_cols,
            os.path.join(out_dir, "sweep_grid_animation.gif"),
            seed=args.grid_seed,
            fps=args.fps,
            max_frames=args.max_frames,
            subtitle=subtitle,
        )

    print("---------------------------------------------------")
    print("完了．出力ファイル一覧:")
    for f in sorted(os.listdir(out_dir)):
        fpath = os.path.join(out_dir, f)
        if os.path.isfile(fpath):
            size_kb = os.path.getsize(fpath) / 1024
            print(f"  {f:40s} ({size_kb:6.1f} KB)")


if __name__ == "__main__":
    main()
