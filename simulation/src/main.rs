mod analytic;
mod config;
mod grid;
mod mechanisms;
mod metrics;
mod record;
mod simulation;
mod world;

use std::cell::RefCell;
use std::rc::Rc;

use clap::{Parser, Subcommand};
use config::{Config, MoveMode, MoveStrategy, SatisfactionRule};
use mechanisms::DecisionObserver;
use runvault::{Lineage, Run, RunOptions, Stage};
use simulation::{run as run_simulation, run_observed as run_simulation_observed};

use analytic::dynamics::{DynamicsConfig, FlowModel};
use analytic::phase::PhaseConfig;
use analytic::runner::{
    cmd_bnm, cmd_bnm_basin, cmd_tipping, BnmBasinArgs, BnmRunArgs, TippingRunArgs,
};
use analytic::tipping::{FlowAsymmetry, Speculation, TippingConfig};
use analytic::tolerance::ToleranceSchedule;

// ---------------------------------------------------------------------------
// CLI definitions
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(
    name = "schelling",
    about = "Replication of Schelling (1971), Dynamic Models of Segregation"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    /// Development run: write it under results/_scratch/ so it is never synced to the vault.
    #[arg(long, global = true)]
    scratch: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run a single simulation (default)
    Run(RunArgs),
    /// Run parameter sensitivity analysis (grid search)
    Sweep(SweepArgs),
    /// Run the bounded-neighborhood model (analytical model) once
    Bnm(BnmArgs),
    /// Analyze basins of attraction for the bounded-neighborhood model (initial-condition grid sweep)
    BnmBasin(BnmBasinCliArgs),
    /// Run the tipping model (extended dynamics including speculation, asymmetry, and channeling)
    Tipping(TippingArgs),
}

#[derive(Parser, Debug)]
struct RunArgs {
    /// Number of grid rows
    #[arg(long, default_value_t = 13)]
    rows: usize,

    /// Number of grid columns
    #[arg(long, default_value_t = 16)]
    cols: usize,

    /// Number of agents in group A (0 = calculate automatically)
    #[arg(long, default_value_t = 0)]
    n_a: usize,

    /// Number of agents in group B (0 = calculate automatically)
    #[arg(long, default_value_t = 0)]
    n_b: usize,

    /// Vacancy rate [0, 1]
    #[arg(long, default_value_t = 0.30)]
    vacant_rate: f64,

    /// Tolerance limit τ: minimum required same-color neighbor ratio (used only when --rule is not specified)
    #[arg(long, default_value_t = 0.333)]
    threshold: f64,

    /// Satisfaction rule: "ratio:X" (segregation form) / "min-same:N" (congregation form, Fig.16) /
    /// "bounded:L:H" (integration form, Fig.17). If omitted, construct a ratio rule from --threshold.
    #[arg(long)]
    rule: Option<String>,

    /// Movement operation mode: "standard" (loose operation; only dissatisfied agents move) / "strict" (strict operation, Fig.8;
    /// satisfied agents also move speculatively to vacant cells that strictly improve the same-color ratio).
    #[arg(long, default_value = "standard")]
    move_mode: String,

    /// Destination-selection strategy: "nearest" (first satisfactory vacant cell at the shortest distance; existing behavior) /
    /// "best-local" (cell with the highest post-move same-color ratio in the nearest distance band; improves the
    /// minority cluster ratio for unequal numbers in Fig.12).
    #[arg(long, default_value = "nearest")]
    move_strategy: String,

    /// Maximum number of iterations
    #[arg(long, default_value_t = 500)]
    max_iterations: usize,

    /// Random seed (random if omitted)
    #[arg(long)]
    seed: Option<u64>,

    /// Snapshot-saving interval (0 = do not save)
    #[arg(long, default_value_t = 1)]
    snapshot_interval: usize,

    /// Results output directory
    #[arg(long, default_value = "results")]
    output_dir: String,
}

#[derive(Parser, Debug)]
struct SweepArgs {
    /// Range of tolerance limits τ ("start:stop:step" or a single value)
    #[arg(long, default_value = "0.333")]
    threshold: String,

    /// Range of vacancy rates ("start:stop:step" or a single value)
    #[arg(long, default_value = "0.30")]
    vacant_rate: String,

    /// Number of grid rows
    #[arg(long, default_value_t = 13)]
    rows: usize,

    /// Number of grid columns
    #[arg(long, default_value_t = 16)]
    cols: usize,

    /// Comma-separated random seeds (e.g., "42,123,456")
    #[arg(long, default_value = "42")]
    seeds: String,

    /// Maximum number of iterations
    #[arg(long, default_value_t = 500)]
    max_iterations: usize,

