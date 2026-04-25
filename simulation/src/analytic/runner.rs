//! CLI から呼ばれる I/O オーケストレーション．
//!
//! `bnm` / `bnm-basin` サブコマンドの実装本体．許容スケジュール CSV，
//! 反応曲線 CSV，平衡点 CSV，ベクトル場 CSV，軌跡 CSV，吸引域 CSV を生成する．

use std::fs::{self, File};
use std::io::BufWriter;
use std::path::Path;

use chrono::Local;
use csv::Writer;
use serde::{Deserialize, Serialize};

use super::dynamics::{basin_of_attraction, integrate, BasinSample, DynamicsConfig};
use super::phase::{Equilibrium, EquilibriumKind, PhaseConfig, Stability, ViabilityRegion};
use super::tipping::{
    classify_tipping, FlowAsymmetry, Speculation, TippingClassification, TippingConfig,
    TippingType,
};

// ---------------------------------------------------------------------------
// config.json (bnm 用)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BnmConfigJson {
    pub command: &'static str,
    pub preset: Option<String>,
    pub phase: PhaseConfig,
    pub dynamics: DynamicsConfig,
    pub init: Option<(f64, f64)>,
    pub output_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BnmBasinConfigJson {
    pub command: &'static str,
    pub preset: Option<String>,
    pub phase: PhaseConfig,
    pub dynamics: DynamicsConfig,
    pub n_w: usize,
    pub n_b: usize,
    pub output_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TippingConfigJson {
    pub command: &'static str,
    pub preset: Option<String>,
    pub config: TippingConfig,
    pub init: (f64, f64),
    pub classification: TippingClassification,
    pub output_dir: String,
}

// ---------------------------------------------------------------------------
// CSV 行構造体
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct ScheduleRow {
    r: f64,
    f_r: f64,
}

#[derive(Serialize)]
struct ReactionRow {
    own: f64,
    max_other: f64,
}

#[derive(Serialize)]
struct EquilibriumRow {
    w: f64,
    b: f64,
    kind: String,
    stability: String,
}

#[derive(Serialize)]
struct VectorRow {
    w: f64,
    b: f64,
    dw_sign: f64,
    db_sign: f64,
    region: String,
}

#[derive(Serialize)]
struct TrajectoryRow {
    t: f64,
    w: f64,
    b: f64,
}

#[derive(Serialize)]
struct BasinRow {
    w0: f64,
    b0: f64,
    final_w: f64,
    final_b: f64,
    converged: bool,
    converged_kind: String,
    steps: usize,
}

// ---------------------------------------------------------------------------
// 共通ユーティリティ
// ---------------------------------------------------------------------------

fn equilibrium_kind_label(kind: EquilibriumKind) -> &'static str {
    match kind {
        EquilibriumKind::AllWhite => "all_white",
        EquilibriumKind::AllBlack => "all_black",
        EquilibriumKind::Mixed => "mixed",
        EquilibriumKind::Empty => "empty",
    }
}

fn stability_label(s: Stability) -> &'static str {
    match s {
        Stability::Stable => "stable",
        Stability::Unstable => "unstable",
        Stability::Saddle => "saddle",
    }
}

fn region_label(r: ViabilityRegion) -> &'static str {
    match r {
        ViabilityRegion::BothViable => "both_viable",
        ViabilityRegion::WViableOnly => "w_viable_only",
        ViabilityRegion::BViableOnly => "b_viable_only",
        ViabilityRegion::NeitherViable => "neither_viable",
    }
}

fn write_csv<T: Serialize>(path: &str, rows: &[T]) {
    let file = File::create(path).unwrap_or_else(|e| panic!("CSV作成失敗 {}: {}", path, e));
    let mut wtr = Writer::from_writer(BufWriter::new(file));
    for row in rows {
        wtr.serialize(row).expect("CSV書き込み失敗");
    }
    wtr.flush().expect("CSVフラッシュ失敗");
}

