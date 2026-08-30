[English](reproduction.md) | **日本語**

# 論文再現実験（Fig. 7–32）

`schelling-tools reproduce` は論文で報告された主要実験を一括で実行し，各図の報告値との比較レポートを生成します．

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

## 再現対象

| キー | 図 | 設定 | 論文値 |
|---|---|---|---|
| `fig11_tau_one_third` | Fig. 11 | τ=1/3, 等数 | avg_same 65–75% |
| `fig09_tau_one_half_lenient` | Fig. 9 | τ=1/2, 等数（緩運用） | avg_same 80–83% |
| `fig08_tau_one_half_strict` | Fig. 8 | τ=1/2, 等数, 厳格運用（`--move-mode strict`） | avg_same 89–91% |
| `fig12_unequal_two_to_one` | Fig. 12 | τ=1/3, 不等数 97:49, `best-local` 戦略 | 少数派 >80% |
| `fig16_congregationist_min_same_3` | Fig. 16 | `min-same:3` | avg_same ≈75%, 異色近隣なし ≈38% |
| `fig17_integrationist_bounded_3_6` | Fig. 17 | `bounded:3:6` | 定性報告: dead space形成・収束困難 |
| `fig14_tau_sweep` | Fig. 14 | τ=0.10–0.60 (0.05刻み) | 0.35–0.50で急峻な上昇 |

## 解析モデル (BNM + Tipping)

| キー | 図 | 設定 | 期待される結果 |
|---|---|---|---|
| `fig18_linear_two_to_one` | Fig. 18 | 直線型・1:2 比 | 端点2安定 + 不安定混合 |
| `fig19_steep_three_stable` | Fig. 19 | 急勾配 (中央値=1.5) | 3 安定均衡，混合へ収束 |
| `fig20_lenient_linear` | Fig. 20 | 緩勾配の直線型 (R_max=3, 対称) | 反応曲線の頂点が上がる |
| `fig21_steep_linear` | Fig. 21 | 急勾配の直線型 (R_max=1, 対称) | 反応曲線の頂点が下がる |
| `fig22_unequal_no_intersection` | Fig. 22 | 不等数で曲線非交差 | 混合均衡なし |
| `fig23_limiting_numbers` | Fig. 23 | 入域上限クオータ | クオータが混合を生む |
| `fig24_asymmetric_tolerance` | Fig. 24 | 非対称許容 (W R_max=2, B R_max=1) | 偏在する混合均衡 |
| `fig25_zero_tolerance_intercept` | Fig. 25 | ゼロ許容者切片付きアフィン | 端点流出が強まる |
| `fig26_capacity_constraint` | Fig. 26 | 容量制約 C=120 | 混合均衡が容量線上に乗る |
| `fig27_piecewise_schedule` | Fig. 27 | 区分線形 (S字CDF) | 非一様な許容分布 |
| `fig28_unequal_tolerant_minority` | Fig. 28 | 不等数 + 少数派が寛容 | 混合均衡が生き残る |
| `fig29_strong_quota` | Fig. 29 | 強いクオータ (B pop_max=20) | 混合均衡が低 B 域に固定される |
| `fig30a_in_tipping_only` | Fig. 30a | B 極寛容 | in-tipping のみ |
| `fig30b_out_tipping_only` | Fig. 30b | Fig.18 と同構造 | out-tipping のみ |
| `fig31_both_tipping` | Fig. 31 | W 不寛容 + B 寛容 | 両ティッピング，all_black へ |
| `fig32_neither_tipping` | Fig. 32 | Fig.19 と同構造 | ティッピングなし，混合へ |

## 注記

- **Fig. 8（厳格運用）** は `--move-mode strict` で実行し，満足エージェントもより同質な近隣へ投機的に移動します．緩運用の Fig. 9 よりはるかに鋭い分離が起こり，τ=1/2 ではほぼ完全分離へ向かう傾向があり，論文値 89–91% を上回ります．
- **Fig. 12（不等数）** は `--move-strategy best-local` で実行し，少数派が最も同質な区画へ寄り集まります．これにより少数派クラスタ比率が論文値 > 80% に近づきます（`nearest` 戦略のおよそ 55–67% に対して）．
- **Fig. 17 は論文が定量値を示していない**ため，数値比較ではなく収束ステップ数の挙動（統合選好は収束困難）で論文挙動を確認します．

## 出力ファイル

```
results/paper_reproduction/{timestamp}/
├── reproduction_summary.json       ← 構造化データ（per-seed メトリクスと集計）
├── reproduction_summary.csv        ← 表形式の per-seed 結果
├── reproduction_report.txt         ← コンソール出力と同じ比較レポート
├── fig11_tau_one_third/
│   └── seed_{N}/schelling/run_{timestamp}_{cfg8}_{exec4}/metrics.csv
├── fig16_congregationist_min_same_3/
│   └── ...
├── fig14_tau_sweep/
│   └── schelling/
│       ├── sweep_{timestamp}_{cfg8}_{exec4}/   ← 親（グリッド定義）
│       └── run_{timestamp}_{cfg8}_{exec4}/     ← 条件ごとの子
└── fig18_bnm_linear/
    └── schelling-analytic/bnm_{timestamp}_{cfg8}_{exec4}/artifacts/
```

各実験の出力は runvault の run ディレクトリになり，`reproduce` はその場所を `runvault path --latest --subcommand ...` で解決します．τ 感度解析（Fig. 14）のサマリ表はファイルとしては存在せず，親 run に紐づく子 run から組み直されます．

結果のメトリクスや図の読み方は [可視化 — 出力の解釈](visualization.ja.md#出力の解釈) を参照してください．
