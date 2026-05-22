[English](architecture.md) | **日本語**

# アーキテクチャ

## リポジトリ構成

Cargo workspace + uv workspace の 2 プロジェクト構成です．

```
schelling1971/
├── Cargo.toml                 ← Cargo workspace ルート
├── pyproject.toml             ← uv workspace ルート
├── simulation/                ← Rust プロジェクト (schelling-simulation)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs                     ← CLI (run / sweep / bnm / bnm-basin / tipping)
│       ├── config.rs                   ← 空間モデル設定
│       ├── grid.rs                     ← Cell 型 (グリッドは socsim-grid に委譲)
│       ├── metrics.rs                  ← 分離度指標
│       ├── world.rs                    ← socsim WorldState 実装 (socsim-grid GridIndex + 色マップ)
│       ├── mechanisms.rs               ← socsim Mechanism 実装 (移動規則 + scratch/request_stop)
│       ├── simulation.rs               ← 空間モデル動学 (socsim エンジン駆動)
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

- `cargo run` はワークスペースルートから `simulation` クレートを起動します（`-p schelling-simulation` は省略可，メンバーが 1 つのため）．
- `uv run` は uv workspace のメンバー `tools` が公開する `schelling-tools` コマンドを呼び出します．

## socsim フレームワーク上の空間モデル

空間モデル（`run` / `sweep`）のシミュレーションエンジンは社会シミュレーション基盤 [rs-social-simulation-tools](https://github.com/akitenkrad/rs-social-simulation-tools)（socsim）の上に構築されています（git 依存，commit は `Cargo.lock` で固定）．使用している主な API は次のとおりです．

- `WorldState` / `Mechanism` / `Scheduler` / `SimRng` — エンジン中核．
- `socsim-grid` の `Grid` / `GridIndex` — 格子・ムーア近傍・空きセル探索．旧実装の手書き近傍計算（moore_neighbors / vacant_cells / chebyshev など）は `socsim-grid` の `Grid`・`GridIndex` に置き換え，空間モジュールでは Schelling 固有の判定（満足・同色比率・移動先探索）のみをドメインヘルパとして実装する．
- `StepContext::request_stop` / `Simulation::stop_requested` — 収束による早期停止．
- `StepContext::scratch` — ステップ結果（移動数・開始時不満足数・収束フラグ）の受け渡し．ドライバは `Simulation::scratch` 経由で読み取る．
- `derive_seed` — 初期化用 RNG とエンジン RNG の分離．

移動メカニズム（`SchellingMoveMechanism`）は `Decision` フェーズで発火し，`StepContext::agent_order` が与えるアクティベーション順（= スケジューラがシャッフルした順序）に従って各エージェントを処理する．移動対象はステップ開始時に不満足だったエージェントのみで，途中で他者の移動により不満足化したエージェントは当該ステップでは動かさない．これにより `n_moved <= n_dissatisfied(ステップ開始時)` が保たれる．移動先選択に乱数は使わず（空きセルをチェビシェフ距離の昇順で探索する最近傍貪欲），ランダム性はアクティベーション順序のシャッフル（`RandomActivationScheduler`）のみに由来する．収束（開始時不満足が空）または行き詰まり（`n_moved == 0`）を検知したら `StepContext::request_stop` でエンジンに停止を要求する．

注: socsim エンジン化（および RNG ストリーム分離）により乱数の消費系列が旧バイナリと変わるため，ビット単位の軌跡再現は保証されません（同一シードでの再現性＝決定論は保証され，論文の定性的再現も保たれます）．

## 直交する解析モジュール

解析モデル（BNM / Tipping）は連続力学のため socsim には載せず，`simulation/src/analytic/` 配下の独立実装のままです．状態を集計人口 $(W, B)$ に縮約し，許容限界スケジュール（CDF）から反応曲線を導出して，位相平面で平衡点・安定性・ベクトル場・軌跡・吸引域を解析します．ティッピングモデルは BNM の上に投機・非対称な流入/退出・結果の類型分類を追加します．

## 参照論文

Schelling, T. C. (1971). Dynamic Models of Segregation.
*Journal of Mathematical Sociology*, 1(2), 143–186.
DOI: [10.1080/0022250X.1971.9989794](https://doi.org/10.1080/0022250X.1971.9989794)
