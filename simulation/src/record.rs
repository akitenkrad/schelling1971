//! Shared runvault recording logic.
//!
//! Paper metadata (research) is identical for the `run`, `sweep`, `bnm`, `bnm-basin`, and
//! `tipping` subcommands, so it is assembled in one place here.

use runvault::{Replication, Run, Target, Work};

use crate::metrics::Metrics;
use crate::simulation::SimulationResult;

/// The paper targeted by this replication study.
///
/// Which figure `bnm` / `bnm-basin` / `tipping` reproduces is determined by `--preset`, not by
/// the subcommand name, so `Target::figure` is not set here (only the claim is shared as a target).
pub fn replication() -> Replication {
    Work::doi("10.1080/0022250X.1971.9989794")
        .title("Dynamic Models of Segregation")
        .year(1971)
        .source_version("published")
        .target(Target::claim(
            "segregation-from-mild-preference",
            "Mild individual preferences produce marked collective segregation",
        ))
        .obsidian_note("研究/98_論文レポート/80-再現実験/実装完了/schelling1971/設計書.md")
}

/// Records one simulation run.
///
/// Writes seven per-step metrics (`step` is the time axis and is therefore not written as a value),
/// plus `converged` / `final_iteration`, each representing the entire run as one value.
pub fn log_simulation(run: &mut Run, result: &SimulationResult) {
    for m in &result.metrics_history {
        log_step(run, m);
    }
    run.log_metrics(
        "run",
        &[
            ("converged", if result.converged { 1.0 } else { 0.0 }),
            ("final_iteration", result.final_iteration as f64),
        ],
    )
    .expect("failed to record run-scoped metrics");
}

/// Writes all seven `Metrics` fields for one step.
fn log_step(run: &mut Run, m: &Metrics) {
    run.log_metrics_at(
        m.step as u64,
        "step",
        "run",
        &[
            ("avg_same_ratio", m.avg_same_ratio),
            ("pct_no_opposite", m.pct_no_opposite),
            ("dissimilarity_index", m.dissimilarity_index),
            ("n_dissatisfied", m.n_dissatisfied as f64),
            ("n_moved", m.n_moved as f64),
            ("avg_same_ratio_a", m.avg_same_ratio_a),
            ("avg_same_ratio_b", m.avg_same_ratio_b),
        ],
    )
    .unwrap_or_else(|e| panic!("failed to record metrics for step {}: {e}", m.step));
}
