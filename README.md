# replication-dinamic-models-of-segregation

Schelling (1971) "Dynamic Models of Segregation" の再現実験実装．

> **設計ドキュメント (DESIGN) はObsidianで管理しています．**
>
> 📝 `Obsidian > 研究/90_再現実装/Dynamic Models of Segregation.md`

## 参照論文

Schelling, T. C. (1971). Dynamic Models of Segregation.  
*Journal of Mathematical Sociology*, 1(2), 143–186.  
DOI: [10.1080/0022250X.1971.9989794](https://doi.org/10.1080/0022250X.1971.9989794)

---

## 実行方法

### 1. シミュレーション (Rust)

```bash
# ビルド
cargo build --release

# 標準設定で実行 (13×16グリッド, τ=1/3)
cargo run --release

# パラメータを指定して実行
cargo run --release -- \
    --rows 20 --cols 20 \
    --threshold 0.5 \
    --seed 42 \
    --output_dir results
```

**主なオプション:**

| オプション | デフォルト | 説明 |
|-----------|-----------|------|
| `--rows` | 13 | グリッド行数 |
| `--cols` | 16 | グリッド列数 |
| `--threshold` | 0.333 | 許容限界 τ |
| `--vacant_rate` | 0.30 | 空き地率 |
| `--seed` | — | 乱数シード |
| `--snapshot_interval` | 1 | スナップショット保存間隔 |
| `--output_dir` | `results` | 出力先ディレクトリ |

**出力ファイル:**

```
results/
├── metrics.csv          ← ステップごとの分離度指標
└── snapshots/
    ├── step_00000.csv   ← 初期状態
    ├── step_00001.csv
    └── ...
```

---

### 2. 可視化 (Python)

Python依存管理には [uv](https://docs.astral.sh/uv/) を使用．

```bash
# 依存パッケージのインストール
uv sync

# 可視化を実行 (シミュレーション後に実行)
uv run python analysis/visualize.py

# アニメーションのフレーム数を制限して高速生成
uv run python analysis/visualize.py --max_frames 30 --fps 8

# アニメーションをスキップして静止画のみ生成
uv run python analysis/visualize.py --no_animation
```

**出力ファイル:**

```
results/figures/
├── animation.gif          ← グリッド進化のアニメーション
├── initial_state.png      ← 初期状態のグリッド
├── final_state.png        ← 最終状態のグリッド
├── comparison.png         ← 初期・中間・最終の3ショット比較
└── metrics_timeseries.png ← メトリクス時系列グラフ
```

---

## ライセンス

MIT
