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
| `fig08_tau_one_half_strict` | Fig. 8 | τ=1/2, equal numbers, strict operation (`--move-mode strict`) | avg_same 89–91% |
| `fig12_unequal_two_to_one` | Fig. 12 | τ=1/3, unequal numbers 97:49, `best-local` strategy | minority >80% |
| `fig16_congregationist_min_same_3` | Fig. 16 | `min-same:3` | avg_same ≈75%, no opposite-color neighbors ≈38% |
| `fig17_integrationist_bounded_3_6` | Fig. 17 | `bounded:3:6` | qualitative report: dead-space formation, hard to converge |
| `fig14_tau_sweep` | Fig. 14 | τ=0.10–0.60 (step 0.05) | sharp rise around 0.35–0.50 |

## Analytic models (BNM + Tipping)

| Key | Figure | Settings | Expected result |
|-----|--------|----------|-----------------|
| `fig18_linear_two_to_one` | Fig. 18 | Linear, 1:2 ratio | Two stable endpoints + unstable mix |
| `fig19_steep_three_stable` | Fig. 19 | Steep slope (median = 1.5) | 3 stable equilibria, converges to mix |
| `fig20_lenient_linear` | Fig. 20 | Lenient linear (R_max = 3, symmetric) | Reaction-curve peak rises |
| `fig21_steep_linear` | Fig. 21 | Steep linear (R_max = 1, symmetric) | Reaction-curve peak falls |
| `fig22_unequal_no_intersection` | Fig. 22 | Unequal numbers, curves do not intersect | No mixed equilibrium |
| `fig23_limiting_numbers` | Fig. 23 | Entry-cap quota | Quota produces a mix |
| `fig24_asymmetric_tolerance` | Fig. 24 | Asymmetric tolerance (W R_max = 2, B R_max = 1) | Off-center mixed equilibrium |
| `fig25_zero_tolerance_intercept` | Fig. 25 | Affine with zero-tolerance intercept | Stronger endpoint outflow |
| `fig26_capacity_constraint` | Fig. 26 | Capacity constraint C = 120 | Mixed equilibrium on the capacity line |
| `fig27_piecewise_schedule` | Fig. 27 | Piecewise-linear (S-shaped CDF) | Non-uniform tolerance distribution |
| `fig28_unequal_tolerant_minority` | Fig. 28 | Unequal numbers + tolerant minority | Mixed equilibrium survives |
| `fig29_strong_quota` | Fig. 29 | Strong quota (B pop_max = 20) | Mixed equilibrium pinned to low B |
| `fig30a_in_tipping_only` | Fig. 30a | B extremely tolerant | In-tipping only |
| `fig30b_out_tipping_only` | Fig. 30b | Same structure as Fig. 18 | Out-tipping only |
| `fig31_both_tipping` | Fig. 31 | W intolerant + B tolerant | Both tipping, toward all_black |
| `fig32_neither_tipping` | Fig. 32 | Same structure as Fig. 19 | No tipping, toward a mix |

## Notes

- **Fig. 8 (strict operation)** is run with `--move-mode strict`, where satisfied agents also make speculative moves toward more homogeneous neighborhoods. This sharpens separation far beyond the lenient Fig. 9 case; under τ=1/2 it tends toward near-complete separation, which sits above the paper's reported 89–91%.
- **Fig. 12 (unequal numbers)** is run with `--move-strategy best-local`, which lets the minority coalesce into the most homogeneous available block. This raises the minority-cluster ratio close to the paper's > 80% (versus roughly 55–67% under the `nearest` strategy).
- **Fig. 17 does not provide quantitative values in the paper**, so instead of a numerical comparison we confirm the paper's behavior via the convergence-step behavior (integrationist preferences are hard to converge).

## Output files

```
results/paper_reproduction/{timestamp}/
├── reproduction_summary.json       # structured data (per-seed metrics and aggregates)
├── reproduction_summary.csv        # tabular per-seed results
├── reproduction_report.txt         # the same comparison report as the console output
├── fig11_tau_one_third/
│   └── seed_{N}/schelling/run_{timestamp}_{cfg8}_{exec4}/metrics.csv
├── fig16_congregationist_min_same_3/
│   └── ...
├── fig14_tau_sweep/
│   └── schelling/
│       ├── sweep_{timestamp}_{cfg8}_{exec4}/   # parent (grid definition)
│       └── run_{timestamp}_{cfg8}_{exec4}/     # one child per condition
└── fig18_bnm_linear/
    └── schelling-analytic/bnm_{timestamp}_{cfg8}_{exec4}/artifacts/
```

Every experiment writes a runvault run directory, and `reproduce` locates them with `runvault path --latest --subcommand ...`. The τ-sensitivity summary table (Fig. 14) does not exist as a file; it is rebuilt from the child runs attached to the parent.

For how to read the resulting metrics and figures, see [Visualization — output interpretation](visualization.md#output-interpretation).
