# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

Schelling (1971) "Dynamic Models of Segregation" の再現実験．Rustでシミュレーションを実行し，Pythonで可視化する2層構成．

設計ドキュメントはObsidianで管理: `Obsidian > 研究/90_再現実装/Dynamic Models of Segregation.md`

## Build & Run

```bash
# ビルド
cargo build --release

# 標準設定で実行 (13x16グリッド, τ=1/3, seed=42)
cargo run --release

# パラメータ指定
cargo run --release -- --rows 20 --cols 20 --threshold 0.5 --seed 42

# 可視化 (シミュレーション実行後，results/latest 経由で最新結果を自動参照)
uv sync
uv run python analysis/visualize.py

# アニメーションなしで高速可視化
uv run python analysis/visualize.py --no_animation

# 特定の実行結果を可視化
uv run python analysis/visualize.py --results_dir results/20260405_153000
```

## Testing & Linting

```bash
cargo test
cargo clippy
cargo fmt --check
```

## Architecture

シミュレーションのパイプライン: `main.rs`(CLI引数→Config) → `simulation::run`(メインループ) → 結果を`results/`に出力 → `visualize.py`で図を生成

### Rust (src/)

- **config.rs** — `Config`構造体．デフォルト値はSchelling論文のFigure 7-10に準拠（13x16グリッド，τ=1/3，空き地率30%）
- **grid.rs** — `Grid`と`Cell`(GroupA/GroupB/Empty)．ムーア近傍（8近傍）の同色比率計算，移動先の仮想比率シミュレーション(`simulated_ratio`)，チェビシェフ距離
- **metrics.rs** — `Metrics`構造体．各ステップの分離度指標（平均同色比率，異色近隣なし割合，非類似性指数D，不満足数，移動数）を集計
- **simulation.rs** — メインループ．不満足エージェントをランダム順で処理し，チェビシェフ距離昇順で最近接の満足可能な空きセルへ移動．移動0またはn_dissatisfied=0で収束

### Python (analysis/)

- **visualize.py** — `results/latest/snapshots/`のCSVと`results/latest/metrics.csv`を読み込み，グリッドヒートマップ・メトリクス時系列・GIFアニメーションを生成．`--results_dir`で特定の実行結果を指定可能

### 出力構造

各実行はタイムスタンプ付きサブディレクトリに保存される．`results/latest`は最新の実行へのシンボリックリンク．

```
results/
├── latest -> 20260405_153000       # 最新実行へのシンボリックリンク
├── 20260405_153000/                # タイムスタンプ付き実行結果
│   ├── metrics.csv                 # ステップごとの分離度指標
│   ├── snapshots/step_*.csv        # グリッド状態 (row,col,cell: 0=空,1=A,2=B)
│   └── figures/                    # 可視化出力 (visualize.py生成)
└── 20260405_160000/                # 別の実行結果
    └── ...
```

## Key Design Decisions

- 乱数: `ChaCha8Rng` (決定的再現性のため)
- 移動ルール: 不満足エージェントは最近接（チェビシェフ距離昇順）の満足可能な空きセルに移動．移動前に`simulated_ratio`で移動先の比率を事前評価
- 収束条件: 不満足エージェント数=0，または移動可能なエージェント数=0
- CSV出力にはserdeとcsvクレートを使用
