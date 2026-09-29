//! I/O orchestration invoked from the CLI.
//!
//! Core implementation of the `bnm` / `bnm-basin` subcommands. Generates tolerance-schedule CSV,
//! reaction-curve CSV, equilibrium CSV, vector-field CSV, trajectory CSV, and basin-of-attraction CSV files.

use std::fs;
use std::fs::File;
use std::io::BufWriter;

use csv::Writer;
use runvault::{Run, RunOptions};
use serde::{Deserialize, Serialize};

use super::dynamics::{
    basin_of_attraction_observed, integrate_observed, BasinSample, DynamicsConfig,
};
use super::phase::{Equilibrium, EquilibriumKind, PhaseConfig, Stability, ViabilityRegion};
use super::tipping::{classify_tipping, FlowAsymmetry, Speculation, TippingConfig, TippingType};

// ---------------------------------------------------------------------------
// config.json (for bnm)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BnmConfigJson {
    pub command: &'static str,
    pub preset: Option<String>,
    pub phase: PhaseConfig,
    pub dynamics: DynamicsConfig,
    pub init: Option<(f64, f64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BnmBasinConfigJson {
    pub command: &'static str,
    pub preset: Option<String>,
    pub phase: PhaseConfig,
    pub dynamics: DynamicsConfig,
    pub n_w: usize,
    pub n_b: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TippingConfigJson {
    pub command: &'static str,
    pub preset: Option<String>,
    pub config: TippingConfig,
    pub init: (f64, f64),
}

// ---------------------------------------------------------------------------
// CSV row structs
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
    a: f64,
    b: f64,
    kind: String,
    stability: String,
}

#[derive(Serialize)]
struct VectorRow {
    a: f64,
    b: f64,
    da_sign: f64,
    db_sign: f64,
    region: String,
}

#[derive(Serialize)]
struct TrajectoryRow {
    t: f64,
    a: f64,
    b: f64,
}

#[derive(Serialize)]
struct BasinRow {
    a0: f64,
    b0: f64,
    final_a: f64,
    final_b: f64,
    converged: bool,
    converged_kind: String,
    steps: usize,
}

// ---------------------------------------------------------------------------
// Shared utilities
// ---------------------------------------------------------------------------

fn equilibrium_kind_label(kind: EquilibriumKind) -> &'static str {
    match kind {
        EquilibriumKind::AllWhite => "all_a",
        EquilibriumKind::AllBlack => "all_b",
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
    let file =
        File::create(path).unwrap_or_else(|e| panic!("failed to create CSV {}: {}", path, e));
    let mut wtr = Writer::from_writer(BufWriter::new(file));
    for row in rows {
        wtr.serialize(row).expect("failed to write CSV");
    }
    wtr.flush().expect("failed to flush CSV");
}

fn write_json<T: Serialize>(path: &str, value: &T) {
    let file =
        File::create(path).unwrap_or_else(|e| panic!("failed to create JSON {}: {}", path, e));
    serde_json::to_writer_pretty(BufWriter::new(file), value).expect("failed to write JSON");
}

/// Starts a run for an analysis subcommand and returns `(run, artifacts directory)`.
///
/// These are deterministic analytical computations that do not use an RNG, so set `domain = "analysis"`.
/// Using `simulation` would require master_seed and would record a nonexistent seed.
fn start_run<T: Serialize + ?Sized>(
    subcommand: &'static str,
    output_base: &str,
    parameters: &T,
    scratch: bool,
) -> (Run, String) {
    let run = Run::start(
        RunOptions::new("schelling-analytic", subcommand)
            .scratch(scratch)
            .repo_id("schelling1971")
            .domain("analysis")
            .results_root(output_base)
            .parameters(parameters)
            .expect("runvault: failed to build parameters")
            .replication(crate::record::replication()),
    )
    .expect("runvault: failed to start run");

    let artifacts = run.dir().join("artifacts");
    fs::create_dir_all(&artifacts).expect("failed to create artifacts directory");
    let dir = artifacts.to_string_lossy().into_owned();
    (run, dir)
}

/// Shared: outputs schedule, reaction-curve, equilibrium, and vector-field CSV files.
fn dump_phase_artifacts(phase: &PhaseConfig, output_dir: &str) -> Vec<Equilibrium> {
    // tolerance_a.csv / tolerance_b.csv
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
    write_csv(&format!("{}/tolerance_a.csv", output_dir), &w_sched_rows);
    write_csv(&format!("{}/tolerance_b.csv", output_dir), &b_sched_rows);

    // reaction_curve_a.csv / reaction_curve_b.csv
    let w_react_rows: Vec<ReactionRow> = phase
        .w_reaction()
        .sample(n_samples)
        .into_iter()
        .map(|(o, m)| ReactionRow {
            own: o,
            max_other: m,
        })
        .collect();
    let b_react_rows: Vec<ReactionRow> = phase
        .b_reaction()
        .sample(n_samples)
        .into_iter()
        .map(|(o, m)| ReactionRow {
            own: o,
            max_other: m,
        })
        .collect();
    write_csv(
        &format!("{}/reaction_curve_a.csv", output_dir),
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
            a: e.w,
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
            a: s.w,
            b: s.b,
            da_sign: s.dw_sign,
            db_sign: s.db_sign,
            region: region_label(s.region).to_string(),
        })
        .collect();
    write_csv(&format!("{}/vector_field.csv", output_dir), &field_rows);

    eqs
}

