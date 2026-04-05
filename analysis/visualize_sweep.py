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
    └── sweep_overview.png        ← 2×2 パネル概要図
"""

from __future__ import annotations

import argparse
import json
import os
import sys

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
# メイン
# --------------------------------------------------------------------------- #


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description="Schelling 分離モデル パラメータスイープ 可視化スクリプト"
    )
    p.add_argument(
        "--sweep_dir", default="results/latest",
        help="スイープ結果のディレクトリ (default: results/latest)",
    )
    p.add_argument(
        "--output_dir", default=None,
        help="図の保存先ディレクトリ (default: {sweep_dir}/figures)",
    )
    return p.parse_args()


def main() -> None:
    args = parse_args()

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
    print("[1/5] sweep_summary.csv を読み込み中 ...")
    df = pd.read_csv(summary_path)
    print(f"      {len(df)} 行")

    # 設定読み込み
    print("[2/5] スイープ設定を確認中 ...")
    config = load_sweep_config(sweep_dir)
    sweep_type, sweep_cols = detect_sweep_type(df)
    subtitle = make_subtitle(config, df)
    print(f"      スイープ種別: {sweep_type} ({', '.join(sweep_cols)})")
    print(f"      {subtitle}")

    # 図の生成
    print("[3/5] 平均同色近隣比率を保存中 ...")
    save_avg_same_ratio(
        df, sweep_type, sweep_cols,
        os.path.join(out_dir, "sweep_avg_same_ratio.png"), subtitle,
    )

    print("[4/5] 異色近隣なし割合・収束ステップ数を保存中 ...")
    save_pct_no_opposite(
        df, sweep_type, sweep_cols,
        os.path.join(out_dir, "sweep_pct_no_opposite.png"), subtitle,
    )
    save_convergence(
        df, sweep_type, sweep_cols,
        os.path.join(out_dir, "sweep_convergence.png"), subtitle,
    )

    print("[5/5] 概要パネルを保存中 ...")
    save_overview(
        df, sweep_type, sweep_cols,
        os.path.join(out_dir, "sweep_overview.png"), subtitle,
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