    /// Snapshot-saving interval (0 = do not save; default for sweep is 0)
    #[arg(long, default_value_t = 0)]
    snapshot_interval: usize,

    /// Base directory for result output
    #[arg(long, default_value = "results")]
    output_dir: String,
}

// ---------------------------------------------------------------------------
// BNM-related CLI arguments
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
struct BnmArgs {
    /// Preset name (fig18 / fig19 / fig20 / fig21 / fig22 / fig23 / fig24 / fig25 /
    /// fig26 / fig27 / fig28 / fig29). If omitted, --w-tolerance and related arguments are required.
    #[arg(long)]
    preset: Option<String>,

    /// Tolerance schedule for group W, in "linear:r_max=2.0:pop_max=100" form.
    /// Overrides the preset when one is specified.
    #[arg(long)]
    w_tolerance: Option<String>,

    /// Tolerance schedule for group B.
    #[arg(long)]
    b_tolerance: Option<String>,

    /// Capacity constraint W+B<=C. Unlimited if omitted.
    #[arg(long)]
    capacity: Option<f64>,

    /// Initial values "W,B". The preset's default_init is used when a preset is specified.
    #[arg(long)]
    init: Option<String>,

    /// Flow-rate model: "continuous:k_w=1.0:k_b=1.0:dt=0.1" / "discrete"
    #[arg(long, default_value = "continuous:k_w=1.0:k_b=1.0:dt=0.1")]
    flow: String,

    /// Maximum number of steps
    #[arg(long, default_value_t = 5000)]
    max_steps: usize,

    /// Convergence tolerance
    #[arg(long, default_value_t = 1e-4)]
    convergence_tol: f64,

    /// Results output directory
    #[arg(long, default_value = "results")]
    output_dir: String,
}

#[derive(Parser, Debug)]
struct TippingArgs {
    /// Preset name (fig30a / fig30b / fig31 / fig32, etc.).
    #[arg(long)]
    preset: Option<String>,

    #[arg(long)]
    w_tolerance: Option<String>,

    #[arg(long)]
    b_tolerance: Option<String>,

    #[arg(long)]
    capacity: Option<f64>,

    #[arg(long)]
    init: Option<String>,

    /// Speculation model: "none" / "linear:alpha=0.3" / "trend:window=5:weight=0.5"
    #[arg(long, default_value = "none")]
    speculation: String,

    /// Flow-rate asymmetry: "w_in=1.0:w_out=1.0:b_in=1.0:b_out=1.0" (symmetric if omitted)
    #[arg(long)]
    asymmetry: Option<String>,

    /// Channeling (effective-capacity reduction factor 0..=1). Used with capacity.
    #[arg(long)]
    channeling: Option<f64>,

    #[arg(long, default_value = "continuous:k_w=1.0:k_b=1.0:dt=0.1")]
    flow: String,

    #[arg(long, default_value_t = 5000)]
    max_steps: usize,

    #[arg(long, default_value_t = 1e-4)]
    convergence_tol: f64,

    #[arg(long, default_value = "results")]
    output_dir: String,
}

#[derive(Parser, Debug)]
struct BnmBasinCliArgs {
    #[arg(long)]
    preset: Option<String>,

    #[arg(long)]
    w_tolerance: Option<String>,

    #[arg(long)]
    b_tolerance: Option<String>,

    #[arg(long)]
    capacity: Option<f64>,

    /// Number of divisions in the initial-condition grid, "n_w x n_b".
    #[arg(long, default_value = "20x20")]
    init_grid: String,

    #[arg(long, default_value = "continuous:k_w=1.0:k_b=1.0:dt=0.1")]
    flow: String,

    #[arg(long, default_value_t = 3000)]
    max_steps: usize,

    #[arg(long, default_value_t = 1e-3)]
    convergence_tol: f64,

    #[arg(long, default_value = "results")]
    output_dir: String,
}

// ---------------------------------------------------------------------------
// BNM argument parsers
// ---------------------------------------------------------------------------

/// Parses strings such as "linear:r_max=2.0:pop_max=100" into a ToleranceSchedule.
fn parse_tolerance_string(s: &str) -> ToleranceSchedule {
    let parts: Vec<&str> = s.split(':').collect();
    let kind = parts[0];
    let mut kwargs: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for kv in &parts[1..] {
        let mut it = kv.splitn(2, '=');
        let k = it.next().expect("expected key=value format").to_string();
        let v: f64 = it
            .next()
            .expect("expected key=value format")
            .parse()
            .expect("failed to parse number");
        kwargs.insert(k, v);
    }
    let pop_max = *kwargs.get("pop_max").expect("pop_max is required");
    match kind {
        "linear" => {
            let r_max = *kwargs.get("r_max").expect("r_max is required");
            ToleranceSchedule::Linear { r_max, pop_max }
        }
        "affine" => {
            let intercept_pop = *kwargs.get("intercept_pop").unwrap_or(&0.0);
            let slope = *kwargs.get("slope").expect("slope is required");
            ToleranceSchedule::Affine {
                intercept_pop,
                slope,
                pop_max,
            }
        }
        _ => panic!("unsupported schedule type: \"{}\" (linear / affine)", kind),
    }
}