fn write_json<T: Serialize>(path: &str, value: &T) {
    let file = File::create(path).unwrap_or_else(|e| panic!("JSON作成失敗 {}: {}", path, e));
    serde_json::to_writer_pretty(BufWriter::new(file), value).expect("JSON書き込み失敗");
}

fn make_output_dir(base: &str, suffix: &str) -> String {
    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let dir = format!("{}/{}_{}", base, timestamp, suffix);
    fs::create_dir_all(&dir).expect("出力ディレクトリ作成失敗");

    // latest シンボリックリンクを更新
    let symlink_path = Path::new(base).join("latest");
    if symlink_path.is_symlink() {
        let _ = fs::remove_file(&symlink_path);
    }
    #[cfg(unix)]
    {
        let link_target = format!("{}_{}", timestamp, suffix);
        let _ = std::os::unix::fs::symlink(&link_target, &symlink_path);
    }
    dir
}

/// 共通: スケジュール・反応曲線・平衡点・ベクトル場 CSV を出力する．
fn dump_phase_artifacts(phase: &PhaseConfig, output_dir: &str) -> Vec<Equilibrium> {
    // tolerance_w.csv / tolerance_b.csv
    let n_samples = 200;
    let w_sched_rows: Vec<ScheduleRow> = phase
        .w_schedule
        .sample(n_samples)
        .into_iter()
        .map(|(r, f)| ScheduleRow { r, f_r: f })
        .collect();
    let b_sched_rows: Vec<ScheduleRow> = phase
        .b_schedule
        .sample(n_samples)
        .into_iter()
        .map(|(r, f)| ScheduleRow { r, f_r: f })
        .collect();
    write_csv(&format!("{}/tolerance_w.csv", output_dir), &w_sched_rows);
    write_csv(&format!("{}/tolerance_b.csv", output_dir), &b_sched_rows);

    // reaction_curve_w.csv / reaction_curve_b.csv
    let w_react_rows: Vec<ReactionRow> = phase
        .w_reaction()
        .sample(n_samples)
        .into_iter()
        .map(|(o, m)| ReactionRow { own: o, max_other: m })
        .collect();
    let b_react_rows: Vec<ReactionRow> = phase
        .b_reaction()
        .sample(n_samples)
        .into_iter()
        .map(|(o, m)| ReactionRow { own: o, max_other: m })
        .collect();
    write_csv(
        &format!("{}/reaction_curve_w.csv", output_dir),
        &w_react_rows,
    );
    write_csv(
        &format!("{}/reaction_curve_b.csv", output_dir),
        &b_react_rows,
    );

    // equilibria.csv
    let eqs = phase.equilibria();
    let eq_rows: Vec<EquilibriumRow> = eqs
        .iter()
        .map(|e| EquilibriumRow {
            w: e.w,
            b: e.b,
            kind: equilibrium_kind_label(e.kind).to_string(),
            stability: stability_label(e.stability).to_string(),
        })
        .collect();
    write_csv(&format!("{}/equilibria.csv", output_dir), &eq_rows);

    // vector_field.csv
    let field = phase.vector_field(20, 20);
    let field_rows: Vec<VectorRow> = field
        .into_iter()
        .map(|s| VectorRow {
            w: s.w,
            b: s.b,
            dw_sign: s.dw_sign,
            db_sign: s.db_sign,
            region: region_label(s.region).to_string(),
        })
        .collect();
    write_csv(&format!("{}/vector_field.csv", output_dir), &field_rows);

    eqs
}

// ---------------------------------------------------------------------------
// 公開エントリ: cmd_bnm
// ---------------------------------------------------------------------------

pub struct BnmRunArgs {
    pub preset_name: Option<String>,
    pub phase: PhaseConfig,
    pub dynamics: DynamicsConfig,
    pub init: (f64, f64),
    pub output_base: String,
}

