**English** | [日本語](reproduction.ja.md)

# Paper Reproduction (Fig. 7–32)

`schelling-tools reproduce` runs the main experiments reported in the paper in one shot and produces a report comparing the results against each figure's reported values.

```bash
# Reproduce all experiments with 5 seeds (recommended)
uv run schelling-tools reproduce

# Specify seeds
uv run schelling-tools reproduce --seeds 42,123,456,789,2024

# Run only specific experiments (comma-separated)
uv run schelling-tools reproduce --only fig11_tau_one_third,fig16_congregationist_min_same_3

# Skip the τ sensitivity analysis to run faster
uv run schelling-tools reproduce --skip-sweep

# Skip the analytic models (Fig. 18–32)
uv run schelling-tools reproduce --skip-analytic

# Run only the analytic models (skip the spatial model + τ sensitivity analysis)
uv run schelling-tools reproduce --analytic-only

# Skip cargo build (use the already-built binary)
uv run schelling-tools reproduce --skip-build
```

## Reproduction targets

| Key | Figure | Settings | Paper value |
|-----|--------|----------|-------------|
| `fig11_tau_one_third` | Fig. 11 | τ=1/3, equal numbers | avg_same 65–75% |
| `fig09_tau_one_half_lenient` | Fig. 9 | τ=1/2, equal numbers (lenient) | avg_same 80–83% |
| `fig08_tau_one_half_strict` | Fig. 8 | τ=1/2, equal numbers (approximation of strict) | avg_same 89–91% |
| `fig12_unequal_two_to_one` | Fig. 12 | τ=1/3, unequal numbers 97:49 | minority >80% |
| `fig16_congregationist_min_same_3` | Fig. 16 | `min-same:3` | avg_same ≈75%, no opposite-color neighbors ≈38% |
| `fig17_integrationist_bounded_3_6` | Fig. 17 | `bounded:3:6` | qualitative report: dead-space formation, hard to converge |
| `fig14_tau_sweep` | Fig. 14 | τ=0.10–0.60 (step 0.05) | sharp rise around 0.35–0.50 |

## Analytic models (BNM + Tipping)

| Key | Figure | Settings | Expected result |
|-----|--------|----------|-----------------|
| `fig18_linear_two_to_one` | Fig. 18 | Linear, 1:2 ratio | Two stable endpoints + unstable mix |
| `fig19_steep_three_stable` | Fig. 19 | Steep slope (median = 1.5) | 3 stable equilibria, converges to mix |
| `fig22_unequal_no_intersection` | Fig. 22 | Unequal numbers, curves do not intersect | No mixed equilibrium |
| `fig23_limiting_numbers` | Fig. 23 | Entry-cap quota | Quota produces a mix |
| `fig30a_in_tipping_only` | Fig. 30a | B extremely tolerant | In-tipping only |
| `fig30b_out_tipping_only` | Fig. 30b | Same structure as Fig. 18 | Out-tipping only |
| `fig31_both_tipping` | Fig. 31 | W intolerant + B tolerant | Both tipping, toward all_black |
| `fig32_neither_tipping` | Fig. 32 | Same structure as Fig. 19 | No tipping, toward a mix |

## Notes

- **The "strict" variant of Fig. 8 is not reproduced.** In the current implementation, satisfied agents do not move. The strict version in the paper involves speculative moves.
- **Fig. 17 does not provide quantitative values in the paper**, so instead of a numerical comparison we confirm the paper's behavior via the convergence-step behavior (integrationist preferences are hard to converge).

## Output files

```
results/paper_reproduction/{timestamp}/
├── reproduction_summary.json       # structured data (per-seed metrics and aggregates)
├── reproduction_summary.csv        # tabular per-seed results
├── reproduction_report.txt         # the same comparison report as the console output
├── fig11_tau_one_third/
│   └── seed_{N}/{timestamp}/metrics.csv
├── fig16_congregationist_min_same_3/
│   └── ...
└── fig14_tau_sweep/
    └── {timestamp}_sweep/sweep_summary.csv
```

For how to read the resulting metrics and figures, see [Visualization — output interpretation](visualization.md#output-interpretation).
