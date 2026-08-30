//! runvault への記録の共通部分．
//!
//! 論文メタデータ (research) は `run` / `sweep` / `bnm` / `bnm-basin` / `tipping`
//! のどのサブコマンドでも同一なので，ここ 1 箇所で組み立てる．

use runvault::{Replication, Run, Target, Work};

use crate::metrics::Metrics;
use crate::simulation::SimulationResult;

/// この再現実験が対象としている論文．
///
/// `bnm` / `bnm-basin` / `tipping` がどの図を再現するかは `--preset` で決まり，
/// サブコマンド名からは決まらないため，`Target::figure` はここでは付けない
/// (claim だけを共通の対象として持つ)．
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

/// シミュレーション 1 本ぶんの記録．
///
/// ステップごとの 7 指標 (`step` は時間軸なので値としては書かない) と，
/// run 全体を 1 つの値で表す `converged` / `final_iteration` を書く．
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
    .expect("run スコープの指標の記録に失敗");
}

/// `Metrics` の 7 フィールドを 1 ステップぶんまとめて書く．
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
    .unwrap_or_else(|e| panic!("step {} の指標の記録に失敗: {e}", m.step));
}