pub fn cmd_bnm(args: BnmRunArgs) {
    let output_dir = make_output_dir(&args.output_base, "bnm");

    println!("=== Schelling 境界近隣モデル ===");
    println!("プリセット: {:?}", args.preset_name);
    println!(
        "W: {} | B: {} | capacity: {:?}",
        args.phase.w_schedule.label(),
        args.phase.b_schedule.label(),
        args.phase.capacity
    );
    println!("初期値: W₀={}, B₀={}", args.init.0, args.init.1);
    println!("出力先: {}", output_dir);
    println!("---------------------------------------");

    // 共通アーティファクト出力
    let eqs = dump_phase_artifacts(&args.phase, &output_dir);

    // 軌跡を計算
    let traj = integrate(&args.phase, &args.dynamics, args.init);
    let traj_rows: Vec<TrajectoryRow> = traj
        .history
        .iter()
        .map(|p| TrajectoryRow {
            t: p.t,
            w: p.w,
            b: p.b,
        })
        .collect();
    write_csv(&format!("{}/trajectory.csv", output_dir), &traj_rows);

    // config.json
    let config_json = BnmConfigJson {
        command: "bnm",
        preset: args.preset_name.clone(),
        phase: args.phase.clone(),
        dynamics: args.dynamics,
        init: Some(args.init),
        output_dir: output_dir.clone(),
    };
    write_json(&format!("{}/config.json", output_dir), &config_json);

    // サマリ表示
    println!(
        "平衡点: {} 個 (端点 + 混合 + 空)",
        eqs.iter()
            .filter(|e| e.kind != EquilibriumKind::Empty)
            .count()
            + 1
    );
    for e in &eqs {
        println!(
            "  - ({:.2}, {:.2}) [{}, {}]",
            e.w,
            e.b,
            equilibrium_kind_label(e.kind),
            stability_label(e.stability)
        );
    }
    let last = traj.history.last().unwrap();
    println!(
        "軌跡: {} ステップ | 収束: {} | 終点: ({:.2}, {:.2})",
        traj.history.len() - 1,
        if traj.converged { "Yes" } else { "No" },
        last.w,
        last.b
    );
    if let Some(eq) = traj.final_equilibrium {
        println!(
            "  → 収束先: {} (kind={})",
            equilibrium_kind_label(eq.kind),
            equilibrium_kind_label(eq.kind)
        );
    }
    println!("CSV → {}/{{tolerance,reaction_curve,equilibria,vector_field,trajectory}}.csv", output_dir);
    println!("設定 → {}/config.json", output_dir);
}

// ---------------------------------------------------------------------------
// 公開エントリ: cmd_bnm_basin
// ---------------------------------------------------------------------------

pub struct BnmBasinArgs {
    pub preset_name: Option<String>,
    pub phase: PhaseConfig,
    pub dynamics: DynamicsConfig,
    pub n_w: usize,
    pub n_b: usize,
    pub output_base: String,
}

// ---------------------------------------------------------------------------
// 公開エントリ: cmd_tipping
// ---------------------------------------------------------------------------

pub struct TippingRunArgs {
    pub preset_name: Option<String>,
    pub tipping: TippingConfig,
    pub init: (f64, f64),
    pub output_base: String,
}

fn tipping_type_label(t: TippingType) -> &'static str {
    match t {
        TippingType::InTippingOnly => "in_tipping_only",
        TippingType::OutTippingOnly => "out_tipping_only",
        TippingType::Both => "both",
        TippingType::Neither => "neither",
    }
}

