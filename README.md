# replication-dinamic-models-of-segregation

Schelling (1971) "Dynamic Models of Segregation" の再現実験実装．

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

各実行はタイムスタンプ付きサブディレクトリに保存される．`results/latest`は最新の実行へのシンボリックリンク．

```
results/
├── latest -> 20260405_153000       ← 最新実行へのシンボリックリンク
├── 20260405_153000/
│   ├── metrics.csv                 ← ステップごとの分離度指標
│   └── snapshots/
│       ├── step_00000.csv          ← 初期状態
│       ├── step_00001.csv
│       └── ...
└── 20260405_160000/
    └── ...
```

---

### 2. パラメータスイープ（感度分析）

パラメータの範囲を `start:stop:step` 形式で指定し，グリッドサーチを実行する．

```bash
# τ を 0.1〜0.9 まで 0.1 刻みでスイープ
cargo run --release -- sweep --threshold 0.1:0.9:0.1

# τ と空き地率の2次元スイープ
cargo run --release -- sweep --threshold 0.1:0.5:0.1 --vacant_rate 0.2:0.4:0.1

# 複数シードで統計的安定性を確認
cargo run --release -- sweep --threshold 0.1:0.9:0.1 --seeds 42,123,456

# グリッドサイズを変更してスイープ
cargo run --release -- sweep --threshold 0.1:0.9:0.1 --rows 20 --cols 20
```

**sweepオプション:**

| オプション | デフォルト | 説明 |
|-----------|-----------|------|
| `--threshold` | 0.333 | τ の範囲（`start:stop:step`）または単一値 |
| `--vacant_rate` | 0.30 | 空き地率の範囲（`start:stop:step`）または単一値 |
| `--rows` | 13 | グリッド行数 |
| `--cols` | 16 | グリッド列数 |
| `--seeds` | 42 | カンマ区切りの乱数シード |
| `--max_iterations` | 500 | 最大反復回数 |
| `--snapshot_interval` | 0 | スナップショット保存間隔（0=保存しない） |
| `--output_dir` | `results` | 出力先ベースディレクトリ |

**出力ファイル:**

```
results/{timestamp}_sweep/
├── sweep_summary.csv                ← 全パラメータ組み合わせの最終メトリクス
├── sweep_config.json                ← スイープ設定（再現用）
├── tau_0.100_vac_0.300_seed_42/
│   └── metrics.csv
├── tau_0.200_vac_0.300_seed_42/
│   └── metrics.csv
└── ...
```

---

### 3. 可視化 (Python)

Python依存管理には [uv](https://docs.astral.sh/uv/) を使用．

```bash
# 依存パッケージのインストール
uv sync

# 可視化を実行 (最新の実行結果を自動参照)
uv run python analysis/visualize.py

# アニメーションのフレーム数を制限して高速生成
uv run python analysis/visualize.py --max_frames 30 --fps 8

# アニメーションをスキップして静止画のみ生成
uv run python analysis/visualize.py --no_animation

# 特定の実行結果を可視化
uv run python analysis/visualize.py --results_dir results/20260405_153000
```

**出力ファイル:**

```
results/latest/figures/
├── animation.gif          ← グリッド進化のアニメーション
├── initial_state.png      ← 初期状態のグリッド
├── final_state.png        ← 最終状態のグリッド
├── comparison.png         ← 初期・中間・最終の3ショット比較
└── metrics_timeseries.png ← メトリクス時系列グラフ
```

---

## 出力の解釈

### metrics.csv

各ステップにおける分離度指標を記録したCSVファイル．

| カラム | 説明 | 値の範囲 | 読み方 |
|-------|------|---------|-------|
| `step` | シミュレーションステップ番号 | 0〜 | — |
| `avg_same_ratio` | 全エージェントの平均同色近隣比率 | 0.0〜1.0 | 高いほど分離が進行．ランダム配置では集団比率に近い値（≈0.5），収束時は0.6〜0.8程度になる |
| `avg_same_ratio_a` | 集団Aの平均同色近隣比率 | 0.0〜1.0 | 集団間の分離度の非対称性を確認するために使用 |
| `avg_same_ratio_b` | 集団Bの平均同色近隣比率 | 0.0〜1.0 | 同上 |
| `pct_no_opposite` | 異色近隣を持たないエージェントの割合 | 0〜100 (%) | 高いほど同色のみに囲まれたエージェントが多い＝分離が強い |
| `dissimilarity_index` | 非類似性指数 D（簡易版） | 0.0〜0.5 | 格子全体を1ゾーンとした D = 0.5 × |a/A − b/B|．集団サイズが均等なら≈0 |
| `n_dissatisfied` | 不満足エージェント数 | 0〜 | 同色近隣比率がτ未満のエージェント数．0になると収束 |
| `n_moved` | 移動したエージェント数 | 0〜 | 各ステップで実際に移動が成立した数．0になると収束 |

### 可視化出力 (results/latest/figures/)

| ファイル | 内容 | 見るポイント |
|---------|------|------------|
| `initial_state.png` | 初期配置のグリッド | ランダム配置であることを確認．青=集団A，赤=集団B，白=空きセル |
| `final_state.png` | 収束後のグリッド | 同色エージェントのクラスター形成を観察．τが低くても顕著な分離が生じる点がSchellingの主要な知見 |
| `comparison.png` | 初期・中間・最終の3ショット比較 | 分離の進行過程を概観．初期のランダム配置から徐々にクラスターが成長する様子を確認 |
| `metrics_timeseries.png` | メトリクス時系列（4パネル） | 左上: 平均同色比率の上昇カーブ，右上: 異色近隣なし割合の推移，左下: 不満足数・移動数の減衰，右下: 非類似性指数D |
| `animation.gif` | グリッド進化のアニメーション | 左パネルでエージェントの移動，右パネルでメトリクスの変化をステップごとに追跡 |

### 典型的な結果の読み方

- **τ=1/3（デフォルト）の場合**: 各エージェントは近隣の1/3以上が同色であれば満足する緩い条件だが，avg_same_ratioは初期の≈0.50から≈0.65以上まで上昇し，マクロレベルでは顕著なクラスターが形成される．これがSchellingの「穏やかな個人選好がマクロな分離を生む」という主張の核心．
- **収束の判定**: `n_dissatisfied=0`（全員が満足）または`n_moved=0`（移動先が見つからない）でシミュレーションが終了する．

---

## ライセンス

MIT