/// Parses "continuous:k_w=1.0:k_b=1.0:dt=0.1" / "discrete" into a FlowModel.
fn parse_flow_string(s: &str) -> FlowModel {
    let parts: Vec<&str> = s.split(':').collect();
    match parts[0] {
        "continuous" => {
            let mut kwargs: std::collections::HashMap<String, f64> =
                std::collections::HashMap::new();
            for kv in &parts[1..] {
                let mut it = kv.splitn(2, '=');
                let k = it.next().expect("expected key=value format").to_string();
                let v: f64 = it
                    .next()
                    .expect("expected key=value format")
                    .parse()
                    .expect("failed to parse number");
                kwargs.insert(k, v);
            }
            FlowModel::Continuous {
                k_w: *kwargs.get("k_w").unwrap_or(&1.0),
                k_b: *kwargs.get("k_b").unwrap_or(&1.0),
                dt: *kwargs.get("dt").unwrap_or(&0.1),
            }
        }
        "discrete" => FlowModel::DiscreteBatch,
        _ => panic!(
            "unsupported flow type: \"{}\" (continuous / discrete)",
            parts[0]
        ),
    }
}

/// Parses "W,B" into a tuple.
fn parse_init_string(s: &str) -> (f64, f64) {
    let parts: Vec<&str> = s.split(',').collect();
    assert_eq!(parts.len(), 2, "init must use \"W,B\" format");
    let w: f64 = parts[0].trim().parse().expect("failed to parse W");
    let b: f64 = parts[1].trim().parse().expect("failed to parse B");
    (w, b)
}

/// Parses "20x20" into (20, 20).
fn parse_grid_string(s: &str) -> (usize, usize) {
    let parts: Vec<&str> = s.split('x').collect();
    assert_eq!(parts.len(), 2, "init-grid must use \"NxM\" format");
    let n: usize = parts[0].trim().parse().expect("failed to parse N");
    let m: usize = parts[1].trim().parse().expect("failed to parse M");
    (n, m)
}

/// Builds a PhaseConfig, initial values, and a preset name from BnmArgs.
fn build_bnm_inputs(
    preset: Option<String>,
    w_tol: Option<String>,
    b_tol: Option<String>,
    capacity: Option<f64>,
    init: Option<String>,
) -> (Option<String>, PhaseConfig, (f64, f64)) {
    if let Some(name) = &preset {
        let p = analytic::preset::lookup(name).unwrap_or_else(|| {
            panic!(
                "unknown preset: \"{}\" (available: {:?})",
                name,
                analytic::preset::all_names()
            )
        });
        let mut phase = p.phase;
        if let Some(s) = w_tol {
            phase.w_schedule = parse_tolerance_string(&s);
        }
        if let Some(s) = b_tol {
            phase.b_schedule = parse_tolerance_string(&s);
        }
        if let Some(c) = capacity {
            phase.capacity = Some(c);
        }
        let init = init
            .map(|s| parse_init_string(&s))
            .unwrap_or(p.default_init);
        (Some(name.clone()), phase, init)
    } else {
        let w_schedule =
            parse_tolerance_string(&w_tol.expect("--w-tolerance is required without --preset"));
        let b_schedule =
            parse_tolerance_string(&b_tol.expect("--b-tolerance is required without --preset"));
        let phase = PhaseConfig {
            w_schedule,
            b_schedule,
            capacity,
        };
        let init = init.map(|s| parse_init_string(&s)).unwrap_or((0.0, 0.0));
        (None, phase, init)
    }
}

// ---------------------------------------------------------------------------
// BNM subcommand implementation
// ---------------------------------------------------------------------------

fn cmd_bnm_dispatch(args: BnmArgs, scratch: bool) {
    let (preset_name, phase, init) = build_bnm_inputs(
        args.preset,
        args.w_tolerance,
        args.b_tolerance,
        args.capacity,
        args.init,
    );
    let dynamics = DynamicsConfig {
        flow: parse_flow_string(&args.flow),
        max_steps: args.max_steps,
        convergence_tol: args.convergence_tol,
    };
    cmd_bnm(
        BnmRunArgs {
            preset_name,
            phase,
            dynamics,
            init,
            output_base: args.output_dir,
        },
        scratch,
    );
}