pub fn cmd_tipping(args: TippingRunArgs) {
    let output_dir = make_output_dir(&args.output_base, "tipping");

    println!("=== Schelling ティッピングモデル ===");
    println!("プリセット: {:?}", args.preset_name);
    println!("初期値: W₀={}, B₀={}", args.init.0, args.init.1);
    println!("出力先: {}", output_dir);
    println!("---------------------------------------");

    // 共通アーティファクト出力
    let _eqs = dump_phase_artifacts(&args.tipping.phase, &output_dir);

    // ティッピング類型分類
    let classification = classify_tipping(&args.tipping.phase);
    println!(
        "ティッピング類型: {} (全W安定={}, 安定混合={})",
        tipping_type_label(classification.tipping_type),
        classification.all_white_stable,
        classification.mixed_stable_exists,
    );

    // 軌跡を計算
    let traj = args.tipping.integrate(args.init);
    let traj_rows: Vec<TrajectoryRow> = traj
        .history
        .iter()
        .map(|p| TrajectoryRow {
            t: p.t,
            w: p.w,
            b: p.b,
        })
        .collect();
    write_csv(&format!("{}/trajectory.csv", output_dir), &traj_rows);

    // 分類サマリ
    write_json(
        &format!("{}/tipping_classification.json", output_dir),
        &serde_json::json!({
            "type": tipping_type_label(classification.tipping_type),
            "all_white_stable": classification.all_white_stable,
            "mixed_stable_exists": classification.mixed_stable_exists,
        }),
    );

    // config.json
    let config_json = TippingConfigJson {
        command: "tipping",
        preset: args.preset_name.clone(),
        config: args.tipping,
        init: args.init,
        classification,
        output_dir: output_dir.clone(),
    };
    write_json(&format!("{}/config.json", output_dir), &config_json);

    let last = traj.history.last().unwrap();
    println!(
        "軌跡: {} ステップ | 収束: {} | 終点: ({:.2}, {:.2})",
        traj.history.len() - 1,
        if traj.converged { "Yes" } else { "No" },
        last.w,
        last.b
    );
    println!(
        "  → 収束先: {:?}",
        traj.final_equilibrium.map(|e| equilibrium_kind_label(e.kind))
    );
    println!("CSV → {}/{{...,trajectory}}.csv", output_dir);
    println!("分類 → {}/tipping_classification.json", output_dir);
    println!("設定 → {}/config.json", output_dir);
}

// 抑止のための公開
#[allow(dead_code)]
pub fn make_speculation_none() -> Speculation {
    Speculation::None
}
#[allow(dead_code)]
pub fn make_default_asymmetry() -> FlowAsymmetry {
    FlowAsymmetry {
        w_inflow: 1.0,
        w_outflow: 1.0,
        b_inflow: 1.0,
        b_outflow: 1.0,
    }
}

pub fn cmd_bnm_basin(args: BnmBasinArgs) {
    let output_dir = make_output_dir(&args.output_base, "bnm_basin");

    println!("=== Schelling 境界近隣モデル — 吸引域解析 ===");
    println!("プリセット: {:?}", args.preset_name);
    println!(
        "初期条件グリッド: {}×{} ({} 点)",
        args.n_w + 1,
        args.n_b + 1,
        (args.n_w + 1) * (args.n_b + 1)
    );
    println!("出力先: {}", output_dir);
    println!("---------------------------------------");

    let _eqs = dump_phase_artifacts(&args.phase, &output_dir);

    let basin: Vec<BasinSample> =
        basin_of_attraction(&args.phase, &args.dynamics, args.n_w, args.n_b);
    let basin_rows: Vec<BasinRow> = basin
        .iter()
        .map(|s| BasinRow {
            w0: s.w0,
            b0: s.b0,
            final_w: s.final_w,
            final_b: s.final_b,
            converged: s.converged,
            converged_kind: s
                .converged_kind
                .map(|k| equilibrium_kind_label(k).to_string())
                .unwrap_or_else(|| "none".to_string()),
            steps: s.steps,
        })
        .collect();
    write_csv(&format!("{}/basin.csv", output_dir), &basin_rows);

    let config_json = BnmBasinConfigJson {
        command: "bnm-basin",
        preset: args.preset_name,
        phase: args.phase,
        dynamics: args.dynamics,
        n_w: args.n_w,
        n_b: args.n_b,
        output_dir: output_dir.clone(),
    };
    write_json(&format!("{}/config.json", output_dir), &config_json);

    // 集計: 各収束先のサンプル数
    let mut counts = std::collections::HashMap::<String, usize>::new();
    for r in &basin_rows {
        *counts.entry(r.converged_kind.clone()).or_insert(0) += 1;
    }
    println!("吸引域サマリ:");
    for (kind, n) in &counts {
        println!("  {} → {} 点", kind, n);
    }
    println!("CSV → {}/basin.csv", output_dir);
    println!("設定 → {}/config.json", output_dir);
}