// ---------------------------------------------------------------------------
// Public entry point: cmd_bnm
// ---------------------------------------------------------------------------

pub struct BnmRunArgs {
    pub preset_name: Option<String>,
    pub phase: PhaseConfig,
    pub dynamics: DynamicsConfig,
    pub init: (f64, f64),
    pub output_base: String,
}

pub fn cmd_bnm(args: BnmRunArgs, scratch: bool) {
    let parameters = BnmConfigJson {
        command: "bnm",
        preset: args.preset_name.clone(),
        phase: args.phase.clone(),
        dynamics: args.dynamics,
        init: Some(args.init),
    };
    let (run, output_dir) = start_run("bnm", &args.output_base, &parameters, scratch);

    println!("=== Schelling Bounded-Neighborhood Model ===");
    println!("Preset: {:?}", args.preset_name);
    println!(
        "W: {} | B: {} | capacity: {:?}",
        args.phase.w_schedule.label(),
        args.phase.b_schedule.label(),
        args.phase.capacity
    );
    println!("Initial values: W₀={}, B₀={}", args.init.0, args.init.1);
    println!("Output: {}", run.dir().display());
    println!("---------------------------------------");

    // Output shared artifacts.
    let eqs = dump_phase_artifacts(&args.phase, &output_dir);

    // Compute the trajectory. The unit is one integration step—the only work that grows with `--max-steps`.
    // The preceding CSV outputs (200 tolerance-schedule points, 200 reaction-curve points, and a 20x20
    // vector field) are all fixed-size and cannot be swept. Because convergence stops the computation
    // before `--max-steps`, count with an unbounded stage instead of using the limit as the denominator.
    let mut stage = run.unbounded_stage("steps");
    let traj = integrate_observed(&args.phase, &args.dynamics, args.init, |_| stage.tick());
    stage.close();
    let traj_rows: Vec<TrajectoryRow> = traj
        .history
        .iter()
        .map(|p| TrajectoryRow {
            t: p.t,
            a: p.w,
            b: p.b,
        })
        .collect();
    write_csv(&format!("{}/trajectory.csv", output_dir), &traj_rows);

    // Display summary.
    println!(
        "Equilibria: {} (endpoints + mixed + empty)",
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
        "Trajectory: {} steps | Converged: {} | Endpoint: ({:.2}, {:.2})",
        traj.history.len() - 1,
        if traj.converged { "Yes" } else { "No" },
        last.w,
        last.b
    );
    if let Some(eq) = traj.final_equilibrium {
        println!(
            "  → Attractor: {} (kind={})",
            equilibrium_kind_label(eq.kind),
            equilibrium_kind_label(eq.kind)
        );
    }
    println!(
        "CSV → {}/{{tolerance,reaction_curve,equilibria,vector_field,trajectory}}.csv",
        output_dir
    );
    let dir = run.finish().expect("runvault: failed to finish run");
    println!("Config → {}/config.json", dir.display());
}

// ---------------------------------------------------------------------------
// Public entry point: cmd_bnm_basin
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
// Public entry point: cmd_tipping
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

