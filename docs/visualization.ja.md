[English](visualization.md) | **日本語**

# 可視化 (Python)

Python ツールは `schelling-tools` パッケージにまとまっています．Python 依存管理には [uv](https://docs.astral.sh/uv/) を使用します．ワークスペースルートで `uv sync` すると `tools` 配下の `schelling-tools` パッケージがエディタブルインストールされ，`schelling-tools` コマンドが使えるようになります．

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

## スイープ結果の可視化

`schelling-tools visualize-sweep` はスイープ結果（`sweep_summary.csv`）を読み込み，パラメータと最終メトリクスの関係を可視化します．1Dスイープ（1パラメータのみ変化）と2Dスイープ（2パラメータが変化）を自動判別します．

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

- **1Dスイープ**: 折れ線グラフ．複数シードの場合は平均線＋標準偏差のエラーバー＋個別点をプロット．
- **2Dスイープ**: ヒートマップ．セル内に数値をアノテーション．
- **グリッドアニメーション**: 各セルが (τ, vacant_rate) 一組の run のスナップショットを再生する合成 GIF．2D スイープでは行=vacant_rate, 列=threshold で配置．1D スイープでは横一列〜矩形に折りたたんで配置．収束ステップ数の異なる run は最終フレームを保持して同期する．スナップショットが無い run（sweep を `--snapshot-interval 0` で実行した場合）は空セルとして表示される．

## 解析モデル可視化 (BNM / Tipping)

```bash
# BNM (反応曲線・位相図・軌跡・吸引域)
uv run schelling-tools visualize-bnm

# Tipping (上記 + ティッピング類型注釈)
uv run schelling-tools visualize-tipping
```

## show-experiment-settings

`schelling-tools show-experiment-settings` は (1) 論文再現実験の定義一覧，または (2) 既存実行結果ディレクトリで使われた設定値を表示します．

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

`run` 実行時は `results/{timestamp}/config.json` が，`sweep` 実行時は `results/{timestamp}_sweep/sweep_config.json` が自動生成されます．両者は本コマンドが自動判別して整形表示します．

> **注**: 旧バージョン（`config.json` 出力対応前）で生成された結果ディレクトリには設定ファイルが含まれていないため，`--results-dir` モードでは表示できません．その場合は再実行してください．

## 出力の解釈

### metrics.csv

各ステップにおける分離度指標を記録したCSVファイルです．

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
- **τ感度解析の非線形性**: Fig. 14 スイープでは，τ=0.35付近までは avg_same がゆるやかに上昇し，τ=0.45–0.55 付近で急峻に 0.80→0.90 に跳ね上がる．Schellingが論文中で強調した「ミクロ選好とマクロ結果の非対応性」の核心的エビデンス．
- **集会選好 vs 分離選好の区別不能性**: `--rule min-same:3` と `--rule ratio:0.4` はほぼ同じ均衡同色比率（≈0.78）を示す．これは論文Fig.16の「集結志向も分離志向もマクロでは同等の分離を生む」という主要知見に対応．
- **統合選好の収束困難性**: `--rule bounded:3:6` では一部のシードで収束が遅く（15ステップ以上），論文が指摘する「dead space 形成」と整合．
- **収束の判定**: `n_dissatisfied=0`（全員が満足）または `n_moved=0`（移動先が見つからない）でシミュレーションが終了する．

#### 解析モデル (BNM / Tipping) の典型結果

- **Fig.18 (直線型, 1:2 比)**: 端点 `(W_max, 0)` と `(0, B_max)` のみが安定．混合均衡 `(21.7, 34.0)` は不安定．「混合は静的には可能だが動的に維持できない」という Schelling の核心的命題に対応．
- **Fig.19 (急勾配, 中央値=1.5)**: 端点 2 つに加え対称混合均衡 `(60, 60)` が安定．非対称混合 `(27.6, 72.4)` と `(72.4, 27.6)` は鞍点．初期 `(30, 30)` から `(60, 60)` へ滑らかに収束する軌跡が見られる．
- **Fig.31 (両ティッピング)**: $W$ 不寛容 + $B$ 寛容で B 反応曲線の頂点が $W_{\max}$ を超える．初期 `(100, 15)` から all_black `(0, 50)` へ大きく曲がる軌跡（ホワイトフライト動学）が観察される．
- **Fig.32 (ティッピングなし)**: 安定混合均衡が存在し，端点も安定．基本的にどの初期条件も最も近い安定均衡に収束する頑健な多相安定．
- **basin 解析の見方**: `bnm-basin` で得られる `basin_of_attraction.png` は，各色領域の面積比率がそのまま「ランダム初期化したときに各均衡へ落ちる確率」に対応．Fig.19 では混合均衡の basin が最大（≈ 50%）．