/// Parses a speculation string.
fn parse_speculation_string(s: &str) -> Speculation {
    let parts: Vec<&str> = s.split(':').collect();
    match parts[0] {
        "none" => Speculation::None,
        "linear" => {
            let mut alpha = 0.0_f64;
            for kv in &parts[1..] {
                let mut it = kv.splitn(2, '=');
                let k = it.next().unwrap_or("");
                let v: f64 = it.next().unwrap_or("0").parse().unwrap_or(0.0);
                if k == "alpha" {
                    alpha = v;
                }
            }
            Speculation::Linear { alpha }
        }
        "trend" => {
            let mut window = 5_usize;
            let mut weight = 0.5_f64;
            for kv in &parts[1..] {
                let mut it = kv.splitn(2, '=');
                let k = it.next().unwrap_or("");
                let v = it.next().unwrap_or("0");
                match k {
                    "window" => window = v.parse().unwrap_or(5),
                    "weight" => weight = v.parse().unwrap_or(0.5),
                    _ => {}
                }
            }
            Speculation::Trend { window, weight }
        }
        _ => panic!(
            "unsupported speculation model: \"{}\" (none / linear / trend)",
            parts[0]
        ),
    }
}

/// Parses a flow-rate asymmetry string.
fn parse_asymmetry_string(s: &str) -> FlowAsymmetry {
    let mut a = FlowAsymmetry {
        w_inflow: 1.0,
        w_outflow: 1.0,
        b_inflow: 1.0,
        b_outflow: 1.0,
    };
    for kv in s.split(':') {
        let mut it = kv.splitn(2, '=');
        let k = it.next().unwrap_or("");
        let v: f64 = it.next().unwrap_or("1").parse().unwrap_or(1.0);
        match k {
            "w_in" => a.w_inflow = v,
            "w_out" => a.w_outflow = v,
            "b_in" => a.b_inflow = v,
            "b_out" => a.b_outflow = v,
            _ => panic!("unsupported key: \"{}\" (w_in/w_out/b_in/b_out)", k),
        }
    }
    a
}

fn cmd_tipping_dispatch(args: TippingArgs, scratch: bool) {
    let (preset_name, phase, init) = build_bnm_inputs(
        args.preset,
        args.w_tolerance,
        args.b_tolerance,
        args.capacity,
        args.init,
    );
    let dynamics = DynamicsConfig {
        flow: parse_flow_string(&args.flow),
        max_steps: args.max_steps,
        convergence_tol: args.convergence_tol,
    };
    let speculation = parse_speculation_string(&args.speculation);
    let asymmetry = args.asymmetry.map(|s| parse_asymmetry_string(&s));
    let tipping = TippingConfig {
        phase,
        dynamics,
        speculation,
        asymmetry,
        channeling: args.channeling,
    };
    cmd_tipping(
        TippingRunArgs {
            preset_name,
            tipping,
            init,
            output_base: args.output_dir,
        },
        scratch,
    );
}

fn cmd_bnm_basin_dispatch(args: BnmBasinCliArgs, scratch: bool) {
    let (preset_name, phase, _) = build_bnm_inputs(
        args.preset,
        args.w_tolerance,
        args.b_tolerance,
        args.capacity,
        None,
    );
    let dynamics = DynamicsConfig {
        flow: parse_flow_string(&args.flow),
        max_steps: args.max_steps,
        convergence_tol: args.convergence_tol,
    };
    let (n_w, n_b) = parse_grid_string(&args.init_grid);
    cmd_bnm_basin(
        BnmBasinArgs {
            preset_name,
            phase,
            dynamics,
            n_w,
            n_b,
            output_base: args.output_dir,
        },
        scratch,
    );
}

// ---------------------------------------------------------------------------
// Range-string parser
// ---------------------------------------------------------------------------

/// Estimates the number of decimal places from the string representation
fn step_decimals(v: f64) -> usize {
    let s = format!("{}", v);
    match s.find('.') {
        Some(pos) => s.len() - pos - 1,
        None => 0,
    }
}

/// Returns an arithmetic sequence for "start:stop:step" and a one-element Vec for a single value.
/// Calculates the number of steps as an integer to tolerate floating-point error.
fn parse_range(s: &str) -> Vec<f64> {
    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        1 => {
            let v: f64 = parts[0].parse().expect("failed to parse number");
            vec![v]
        }
        3 => {
            let start: f64 = parts[0].parse().expect("failed to parse start");
            let stop: f64 = parts[1].parse().expect("failed to parse stop");
            let step: f64 = parts[2].parse().expect("failed to parse step");
            assert!(step > 0.0, "step must be positive");
            // Calculate the number of steps with a tolerance
            let n_steps = ((stop - start) / step + 0.5e-9).floor() as usize;
            // Infer the number of decimal places from step and round to eliminate floating-point error
            let decimals = step_decimals(step);
            let factor = 10_f64.powi(decimals as i32);
            (0..=n_steps)
                .map(|i| ((start + step * i as f64) * factor).round() / factor)
                .collect()
        }
        _ => panic!(
            "invalid range format: \"{}\" (expected \"start:stop:step\" or a single value)",
            s
        ),
    }
}