pub fn cmd_tipping(args: TippingRunArgs, scratch: bool) {
    let parameters = TippingConfigJson {
        command: "tipping",
        preset: args.preset_name.clone(),
        config: args.tipping.clone(),
        init: args.init,
    };
    let (run, output_dir) = start_run("tipping", &args.output_base, &parameters, scratch);

    println!("=== Schelling Tipping Model ===");
    println!("Preset: {:?}", args.preset_name);
    println!("Initial values: W₀={}, B₀={}", args.init.0, args.init.1);
    println!("Output: {}", run.dir().display());
    println!("---------------------------------------");

    // Output shared artifacts.
    let _eqs = dump_phase_artifacts(&args.tipping.phase, &output_dir);

    // Classify the tipping type.
    let classification = classify_tipping(&args.tipping.phase);
    println!(
        "Tipping class: {} (stable all-A={}, stable mixed={})",
        tipping_type_label(classification.tipping_type),
        classification.all_a_stable,
        classification.mixed_stable_exists,
    );

    // Compute the trajectory. The unit is one integration step, as in BNM (and is unbounded for the same reason).
    let mut stage = run.unbounded_stage("steps");
    let traj = args.tipping.integrate_observed(args.init, |_| stage.tick());
    stage.close();
    let traj_rows: Vec<TrajectoryRow> = traj
        .history
        .iter()
        .map(|p| TrajectoryRow {
            t: p.t,
            a: p.w,
            b: p.b,
        })
        .collect();
    write_csv(&format!("{}/trajectory.csv", output_dir), &traj_rows);

    // Classification summary.
    write_json(
        &format!("{}/tipping_classification.json", output_dir),
        &serde_json::json!({
            "type": tipping_type_label(classification.tipping_type),
            "all_a_stable": classification.all_a_stable,
            "mixed_stable_exists": classification.mixed_stable_exists,
        }),
    );

    let last = traj.history.last().unwrap();
    println!(
        "Trajectory: {} steps | Converged: {} | Endpoint: ({:.2}, {:.2})",
        traj.history.len() - 1,
        if traj.converged { "Yes" } else { "No" },
        last.w,
        last.b
    );
    println!(
        "  → Attractor: {:?}",
        traj.final_equilibrium
            .map(|e| equilibrium_kind_label(e.kind))
    );
    println!("CSV → {}/{{...,trajectory}}.csv", output_dir);
    println!(
        "Classification → {}/tipping_classification.json",
        output_dir
    );
    let dir = run.finish().expect("runvault: failed to finish run");
    println!("Config → {}/config.json", dir.display());
}

// Public to suppress warnings.
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

pub fn cmd_bnm_basin(args: BnmBasinArgs, scratch: bool) {
    let parameters = BnmBasinConfigJson {
        command: "bnm-basin",
        preset: args.preset_name.clone(),
        phase: args.phase.clone(),
        dynamics: args.dynamics,
        n_w: args.n_w,
        n_b: args.n_b,
    };
    let (run, output_dir) = start_run("bnm-basin", &args.output_base, &parameters, scratch);

    println!("=== Schelling Bounded-Neighborhood Model — Basin Analysis ===");
    println!("Preset: {:?}", args.preset_name);
    println!(
        "Initial-condition grid: {}×{} ({} points)",
        args.n_w + 1,
        args.n_b + 1,
        (args.n_w + 1) * (args.n_b + 1)
    );
    println!("Output: {}", run.dir().display());
    println!("---------------------------------------");

    let _eqs = dump_phase_artifacts(&args.phase, &output_dir);

    // The unit is one initial-condition point. Integration for one point takes less than one millisecond
    // with the default `--max-steps 3000`; the number of points drives the runtime (measured: 0.5 seconds
    // for 20x20, 7.9 seconds for 100x100, and 23.6 seconds for 200x200). The grid is predetermined and
    // has no early-termination condition, so use a bounded stage with the total number of grid points as the denominator.
    let total_points = (args.n_w + 1) * (args.n_b + 1);
    let mut stage = run.stage("initial conditions", total_points);
    let basin: Vec<BasinSample> =
        basin_of_attraction_observed(&args.phase, &args.dynamics, args.n_w, args.n_b, || {
            stage.tick()
        });
    stage.close();
    let basin_rows: Vec<BasinRow> = basin
        .iter()
        .map(|s| BasinRow {
            a0: s.w0,
            b0: s.b0,
            final_a: s.final_w,
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

    // Aggregate the sample count for each convergence destination.
    let mut counts = std::collections::HashMap::<String, usize>::new();
    for r in &basin_rows {
        *counts.entry(r.converged_kind.clone()).or_insert(0) += 1;
    }
    println!("Basin summary:");
    for (kind, n) in &counts {
        println!("  {} → {} points", kind, n);
    }
    println!("CSV → {}/basin.csv", output_dir);
    let dir = run.finish().expect("runvault: failed to finish run");
    println!("Config → {}/config.json", dir.display());
}
