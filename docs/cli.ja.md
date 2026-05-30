[English](cli.md) | **日本語**

# Rust CLI

`schelling-simulation` クレートは `run` / `sweep` / `bnm` / `bnm-basin` / `tipping` のサブコマンドを持つ CLI を公開しています．まず `cargo build --release` でビルドし，ワークスペースルートから `cargo run --release -- <サブコマンド> ...` で実行します．

## run（空間近接モデル）

2次元グリッド上のエージェント動学．

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
| `--move-mode` | `standard` | 移動運用モード（後述）: `standard` / `strict` |
| `--move-strategy` | `nearest` | 移動先選択戦略: `nearest` / `best-local` |
| `--vacant-rate` | 0.30 | 空き地率 |
| `--seed` | — | 乱数シード |
| `--snapshot-interval` | 1 | スナップショット保存間隔（0=保存しない） |
| `--output-dir` | `results` | 出力先ディレクトリ |

### 満足判定ルール (`--rule`)

論文の3種類の選好形式を `--rule` フラグで切り替えられます．未指定時は `--threshold` から `ratio` ルールが構築されます．

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

### 移動運用モード (`--move-mode`)

論文はエージェントの移動形式を 2 種類に区別しています．

| モード | 意味 | 対応論文図 |
|---|---|---|
| `standard` | 緩運用: 不満足なエージェントのみ移動（デフォルト） | Fig. 9–14 |
| `strict` | 厳格運用: 不満足者に加えて，満足しているエージェントも同色比率を厳密に改善できる空きセルへ投機的に移動する | Fig. 8 |

```bash
# 厳格運用 (Fig. 8): 緩運用よりはるかに鋭い分離が起こる
cargo run --release -- run --threshold 0.5 --move-mode strict
```

厳格運用では満足者がより同質な近隣を求め続けるため，緩運用より分離度が著しく高くなります．不満足移動も投機移動も発生しなくなった時点で停止します．

### 移動先選択戦略 (`--move-strategy`)

移動するエージェントがどの満足できる空きセルを選ぶかを制御します．

| 戦略 | 意味 | 対応論文図 |
|---|---|---|
| `nearest` | チェビシェフ距離昇順で最初に満足できる空きセル（デフォルト） | Fig. 7–14 |
| `best-local` | 満足できる全空きセルのうち移動後同色比率が最大のセル（同比率は近い順→行優先順で先勝ち） | Fig. 12 |

`best-local` は少数派が最も同質な区画へ寄り集まるよう働き，不等数 (Fig. 12) の少数派クラスタ比率を論文値 (> 80%) に近づけます．

```bash
# 不等数 (Fig. 12), best-local 戦略で少数派クラスタを締める
cargo run --release -- run --n-a 97 --n-b 49 --threshold 0.333 --move-strategy best-local
```

`--move-mode` と `--move-strategy` は独立で，併用できます．`sweep` サブコマンドは常に `standard` / `nearest` を用います．

**出力ファイル:**

各実行はタイムスタンプ付きサブディレクトリに保存されます．`results/latest` は最新の実行へのシンボリックリンクです．

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

`results/{timestamp}/` には `config.json` も生成されます．詳細は [`show-experiment-settings`](visualization.ja.md#show-experiment-settings) を参照．

## sweep（パラメータスイープ）

パラメータの範囲を `start:stop:step` 形式で指定し，グリッドサーチを実行します（sweep は `ratio` ルール専用）．

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

**sweep オプション:**

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

## 解析モデル — 境界近隣モデル (BNM) とティッピングモデル

論文 §3–§4 の解析モデル．状態を集計人口 $(W, B)$ に縮約し，許容限界スケジュールから反応曲線を導出して位相平面で動学を解析します．空間配置は扱いません．

### bnm / bnm-basin（境界近隣モデル）

```bash
# 境界近隣モデルの単発実行 (位相図 + 軌跡)
cargo run --release -- bnm --preset fig18 --init 50,25
cargo run --release -- bnm --preset fig19 --init 60,60   # 安定混合へ収束

# 吸引域解析: 初期条件グリッドを掃いて basin マップを生成
cargo run --release -- bnm-basin --preset fig19 --init-grid 30x30
```

### tipping（ティッピングモデル）

```bash
# ティッピングモデル (投機・非対称・チャネリング込み)
cargo run --release -- tipping --preset fig31 --init 100,15
cargo run --release -- tipping --preset fig30a --speculation linear:alpha=0.3
cargo run --release -- tipping --preset fig31 --asymmetry "w_in=0.5:w_out=2.0:b_in=1.0:b_out=1.0"
```

### プリセット

| キー | 図 | 構造 | 期待される平衡 |
|---|---|---|---|
| `fig18` | Fig. 18 | 直線型・1:2 比 | 端点2安定 + 不安定混合 |
| `fig19` | Fig. 19 | 急勾配 (中央値=1.5) | 端点2 + 安定混合 |
| `fig20` | Fig. 20 | 緩勾配の直線型 (R_max=3, 対称) | 反応曲線の頂点が上がる（混合域が拡がる） |
| `fig21` | Fig. 21 | 急勾配の直線型 (R_max=1, 対称) | 反応曲線の頂点が下がる（分離が強まる） |
| `fig22` | Fig. 22 | 不等数で曲線非交差 | 混合均衡なし |
| `fig23` | Fig. 23 | 入域上限クオータ | クオータが混合を生む |
| `fig24` | Fig. 24 | 非対称許容 (W R_max=2, B R_max=1) | 混合均衡が偏在する |
| `fig25` | Fig. 25 | ゼロ許容者切片付きアフィン | 端点流出が強まる |
| `fig26` | Fig. 26 | 容量制約 C=120 | 混合均衡が容量線上に乗る |
| `fig27` | Fig. 27 | 区分線形 (S字CDF) | 非一様な許容分布 |
| `fig28` | Fig. 28 | 不等数 + 少数派が寛容 (R_max=4) | 混合均衡が生き残る |
| `fig29` | Fig. 29 | 強いクオータ (B pop_max=20) | 混合均衡が低 B 域に固定される |
| `fig30a` | Fig. 30a | B が極めて寛容 | in-tipping のみ |
| `fig30b` | Fig. 30b | Fig.18 と同構造 | out-tipping のみ |
| `fig31` | Fig. 31 | W 不寛容 + B 寛容 | 両ティッピング (典型ホワイトフライト) |
| `fig32` | Fig. 32 | Fig.19 と同構造 | ティッピングなし |

### スケジュール手動指定

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

### 出力ファイル (BNM)

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

吸引域解析時は `basin.csv` が，ティッピング時は `tipping_classification.json` が追加されます．

### 可視化

```bash
# BNM (反応曲線・位相図・軌跡・吸引域)
uv run schelling-tools visualize-bnm

# Tipping (上記 + ティッピング類型注釈)
uv run schelling-tools visualize-tipping
```

詳細と出力の解釈は [可視化](visualization.ja.md) を参照してください．