/// Parses a string into a SatisfactionRule.
///
/// - "ratio:0.333"      → Ratio { threshold: 0.333 }
/// - "min-same:3"       → MinSame { min_same: 3 }
/// - "bounded:3:6"      → Bounded { min_same: 3, max_same: 6 }
fn parse_rule_string(s: &str) -> SatisfactionRule {
    let parts: Vec<&str> = s.split(':').collect();
    match parts.as_slice() {
        ["ratio", t] => {
            let threshold: f64 = t.parse().expect("failed to parse ratio threshold");
            SatisfactionRule::Ratio { threshold }
        }
        ["min-same", n] => {
            let min_same: usize = n.parse().expect("failed to parse min-same value");
            SatisfactionRule::MinSame { min_same }
        }
        ["bounded", lo, hi] => {
            let min_same: usize = lo.parse().expect("failed to parse bounded lower limit");
            let max_same: usize = hi.parse().expect("failed to parse bounded upper limit");
            assert!(
                min_same <= max_same,
                "bounded rule requires lower limit ({}) <= upper limit ({})",
                min_same,
                max_same
            );
            SatisfactionRule::Bounded { min_same, max_same }
        }
        _ => panic!(
            "invalid rule format: \"{}\" (expected ratio:X, min-same:N, or bounded:L:H)",
            s
        ),
    }
}

// ---------------------------------------------------------------------------
// One row in the sweep console summary (not written to a file)
// ---------------------------------------------------------------------------

struct SweepRow {
    threshold: f64,
    vacant_rate: f64,
    seed: u64,
    converged: bool,
    final_iteration: usize,
    avg_same_ratio: f64,
}

// ---------------------------------------------------------------------------
// Structure for sweep_config.json
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
struct SweepConfigJson {
    threshold: serde_json::Value,
    vacant_rate: serde_json::Value,
    rows: usize,
    cols: usize,
    seeds: Vec<u64>,
    max_iterations: usize,
    snapshot_interval: usize,
}

// ---------------------------------------------------------------------------
// Structure for config.json (for run)
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
struct RunConfigJson {
    command: &'static str,
    rule: String,
    rule_kind: &'static str,
    move_mode: &'static str,
    move_strategy: &'static str,
    threshold: Option<f64>,
    min_same: Option<usize>,
    max_same: Option<usize>,
    rows: usize,
    cols: usize,
    n_a: usize,
    n_b: usize,
    n_vacant: usize,
    vacant_rate: f64,
    seed: Option<u64>,
    max_iterations: usize,
    snapshot_interval: usize,
}

fn run_config_json(cfg: &Config, vacant_rate: f64) -> RunConfigJson {
    let total = cfg.rows * cfg.cols;
    let n_vacant = total.saturating_sub(cfg.n_a + cfg.n_b);
    let (rule_kind, threshold, min_same, max_same) = match cfg.rule {
        SatisfactionRule::Ratio { threshold } => ("ratio", Some(threshold), None, None),
        SatisfactionRule::MinSame { min_same } => ("min-same", None, Some(min_same), None),
        SatisfactionRule::Bounded { min_same, max_same } => {
            ("bounded", None, Some(min_same), Some(max_same))
        }
    };
    RunConfigJson {
        command: "run",
        rule: cfg.rule.label(),
        rule_kind,
        move_mode: cfg.move_mode.label(),
        move_strategy: cfg.move_strategy.label(),
        threshold,
        min_same,
        max_same,
        rows: cfg.rows,
        cols: cfg.cols,
        n_a: cfg.n_a,
        n_b: cfg.n_b,
        n_vacant,
        vacant_rate,
        seed: cfg.seed,
        max_iterations: cfg.max_iterations,
        snapshot_interval: cfg.snapshot_interval,
    }
}

/// Converts a range string to JSON (range → {start, stop, step}, single value → number)
fn range_to_json(s: &str) -> serde_json::Value {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() == 3 {
        serde_json::json!({
            "start": parts[0].parse::<f64>().unwrap(),
            "stop":  parts[1].parse::<f64>().unwrap(),
            "step":  parts[2].parse::<f64>().unwrap(),
        })
    } else {
        serde_json::json!(parts[0].parse::<f64>().unwrap())
    }
}

// ---------------------------------------------------------------------------
// run subcommand (existing single-run logic)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Sharing a stage (mechanisms are `'static`, so they cannot borrow a Stage)
// ---------------------------------------------------------------------------

