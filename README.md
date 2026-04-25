# replication-dinamic-models-of-segregation

Schelling (1971) "Dynamic Models of Segregation" の再現実験実装．論文の3つのモデルをすべて実装する：

1. **空間近接モデル** (Spatial Proximity Model, Fig.7–17) — 2次元グリッド上のエージェント動学．
2. **境界近隣モデル** (Bounded-Neighborhood Model, Fig.18–29) — 集計人口 $(W,B)$ の位相平面解析．
3. **ティッピングモデル** (Tipping Model, Fig.30–32) — BNM の住宅市場応用 (投機・非対称・類型分類)．

## 参照論文

Schelling, T. C. (1971). Dynamic Models of Segregation.  
*Journal of Mathematical Sociology*, 1(2), 143–186.  
DOI: [10.1080/0022250X.1971.9989794](https://doi.org/10.1080/0022250X.1971.9989794)

---

## リポジトリ構成

Cargo workspace + uv workspace の 2 プロジェクト構成．

```
schelling1971/
├── Cargo.toml                 ← Cargo workspace ルート
├── pyproject.toml             ← uv workspace ルート
├── simulation/                ← Rust プロジェクト (schelling-simulation)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs                     ← CLI (run / sweep / bnm / bnm-basin / tipping)
│       ├── config.rs                   ← 空間モデル設定
│       ├── grid.rs                     ← グリッド + ムーア近傍
│       ├── metrics.rs                  ← 分離度指標
│       ├── simulation.rs               ← 空間モデル動学
│       └── analytic/                   ← 解析モデル (BNM + Tipping)
│           ├── tolerance.rs            ← 許容限界スケジュール (CDF)
│           ├── reaction.rs             ← 反応曲線 B_W(W)
│           ├── phase.rs                ← 平衡点・安定性・ベクトル場
│           ├── dynamics.rs             ← 動学積分 + 吸引域解析
│           ├── tipping.rs              ← 投機・非対称・類型分類
│           ├── preset.rs               ← Fig.18-32 プリセット
│           └── runner.rs               ← CLI I/O オーケストレーション
├── tools/                     ← Python プロジェクト (schelling-tools)
│   ├── pyproject.toml
│   └── src/schelling_tools/
│       ├── cli.py                       ← 統合 CLI (schelling-tools)
│       ├── visualize.py                 ← 空間モデル可視化
│       ├── visualize_sweep.py           ← スイープ可視化
│       ├── visualize_bnm.py             ← BNM 位相図・軌跡・吸引域
│       ├── visualize_tipping.py         ← Tipping 可視化 + 類型注釈
│       ├── reproduce_paper.py           ← 論文 Fig.7-32 一括再現
│       └── show_experiment_settings.py  ← 実験設定値の表示
└── results/                   ← シミュレーション出力 (gitignored)
```

- `cargo run` はワークスペースルートから `simulation` クレートを起動する（`-p schelling-simulation` は省略可，メンバーが 1 つのため）．
- `uv run` は uv workspace のメンバー `tools` が公開する `schelling-tools` コマンドを呼び出す．

---

## 実行方法

### 1. シミュレーション (Rust)

```bash
# ビルド
cargo build --release

# 標準設定で実行 (13×16グリッド, τ=1/3, seed=42)
cargo run --release

# パラメータを指定して実行
cargo run --release -- run \
    --rows 20 --cols 20 \
    --threshold 0.5 \
    --seed 42 \
    --output-dir results
```

**主なオプション:**

| オプション | デフォルト | 説明 |
|-----------|-----------|------|
| `--rows` | 13 | グリッド行数 |
| `--cols` | 16 | グリッド列数 |
| `--n-a`, `--n-b` | 0 (自動) | 各集団のエージェント数（0なら `--vacant-rate` から等数で自動計算） |
| `--threshold` | 0.333 | 許容限界 τ（`--rule` 未指定時のみ使用） |
| `--rule` | — | 満足判定ルール文字列（後述） |
| `--vacant-rate` | 0.30 | 空き地率 |
| `--seed` | — | 乱数シード |
| `--snapshot-interval` | 1 | スナップショット保存間隔（0=保存しない） |
| `--output-dir` | `results` | 出力先ディレクトリ |

#### 満足判定ルール (`--rule`)

論文の3種類の選好形式を `--rule` フラグで切り替えられる．未指定時は `--threshold` から `ratio` ルールが構築される．

| ルール | 形式 | 意味 | 対応論文図 |
|---|---|---|---|
| 分離型 | `ratio:X` | 同色近隣比率 ≥ X | Fig. 7–14（デフォルト） |
| 集会型 | `min-same:N` | 同色近隣の絶対数 ≥ N | Fig. 16 |
| 統合型 | `bounded:L:H` | 同色近隣の絶対数が L–H の範囲 | Fig. 17 |

```bash
# 分離型（既存動作と等価）
cargo run --release -- run --rule ratio:0.333

# 集会選好（同色が絶対数3人以上で満足）
cargo run --release -- run --rule min-same:3

# 統合選好（同色が3〜6人なら満足，多すぎても移動する）
cargo run --release -- run --rule bounded:3:6
```

**出力ファイル:**

各実行はタイムスタンプ付きサブディレクトリに保存される．`results/latest` は最新の実行へのシンボリックリンク．

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

パラメータの範囲を `start:stop:step` 形式で指定し，グリッドサーチを実行する（sweepは`ratio`ルール専用）．

```bash
# τ を 0.1〜0.9 まで 0.1 刻みでスイープ
cargo run --release -- sweep --threshold 0.1:0.9:0.1

# τ と空き地率の2次元スイープ
cargo run --release -- sweep --threshold 0.1:0.5:0.1 --vacant-rate 0.2:0.4:0.1

# 複数シードで統計的安定性を確認
cargo run --release -- sweep --threshold 0.1:0.9:0.1 --seeds 42,123,456

# グリッドサイズを変更してスイープ
cargo run --release -- sweep --threshold 0.1:0.9:0.1 --rows 20 --cols 20
```

**sweepオプション:**

| オプション | デフォルト | 説明 |
|-----------|-----------|------|
| `--threshold` | 0.333 | τ の範囲（`start:stop:step`）または単一値 |
| `--vacant-rate` | 0.30 | 空き地率の範囲（`start:stop:step`）または単一値 |
| `--rows` | 13 | グリッド行数 |
| `--cols` | 16 | グリッド列数 |
| `--seeds` | 42 | カンマ区切りの乱数シード |
| `--max-iterations` | 500 | 最大反復回数 |
| `--snapshot-interval` | 0 | スナップショット保存間隔（0=保存しない） |
| `--output-dir` | `results` | 出力先ベースディレクトリ |

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

### 3. 解析モデル — 境界近隣モデル (BNM) とティッピングモデル

論文 §3–§4 の解析モデル．状態を集計人口 $(W, B)$ に縮約し，許容限界スケジュールから反応曲線を導出して位相平面で動学を解析する．空間配置は扱わない．

#### サブコマンド

```bash
# 境界近隣モデルの単発実行 (位相図 + 軌跡)
cargo run --release -- bnm --preset fig18 --init 50,25
cargo run --release -- bnm --preset fig19 --init 60,60   # 安定混合へ収束

# 吸引域解析: 初期条件グリッドを掃いて basin マップを生成
cargo run --release -- bnm-basin --preset fig19 --init-grid 30x30

# ティッピングモデル (投機・非対称・チャネリング込み)
cargo run --release -- tipping --preset fig31 --init 100,15
cargo run --release -- tipping --preset fig30a --speculation linear:alpha=0.3
cargo run --release -- tipping --preset fig31 --asymmetry "w_in=0.5:w_out=2.0:b_in=1.0:b_out=1.0"
```

#### プリセット

| キー | 図 | 構造 | 期待される平衡 |
|---|---|---|---|
| `fig18` | Fig. 18 | 直線型・1:2 比 | 端点2安定 + 不安定混合 |
| `fig19` | Fig. 19 | 急勾配 (中央値=1.5) | 端点2 + 安定混合 |
| `fig22` | Fig. 22 | 不等数で曲線非交差 | 混合均衡なし |
| `fig23` | Fig. 23 | 入域上限クオータ | クオータが混合を生む |
| `fig30a` | Fig. 30a | B が極めて寛容 | in-tipping のみ |
| `fig30b` | Fig. 30b | Fig.18 と同構造 | out-tipping のみ |
| `fig31` | Fig. 31 | W 不寛容 + B 寛容 | 両ティッピング (典型ホワイトフライト) |
| `fig32` | Fig. 32 | Fig.19 と同構造 | ティッピングなし |

#### スケジュール手動指定

```bash
# 直線型: F(R) = (R/r_max)*pop_max
cargo run --release -- bnm \
  --w-tolerance "linear:r_max=2.0:pop_max=100" \
  --b-tolerance "linear:r_max=2.0:pop_max=50" \
  --init 50,25

# アフィン型: F(R) = clamp(intercept_pop + slope*R, 0, pop_max)
cargo run --release -- bnm \
  --w-tolerance "affine:intercept_pop=20:slope=20:pop_max=100" \
  --b-tolerance "affine:intercept_pop=20:slope=20:pop_max=100" \
  --init 60,60
```

#### 出力ファイル (BNM)

```
results/{timestamp}_bnm/
├── config.json                  # phase / dynamics / init / preset
├── tolerance_w.csv              # CDF F_W(R)
├── tolerance_b.csv
├── reaction_curve_w.csv         # (W, B_W(W))
├── reaction_curve_b.csv         # (B, W_B(B))
├── equilibria.csv               # (w, b, kind, stability)
├── vector_field.csv             # (w, b, dw_sign, db_sign, region)
└── trajectory.csv               # (t, w, b)
```

吸引域解析時は `basin.csv` が，ティッピング時は `tipping_classification.json` が追加される．

#### 可視化

```bash
# BNM (反応曲線・位相図・軌跡・吸引域)
uv run schelling-tools visualize-bnm

# Tipping (上記 + ティッピング類型注釈)
uv run schelling-tools visualize-tipping
```

---

### 4. 論文再現実験（Fig. 7–32）

`schelling-tools reproduce` は論文で報告された主要実験を一括で実行し，各図の報告値との比較レポートを生成する．

```bash
# 全実験を5シードで再現（推奨）
uv run schelling-tools reproduce

# シード指定
uv run schelling-tools reproduce --seeds 42,123,456,789,2024

# 特定実験のみ実行（カンマ区切り可）
uv run schelling-tools reproduce --only fig11_tau_one_third,fig16_congregationist_min_same_3

# τ感度解析をスキップして高速化
uv run schelling-tools reproduce --skip-sweep

# 解析モデル (Fig.18-32) をスキップ
uv run schelling-tools reproduce --skip-analytic

# 解析モデルのみ実行 (空間モデル + τ感度解析をスキップ)
uv run schelling-tools reproduce --analytic-only

# cargo build をスキップ（ビルド済みバイナリを使う）
uv run schelling-tools reproduce --skip-build
```

**再現対象:**

| キー | 図 | 設定 | 論文値 |
|---|---|---|---|
| `fig11_tau_one_third` | Fig. 11 | τ=1/3, 等数 | avg_same 65–75% |
| `fig09_tau_one_half_lenient` | Fig. 9 | τ=1/2, 等数（緩運用） | avg_same 80–83% |
| `fig08_tau_one_half_strict` | Fig. 8 | τ=1/2, 等数（厳格運用の近似） | avg_same 89–91% |
| `fig12_unequal_two_to_one` | Fig. 12 | τ=1/3, 不等数 97:49 | 少数派 >80% |
| `fig16_congregationist_min_same_3` | Fig. 16 | `min-same:3` | avg_same ≈75%, 異色近隣なし ≈38% |
| `fig17_integrationist_bounded_3_6` | Fig. 17 | `bounded:3:6` | 定性報告: dead space形成・収束困難 |
| `fig14_tau_sweep` | Fig. 14 | τ=0.10–0.60 (0.05刻み) | 0.35–0.50で急峻な上昇 |

**解析モデル (BNM + Tipping):**

| キー | 図 | 設定 | 期待される結果 |
|---|---|---|---|
| `fig18_linear_two_to_one` | Fig. 18 | 直線型・1:2 比 | 端点2安定 + 不安定混合 |
| `fig19_steep_three_stable` | Fig. 19 | 急勾配 (中央値=1.5) | 3 安定均衡，混合へ収束 |
| `fig22_unequal_no_intersection` | Fig. 22 | 不等数で曲線非交差 | 混合均衡なし |
| `fig23_limiting_numbers` | Fig. 23 | 入域上限クオータ | クオータが混合を生む |
| `fig30a_in_tipping_only` | Fig. 30a | B 極寛容 | in-tipping のみ |
| `fig30b_out_tipping_only` | Fig. 30b | Fig.18 と同構造 | out-tipping のみ |
| `fig31_both_tipping` | Fig. 31 | W 不寛容 + B 寛容 | 両ティッピング，all_black へ |
| `fig32_neither_tipping` | Fig. 32 | Fig.19 と同構造 | ティッピングなし，混合へ |

**注記:**
- **Fig. 8 の「厳格運用」は未再現**．現実装では満足エージェントは移動しない．論文の厳格版は投機的移動を伴う．
- **Fig. 17 は論文が定量値を示していない**ため，数値比較ではなく収束ステップ数の挙動（統合選好は収束困難）で論文挙動を確認する．

**出力ファイル:**

```
results/paper_reproduction/{timestamp}/
├── reproduction_summary.json       ← 構造化データ（per-seed メトリクスと集計）
├── reproduction_summary.csv        ← 表形式の per-seed 結果
├── reproduction_report.txt         ← コンソール出力と同じ比較レポート
├── fig11_tau_one_third/
│   └── seed_{N}/{timestamp}/metrics.csv
├── fig16_congregationist_min_same_3/
│   └── ...
└── fig14_tau_sweep/
    └── {timestamp}_sweep/sweep_summary.csv
```

---

### 5. 可視化 (Python)

Python依存管理には [uv](https://docs.astral.sh/uv/) を使用．ワークスペースルートで `uv sync` すると `tools` 配下の `schelling-tools` パッケージがエディタブルインストールされ，`schelling-tools` コマンドが使えるようになる．

```bash
# 依存パッケージのインストール (workspace ルートで実行)
uv sync

# 可視化を実行 (最新の実行結果を自動参照)
uv run schelling-tools visualize

# アニメーションのフレーム数を制限して高速生成
uv run schelling-tools visualize --max_frames 30 --fps 8

# アニメーションをスキップして静止画のみ生成
uv run schelling-tools visualize --no_animation

# 特定の実行結果を可視化
uv run schelling-tools visualize --results_dir results/20260405_153000
```

**出力ファイル（単一実行）:**

```
results/latest/figures/
├── animation.gif          ← グリッド進化のアニメーション
├── initial_state.png      ← 初期状態のグリッド
├── final_state.png        ← 最終状態のグリッド
├── comparison.png         ← 初期・中間・最終の3ショット比較
└── metrics_timeseries.png ← メトリクス時系列グラフ
```

#### スイープ結果の可視化

`schelling-tools visualize-sweep` はスイープ結果（`sweep_summary.csv`）を読み込み，パラメータと最終メトリクスの関係を可視化する．1Dスイープ（1パラメータのみ変化）と2Dスイープ（2パラメータが変化）を自動判別する．

```bash
# 最新のスイープ結果を可視化（results/latest 経由）
uv run schelling-tools visualize-sweep

# 特定のスイープ結果を指定
uv run schelling-tools visualize-sweep --sweep_dir results/20260405_161446_sweep
```

**主なオプション:**

| オプション | デフォルト | 説明 |
|-----------|-----------|------|
| `--sweep_dir` | `results/latest` | スイープ結果ディレクトリ |
| `--output_dir` | `{sweep_dir}/figures` | 図の保存先 |
| `--no_grid_animation` | off | パラメータ組み合わせ別グリッドアニメーションの生成をスキップ |
| `--grid_seed` | 先頭シード | グリッドアニメーションで使用する seed |
| `--fps` | 5 | グリッドアニメーションの FPS |
| `--max_frames` | 0 (全フレーム) | グリッドアニメーションの最大フレーム数 |

**出力ファイル（スイープ）:**

```
results/latest/figures/
├── sweep_avg_same_ratio.png  ← 平均同色近隣比率（1D: 折れ線+エラーバー / 2D: ヒートマップ）
├── sweep_pct_no_opposite.png ← 異色近隣なし割合
├── sweep_convergence.png     ← 収束ステップ数
├── sweep_overview.png        ← 4指標の概要パネル（2×2）
└── sweep_grid_animation.gif  ← パラメータ組み合わせ別のグリッド進行アニメーション
                                （sweep を `--snapshot-interval N` (N>0) 付きで実行した場合のみ）
```

- **1Dスイープ**: 折れ線グラフ．複数シードの場合は平均線＋標準偏差のエラーバー＋個別点をプロット
- **2Dスイープ**: ヒートマップ．セル内に数値をアノテーション
- **グリッドアニメーション**: 各セルが (τ, vacant_rate) 一組の run のスナップショットを再生する合成 GIF．2D スイープでは行=vacant_rate, 列=threshold で配置．1D スイープでは横一列〜矩形に折りたたんで配置．収束ステップ数の異なる run は最終フレームを保持して同期する．スナップショットが無い run（sweep を `--snapshot-interval 0` で実行した場合）は空セルとして表示される

---

### 6. 実験設定値の表示

`schelling-tools show-experiment-settings` は (1) 論文再現実験の定義一覧，または (2) 既存実行結果ディレクトリで使われた設定値を表示する．

```bash
# 論文再現実験 (Fig. 7-17) の定義一覧を表示（reproduce 実行前のプレビュー用）
uv run schelling-tools show-experiment-settings

# 特定の実験キーのみ表示（カンマ区切り可）
uv run schelling-tools show-experiment-settings --only fig11_tau_one_third,fig16_congregationist_min_same_3

# 既存実行結果の設定を表示（results/latest 経由で最新を参照）
uv run schelling-tools show-experiment-settings --results-dir results/latest

# 特定の実行結果を指定（run / sweep どちらでも自動判別）
uv run schelling-tools show-experiment-settings --results-dir results/20260425_153000

# JSON 形式で出力
uv run schelling-tools show-experiment-settings --json
uv run schelling-tools show-experiment-settings --results-dir results/latest --json
```

`run` 実行時は `results/{timestamp}/config.json` が，`sweep` 実行時は `results/{timestamp}_sweep/sweep_config.json` が自動生成される．両者は本コマンドが自動判別して整形表示する．

> **注**: 旧バージョン（`config.json` 出力対応前）で生成された結果ディレクトリには設定ファイルが含まれていないため，`--results-dir` モードでは表示できない．その場合は再実行してください．

---

## 出力の解釈

### metrics.csv

各ステップにおける分離度指標を記録したCSVファイル．

| カラム | 説明 | 値の範囲 | 読み方 |
|-------|------|---------|-------|
| `step` | シミュレーションステップ番号 | 0〜 | — |
| `avg_same_ratio` | 全エージェントの平均同色近隣比率 | 0.0〜1.0 | 高いほど分離が進行．ランダム配置では集団比率に近い値（≈0.5），収束時は0.6〜0.9程度になる |
| `avg_same_ratio_a` | 集団Aの平均同色近隣比率 | 0.0〜1.0 | 集団間の分離度の非対称性を確認するために使用 |
| `avg_same_ratio_b` | 集団Bの平均同色近隣比率 | 0.0〜1.0 | 同上 |
| `pct_no_opposite` | 異色近隣を持たないエージェントの割合 | 0〜100 (%) | 高いほど同色のみに囲まれたエージェントが多い＝分離が強い |
| `dissimilarity_index` | 非類似性指数 D（簡易版） | 0.0〜0.5 | 格子全体を1ゾーンとした D = 0.5 × \|a/A − b/B\|．集団サイズが均等なら≈0 |
| `n_dissatisfied` | 不満足エージェント数 | 0〜 | ルール上不満足と判定されたエージェント数．0になると収束 |
| `n_moved` | 移動したエージェント数 | 0〜 | 各ステップで実際に移動が成立した数．0になると収束 |

### 可視化出力 (results/latest/figures/)

| ファイル | 内容 | 見るポイント |
|---------|------|------------|
| `initial_state.png` | 初期配置のグリッド | ランダム配置であることを確認．青=集団A，赤=集団B，白=空きセル |
| `final_state.png` | 収束後のグリッド | 同色エージェントのクラスター形成を観察．τが低くても顕著な分離が生じる点がSchellingの主要な知見 |
| `comparison.png` | 初期・中間・最終の3ショット比較 | 分離の進行過程を概観．初期のランダム配置から徐々にクラスターが成長する様子を確認 |
| `metrics_timeseries.png` | メトリクス時系列（4パネル） | 左上: 平均同色比率の上昇カーブ，右上: 異色近隣なし割合の推移，左下: 不満足数・移動数の減衰，右下: 非類似性指数D |
| `animation.gif` | グリッド進化のアニメーション | 左パネルでエージェントの移動，右パネルでメトリクスの変化をステップごとに追跡 |

### スイープ可視化出力

| ファイル | 内容 | 見るポイント |
|---------|------|------------|
| `sweep_avg_same_ratio.png` | パラメータ vs 平均同色近隣比率 | τの増加に伴う分離度の上昇カーブ．集団A/Bの差異も確認可能 |
| `sweep_pct_no_opposite.png` | パラメータ vs 異色近隣なし割合 | 完全な同色クラスターに囲まれたエージェントの増加傾向 |
| `sweep_convergence.png` | パラメータ vs 収束ステップ数 | τが中程度（0.4〜0.6）で収束に時間がかかり，高すぎると収束しない傾向 |
| `sweep_overview.png` | 4指標の概要パネル | 全体的なパラメータ感度を一覧で把握 |
| `sweep_grid_animation.gif` | 組み合わせ別グリッド進行アニメーション | 各セルが1つのパラメータ組み合わせの run を再生．パラメータが分離パターン形成にどう影響するかを横並びで比較できる |

### 解析モデル可視化出力 (BNM / Tipping)

| ファイル | 内容 | 見るポイント |
|---------|------|------------|
| `tolerance_schedules.png` | 許容限界 CDF $F_W(R), F_B(R)$ | スケジュール形状（直線型 / 急勾配 / 切片付き）を確認 |
| `reaction_curves.png` | 位相平面 $(W, B)$ 上の反応曲線 + 平衡点 | 放物線の交差点と端点の安定性（● 安定，× 不安定）を視認 |
| `phase_portrait.png` | 反応曲線 + ベクトル場（quiver）+ 平衡点 | 各領域の流れ方向（流入/退出）と平衡点の位置関係 |
| `trajectory.png` | 位相平面上の軌跡 | 初期点（★）から終点（●）への遷移経路．混合均衡へ収束するか端点へ吸い寄せられるか |
| `basin_of_attraction.png` | 初期条件の収束先による色分け（bnm-basin のみ） | 各安定均衡の吸引域の広さ・境界の位置 |
| `tipping_classification.png` | ティッピング類型注釈（tipping のみ） | `in_tipping_only` / `out_tipping_only` / `both` / `neither` の判定結果 |

### 典型的な結果の読み方

- **τ=1/3（デフォルト）の場合**: 各エージェントは近隣の1/3以上が同色であれば満足する緩い条件だが，avg_same_ratioは初期の≈0.50から≈0.65以上まで上昇し，マクロレベルでは顕著なクラスターが形成される．これがSchellingの「穏やかな個人選好がマクロな分離を生む」という主張の核心．
- **τ感度解析の非線形性**: `reproduce_paper.py` のFig. 14スイープでは，τ=0.35付近までは avg_same がゆるやかに上昇し，τ=0.45–0.55 付近で急峻に 0.80→0.90 に跳ね上がる．Schellingが論文中で強調した「ミクロ選好とマクロ結果の非対応性」の核心的エビデンス．
- **集会選好 vs 分離選好の区別不能性**: `--rule min-same:3` と `--rule ratio:0.4` はほぼ同じ均衡同色比率（≈0.78）を示す．これは論文Fig.16の「集結志向も分離志向もマクロでは同等の分離を生む」という主要知見に対応．
- **統合選好の収束困難性**: `--rule bounded:3:6` では一部のシードで収束が遅く（15ステップ以上），論文が指摘する「dead space 形成」と整合．
- **収束の判定**: `n_dissatisfied=0`（全員が満足）または `n_moved=0`（移動先が見つからない）でシミュレーションが終了する．

#### 解析モデル (BNM / Tipping) の典型結果

- **Fig.18 (直線型, 1:2 比)**: 端点 `(W_max, 0)` と `(0, B_max)` のみが安定．混合均衡 `(21.7, 34.0)` は不安定．「混合は静的には可能だが動的に維持できない」という Schelling の核心的命題に対応．
- **Fig.19 (急勾配, 中央値=1.5)**: 端点 2 つに加え対称混合均衡 `(60, 60)` が安定．非対称混合 `(27.6, 72.4)` と `(72.4, 27.6)` は鞍点．初期 `(30, 30)` から `(60, 60)` へ滑らかに収束する軌跡が見られる．
- **Fig.31 (両ティッピング)**: $W$ 不寛容 + $B$ 寛容で B 反応曲線の頂点が $W_{\max}$ を超える．初期 `(100, 15)` から all_black `(0, 50)` へ大きく曲がる軌跡（ホワイトフライト動学）が観察される．
- **Fig.32 (ティッピングなし)**: 安定混合均衡が存在し，端点も安定．基本的にどの初期条件も最も近い安定均衡に収束する頑健な多相安定．
- **basin 解析の見方**: `bnm-basin` で得られる `basin_of_attraction.png` は，各色領域の面積比率がそのまま「ランダム初期化したときに各均衡へ落ちる確率」に対応．Fig.19 では混合均衡の basin が最大（≈ 50%）．

---

## ライセンス

MIT

---
*This file was generated by Claude Code.*