/// Wraps a `Stage` so it can be shared with a mechanism, returning an observer and a retrieval handle.
/// Retrieve it with [`close_shared`] when closing.
fn share_stage(stage: Stage) -> (Rc<RefCell<Option<Stage>>>, DecisionObserver) {
    let cell = Rc::new(RefCell::new(Some(stage)));
    let observer: DecisionObserver = {
        let cell = Rc::clone(&cell);
        Rc::new(RefCell::new(move || {
            if let Some(stage) = cell.borrow_mut().as_mut() {
                stage.tick();
            }
        }))
    };
    (cell, observer)
}

/// Retrieves and closes the shared stage.
///
/// manifest.csv is sealed by `finish()`. Adding a row afterward would leave the manifest
/// with a mismatched digest.
fn close_shared(cell: &Rc<RefCell<Option<Stage>>>) {
    if let Some(stage) = cell.borrow_mut().take() {
        stage.close();
    }
}

fn cmd_run(args: RunArgs, scratch: bool) {
    let total = args.rows * args.cols;
    let (n_a, n_b) = if args.n_a == 0 || args.n_b == 0 {
        let n_vacant = (total as f64 * args.vacant_rate).round() as usize;
        let n_agents = total - n_vacant;
        let a = n_agents / 2;
        (a, n_agents - a)
    } else {
        (args.n_a, args.n_b)
    };

    let rule = match &args.rule {
        Some(s) => parse_rule_string(s),
        None => SatisfactionRule::Ratio {
            threshold: args.threshold,
        },
    };

    let move_mode = MoveMode::parse(&args.move_mode).unwrap_or_else(|| {
        panic!(
            "unsupported move-mode: \"{}\" (standard / strict)",
            args.move_mode
        )
    });

    let move_strategy = MoveStrategy::parse(&args.move_strategy).unwrap_or_else(|| {
        panic!(
            "unsupported move-strategy: \"{}\" (nearest / best-local)",
            args.move_strategy
        )
    });

    // Materialize the seed before recording it. If the simulation falls back to
    // rand::random when --seed is omitted, the seed actually used is not recorded anywhere.
    let seed = args.seed.unwrap_or_else(rand::random::<u64>);

    let mut cfg = Config {
        rows: args.rows,
        cols: args.cols,
        n_a,
        n_b,
        rule,
        move_mode,
        move_strategy,
        max_iterations: args.max_iterations,
        seed: Some(seed),
        snapshot_interval: args.snapshot_interval,
        // Determined after Run::start selects the run directory.
        output_dir: String::new(),
    };

    let parameters = run_config_json(&cfg, args.vacant_rate);
    let mut rv = Run::start(
        RunOptions::new("schelling", "run")
            .scratch(scratch)
            .repo_id("schelling1971")
            .domain("simulation")
            .results_root(&args.output_dir)
            .parameters(&parameters)
            .expect("runvault: failed to build parameters")
            .seed_pointers(["/seed"])
            .master_seed(seed)
            .replication(record::replication()),
    )
    .expect("runvault: failed to start run");

    // The run directory itself becomes the output destination. Snapshots go under artifacts/.
    cfg.output_dir = rv.dir().join("artifacts").to_string_lossy().into_owned();

    println!("=== Schelling Segregation Model Replication ===");
    println!(
        "Grid: {}×{} | A: {} | B: {} | Vacant: {} | Rule: {} | Mode: {} | Strategy: {}",
        cfg.rows,
        cfg.cols,
        cfg.n_a,
        cfg.n_b,
        total - cfg.n_a - cfg.n_b,
        cfg.rule.label(),
        cfg.move_mode.label(),
        cfg.move_strategy.label(),
    );
    println!("Seed: {}", seed);
    println!("Output: {}", rv.dir().display());
    println!("---------------------------------------");

    // The unit is one agent's movement decision, not a step: a 400x400 run takes
    // 70.6 seconds and 8 steps to converge (8.8 seconds per step), while a 600x600 run takes
    // 470 seconds and 9 steps (52 seconds per step). With step-based counting, the number
    // would remain unchanged across 30-second reporting intervals (measured).
    //
    // This is unbounded because the mechanism terminates by issuing `request_stop` upon
    // convergence or a deadlock: `--max-iterations` is an upper bound that is not reached,
    // and the total number of decisions cannot be counted before execution.
    let (stage, observer) = share_stage(rv.unbounded_stage("decisions"));
    let result = run_simulation_observed(&cfg, observer);
    close_shared(&stage);
    record::log_simulation(&mut rv, &result);

    let last = result.metrics_history.last().unwrap();
    println!(
        "Converged: {} | Iterations: {}",
        if result.converged { "Yes" } else { "No" },
        result.final_iteration
    );
    println!(
        "Mean same-color neighbor ratio: {:.1}%",
        last.avg_same_ratio * 100.0
    );
    println!(
        "  Group A: {:.1}%  Group B: {:.1}%",
        last.avg_same_ratio_a * 100.0,
        last.avg_same_ratio_b * 100.0
    );
    println!(
        "No-opposite-neighbor percentage: {:.1}%",
        last.pct_no_opposite
    );

    let dir = rv.finish().expect("runvault: failed to finish run");
    println!("Metrics   → {}/metrics.csv", dir.display());
    println!("Config    → {}/config.json", dir.display());
    println!("Snapshots → {}/artifacts/snapshots/", dir.display());
}

// ---------------------------------------------------------------------------
// sweep subcommand
// ---------------------------------------------------------------------------

fn cmd_sweep(args: SweepArgs, scratch: bool) {
    let thresholds = parse_range(&args.threshold);
    let vacant_rates = parse_range(&args.vacant_rate);
    let seeds: Vec<u64> = args
        .seeds
        .split(',')
        .map(|s| s.trim().parse::<u64>().expect("failed to parse seed"))
        .collect();
    assert!(!seeds.is_empty(), "--seeds must not be empty");

    // Build the Cartesian product of all combinations
    struct Combo {
        threshold: f64,
        vacant_rate: f64,
        seed: u64,
        replicate_index: u64,
    }
    let mut combos: Vec<Combo> = Vec::new();
    for &tau in &thresholds {
        for &vac in &vacant_rates {
            for (i, &seed) in seeds.iter().enumerate() {
                combos.push(Combo {
                    threshold: tau,
                    vacant_rate: vac,
                    seed,
                    replicate_index: i as u64,
                });
            }
        }
    }
    let n_total = combos.len();

    // Parent run: parameters contain the grid definition itself; do not write metrics for individual conditions.
    let sweep_parameters = SweepConfigJson {
        threshold: range_to_json(&args.threshold),
        vacant_rate: range_to_json(&args.vacant_rate),
        rows: args.rows,
        cols: args.cols,
        seeds: seeds.clone(),
        max_iterations: args.max_iterations,
        snapshot_interval: args.snapshot_interval,
    };
    // The parent contains a seed "sequence," not a single master seed. The sequence remains
    // in execution_hash through /parameters.seeds and seed_pointers.
    // runvault fills sweep_id with the parent's run_slug.
    let parent = Run::start(
        RunOptions::new("schelling", "sweep")
            .scratch(scratch)
            .repo_id("schelling1971")
            .domain("simulation")
            .results_root(&args.output_dir)
            .parameters(&sweep_parameters)
            .expect("runvault: failed to build sweep parameters")
            .seed_pointers(["/seeds"])
            .sweep_parent()
            .replication(record::replication()),
    )
    .expect("runvault: failed to start parent sweep run");

    let sweep_id = parent
        .sweep_id()
        .expect("runvault: parent sweep run has no sweep_id")
        .to_string();
    let parent_run_uid = parent.run_uid().to_string();

    println!("=== Schelling Segregation Model Parameter Sweep ===");
    println!(
        "Grid: {}×{} | τ: {} values | vacant_rate: {} values | Seeds: {} | Total: {} runs",
        args.rows,
        args.cols,
        thresholds.len(),
        vacant_rates.len(),
        seeds.len(),
        n_total
    );
    println!("Output: {}", parent.dir().display());
    println!("-----------------------------------------------");

    let mut summary_rows: Vec<SweepRow> = Vec::with_capacity(n_total);

    // The unit is one trial for a condition (τ × vacancy rate × seed). Trial lengths vary
    // because they terminate on convergence or deadlock, but **the number of trials is known
    // exactly from the start**, so the stage can be bounded (the denominator is the actual result
    // of building `combos`, not a division of the ranges). Steps are not additionally counted
    // within each trial because a sweep scales with the number of conditions, not the length of one trial
    // (one 13x16 trial with 500 iterations takes a few milliseconds; measured: 13 values of τ × 10
    // vacancy rates × 5 seeds = 650 trials on a 50x50 grid took 92 seconds).
    let mut stage = parent.stage("trials", n_total);

    for (i, combo) in combos.iter().enumerate() {
        let total = args.rows * args.cols;
        let n_vacant = (total as f64 * combo.vacant_rate).round() as usize;
        let n_agents = total - n_vacant;
        let n_a = n_agents / 2;
        let n_b = n_agents - n_a;

        let mut cfg = Config {
            rows: args.rows,
            cols: args.cols,
            n_a,
            n_b,
            rule: SatisfactionRule::Ratio {
                threshold: combo.threshold,
            },
            move_mode: MoveMode::Standard,
            move_strategy: MoveStrategy::Nearest,
            max_iterations: args.max_iterations,
            seed: Some(combo.seed),
            snapshot_interval: args.snapshot_interval,
            output_dir: String::new(),
        };

        let parameters = run_config_json(&cfg, combo.vacant_rate);
        let mut child = Run::start(
            RunOptions::new("schelling", "run")
                .scratch(scratch)
                .repo_id("schelling1971")
                .domain("simulation")
                .results_root(&args.output_dir)
                .parameters(&parameters)
                .expect("runvault: failed to build child-run parameters")
                .seed_pointers(["/seed"])
                .master_seed(combo.seed)
                .replicate_index(combo.replicate_index)
                .lineage(Lineage {
                    sweep_id: Some(sweep_id.clone()),
                    parent_run_uid: Some(parent_run_uid.clone()),
                    ..Default::default()
                })
                .replication(record::replication()),
        )
        .expect("runvault: failed to start child run");

        cfg.output_dir = child.dir().join("artifacts").to_string_lossy().into_owned();

        let result = run_simulation(&cfg);
        record::log_simulation(&mut child, &result);

        let last = result.metrics_history.last().unwrap();

        println!(
            "[{}/{}] τ={:.3} vacant={:.3} seed={} → converged={} iter={} avg_same={:.3}",
            i + 1,
            n_total,
            combo.threshold,
            combo.vacant_rate,
            combo.seed,
            if result.converged { "Yes" } else { "No" },
            result.final_iteration,
            last.avg_same_ratio,
        );

        summary_rows.push(SweepRow {
            threshold: combo.threshold,
            vacant_rate: combo.vacant_rate,
            seed: combo.seed,
            converged: result.converged,
            final_iteration: result.final_iteration,
            avg_same_ratio: last.avg_same_ratio,
        });

        child
            .finish()
            .expect("runvault: failed to finish child run");
        stage.tick();
    }
    stage.close();

    // Display the summary table
    println!("===============================================");
    println!("Sweep complete: {} runs", n_total);
    println!("-----------------------------------------------");
    println!(
        "{:<10} {:<12} {:<6} {:<10} {:<6} {:<10}",
        "threshold", "vacant_rate", "seed", "converged", "iter", "avg_same"
    );
    println!("{}", "-".repeat(60));
    for row in &summary_rows {
        println!(
            "{:<10.3} {:<12.3} {:<6} {:<10} {:<6} {:.3}",
            row.threshold,
            row.vacant_rate,
            row.seed,
            if row.converged { "Yes" } else { "No" },
            row.final_iteration,
            row.avg_same_ratio,
        );
    }

    let dir = parent
        .finish()
        .expect("runvault: failed to finish parent sweep run");
    println!("-----------------------------------------------");
    println!("Sweep definition → {}/config.json", dir.display());
    println!("Metrics for each condition are in the child run's metrics.csv (subcommand=run)");
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

/// Wrapper structure that interprets an invocation without a subcommand as `run`.
/// First attempts to parse flat arguments with clap's `try_parse_from`,
/// and parses with a subcommand only if that fails.
#[derive(Parser, Debug)]
#[command(
    name = "schelling",
    about = "Replication of Schelling (1971), Dynamic Models of Segregation"
)]
struct FlatRunCli {
    /// Development run: write it under results/_scratch/ so it is never synced to the vault.
    #[arg(long, global = true)]
    scratch: bool,
    #[command(flatten)]
    args: RunArgs,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Branch on whether the first argument is a subcommand name
    let has_subcommand = args
        .get(1)
        .map(|a| {
            a == "run"
                || a == "sweep"
                || a == "bnm"
                || a == "bnm-basin"
                || a == "tipping"
                || a == "help"
                || a == "--help"
                || a == "-h"
        })
        .unwrap_or(false);

    if has_subcommand {
        let cli = Cli::parse_from(&args);
        let scratch = cli.scratch;
        match cli.command {
            Some(Commands::Run(run_args)) => cmd_run(run_args, scratch),
            Some(Commands::Sweep(sweep_args)) => cmd_sweep(sweep_args, scratch),
            Some(Commands::Bnm(bnm_args)) => cmd_bnm_dispatch(bnm_args, scratch),
            Some(Commands::BnmBasin(basin_args)) => cmd_bnm_basin_dispatch(basin_args, scratch),
            Some(Commands::Tipping(tipping_args)) => cmd_tipping_dispatch(tipping_args, scratch),
            None => cmd_run(RunArgs::parse_from(args.iter().take(1)), scratch),
        }
    } else {
        // Interpret as flat arguments without a subcommand (backward compatibility)
        let flat = FlatRunCli::parse_from(&args);
        cmd_run(flat.args, flat.scratch);
    }
}
