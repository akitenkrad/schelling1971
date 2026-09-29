use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::BufWriter;

use csv::Writer;
use rand::seq::SliceRandom;

use socsim_core::{derive_seed, AgentId, SimRng};
use socsim_engine::{RandomActivationScheduler, SimulationBuilder};
use socsim_grid::{Boundary, Grid, GridIndex};

use crate::config::Config;
use crate::grid::Cell;
use crate::mechanisms::{no_observer, DecisionObserver, SchellingMoveMechanism};
use crate::metrics::Metrics;
use crate::world::SchellingWorld;

/// Results of an entire simulation run.
pub struct SimulationResult {
    pub metrics_history: Vec<Metrics>,
    pub converged: bool,
    pub final_iteration: usize,
}

// Labels for deriving independent deterministic RNG streams for specific purposes from a
// single root seed. `derive_seed(root, &[label])` yields mutually uncorrelated seeds.
/// Label for the RNG used to shuffle the initial placement.
const RNG_WORLD_INIT: u64 = 0;
/// Label for the socsim engine RNG (activation-order scheduler).
const RNG_ENGINE: u64 = 1;

/// Randomly initializes the grid and constructs the occupancy index and color map.
///
/// As in the previous implementation, groups A and B are placed at shuffled positions, then
/// occupied cells are scanned in **row-major** order to assign `AgentId(0..)` sequentially
/// (by position rather than placement order). This preserves deterministic ID assignment.
pub fn init_world(cfg: &Config, rng: &mut SimRng) -> (GridIndex, BTreeMap<AgentId, Cell>) {
    let total = cfg.rows * cfg.cols;
    assert!(
        cfg.n_a + cfg.n_b <= total,
        "number of agents ({}) exceeds grid size ({})",
        cfg.n_a + cfg.n_b,
        total
    );

    let mut positions: Vec<(usize, usize)> = (0..cfg.rows)
        .flat_map(|r| (0..cfg.cols).map(move |c| (r, c)))
        .collect();
    positions.shuffle(rng);

    // First finalize cell colors in a two-dimensional array (the same procedure as before).
    let mut cells = vec![vec![Cell::Empty; cfg.cols]; cfg.rows];
    for &(r, c) in positions.iter().take(cfg.n_a) {
        cells[r][c] = Cell::GroupA;
    }
    for &(r, c) in positions.iter().skip(cfg.n_a).take(cfg.n_b) {
        cells[r][c] = Cell::GroupB;
    }

    // Scan occupied cells in row-major order, assign AgentId(0..), and populate the indexes.
    let mut index = GridIndex::new(Grid::new(cfg.rows, cfg.cols, Boundary::Fixed));
    let mut colors: BTreeMap<AgentId, Cell> = BTreeMap::new();
    let mut next_id = 0u64;
    for (r, row) in cells.iter().enumerate() {
        for (c, &cell) in row.iter().enumerate() {
            if cell != Cell::Empty {
                let id = AgentId(next_id);
                index
                    .place(id, r, c)
                    .expect("failed to place agent initially");
                colors.insert(id, cell);
                next_id += 1;
            }
        }
    }

    (index, colors)
}

/// Runs the simulation.
///
/// Internally drives the socsim framework's [`Simulation`](socsim_engine::Simulation) engine.
/// [`SchellingMoveMechanism`] applies the movement rule in the `Decision` phase, and
/// [`RandomActivationScheduler`] determines the activation order at each step.
///
/// The movement mechanism requests early termination through
/// [`StepContext::request_stop`](socsim_core::StepContext::request_stop), and the driver exits
/// the loop after checking [`Simulation::stop_requested`](socsim_engine::Simulation::stop_requested).
/// Step results (number moved, number dissatisfied, and convergence) are received through
/// [`Simulation::scratch`](socsim_engine::Simulation::scratch).
pub fn run(cfg: &Config) -> SimulationResult {
    run_observed(cfg, no_observer())
}

/// Runs the same simulation as [`run`] and calls `on_decision` once whenever the mechanism
/// makes a movement decision for one agent.
///
/// See [`DecisionObserver`](crate::mechanisms::DecisionObserver) for why decisions, rather than
/// steps, are counted.
pub fn run_observed(cfg: &Config, on_decision: DecisionObserver) -> SimulationResult {
    // Prepare the output directory.
    let snapshots_dir = format!("{}/snapshots", cfg.output_dir);
    fs::create_dir_all(&snapshots_dir).expect("failed to create snapshot directory");

    // Choose the random seed (random if unspecified) and derive purpose-specific seeds from it.
    let root = cfg.seed.unwrap_or_else(rand::random);

    // Initialize the grid (using the placement-shuffle RNG derived from the root).
    let mut init_rng = SimRng::from_seed(derive_seed(root, &[RNG_WORLD_INIT]));
    let (index, colors) = init_world(cfg, &mut init_rng);

    // Build the world state and engine (with an engine RNG derived under a separate label).
    let world = SchellingWorld::with_modes(
        index,
        colors,
        cfg.rule,
        cfg.move_mode,
        cfg.move_strategy,
        cfg.max_iterations as u64,
    );
    let mut sim = SimulationBuilder::new(world)
        .scheduler(Box::new(RandomActivationScheduler))
        .seed(derive_seed(root, &[RNG_ENGINE]))
        .add_mechanism(Box::new(SchellingMoveMechanism::new(on_decision)))
        .build();

    // Metrics history.
    let mut metrics_history: Vec<Metrics> = Vec::new();

    // Record and save the initial state (step 0).
    metrics_history.push(Metrics::compute(sim.world(), 0, 0, 0));
    if cfg.snapshot_interval > 0 {
        save_snapshot(sim.world(), 0, &snapshots_dir);
    }

    // Observed execution loop for the socsim engine. `run_observed`
    // `while !clock.is_done() && !stop_requested { step(); observe(report); if stop break }`
    // runs the loop and invokes the observer once per step, including the step that requested
    // termination. This preserves the step count, RNG use, and observation timing of the former
    // handwritten loop (step -> read scratch -> break on stop_requested).
    //
    // To finalize the convergence flag and final iteration from the stopping step (or last step),
    // overwrite mutable variables outside the closure on every step (`report.t` is the clock after
    // the step, i.e. the one-based iteration number, matching `iteration` in the former loop).
    let mut converged = false;
    let mut final_iteration = cfg.max_iterations;
    sim.run_observed(|report| {
        let iteration = report.t as usize;

        let n_dissatisfied = *report
            .scratch
            .get::<usize>("n_dissatisfied")
            .expect("n_dissatisfied is missing from scratch data");
        let n_moved = *report
            .scratch
            .get::<usize>("n_moved")
            .expect("n_moved is missing from scratch data");
        let step_converged = *report
            .scratch
            .get::<bool>("converged")
            .expect("converged is missing from scratch data");

        // Record metrics.
        metrics_history.push(Metrics::compute(
            report.world,
            iteration,
            n_dissatisfied,
            n_moved,
        ));

        // Save a snapshot.
        if cfg.snapshot_interval > 0 && iteration.is_multiple_of(cfg.snapshot_interval) {
            save_snapshot(report.world, iteration, &snapshots_dir);
        }

        // Retain each step's convergence flag and iteration number; the stopping/final values remain.
        converged = step_converged;
        final_iteration = iteration;
    })
    .expect("simulation failed");

    SimulationResult {
        metrics_history,
        converged,
        final_iteration,
    }
}

/// Saves a grid snapshot as CSV.
/// Format: row,col,cell  (cell: 0=empty, 1=A, 2=B)
pub fn save_snapshot(world: &SchellingWorld, step: usize, dir: &str) {
    let path = format!("{}/step_{:05}.csv", dir, step);
    let file = File::create(&path).expect("failed to create snapshot file");
    let mut wtr = Writer::from_writer(BufWriter::new(file));
    wtr.write_record(["row", "col", "cell"])
        .expect("failed to write header");
    for r in 0..world.rows() {
        for c in 0..world.cols() {
            wtr.write_record(&[
                r.to_string(),
                c.to_string(),
                world.cell_color(r, c).to_int().to_string(),
            ])
            .expect("failed to write record");
        }
    }
    wtr.flush().expect("failed to flush writer");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{MoveMode, MoveStrategy, SatisfactionRule};
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Allocates a unique output directory for a test.
    fn temp_output_dir() -> String {
        static N: AtomicUsize = AtomicUsize::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "schelling_test_{}_{}_{}",
            std::process::id(),
            n,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        dir.to_string_lossy().into_owned()
    }

    fn test_config(output_dir: String) -> Config {
        Config {
            rows: 13,
            cols: 16,
            n_a: 73,
            n_b: 73,
            rule: SatisfactionRule::Ratio { threshold: 0.5 },
            move_mode: MoveMode::Standard,
            move_strategy: MoveStrategy::Nearest,
            max_iterations: 200,
            seed: Some(42),
            snapshot_interval: 0, // Minimize I/O.
            output_dir,
        }
    }

    fn strict_config(output_dir: String) -> Config {
        Config {
            move_mode: MoveMode::Strict,
            ..test_config(output_dir)
        }
    }

    /// Configuration for measuring the minority (B) cluster ratio with unequal numbers (2:1).
    fn unequal_config(output_dir: String, strategy: MoveStrategy) -> Config {
        Config {
            n_a: 97,
            n_b: 49,
            rule: SatisfactionRule::Ratio {
                threshold: 1.0 / 3.0,
            },
            move_strategy: strategy,
            ..test_config(output_dir)
        }
    }

    /// The same seed produces identical results even through the socsim engine.
    #[test]
    fn same_seed_is_deterministic() {
        let a = run(&test_config(temp_output_dir()));
        let b = run(&test_config(temp_output_dir()));
        assert_eq!(a.converged, b.converged);
        assert_eq!(a.final_iteration, b.final_iteration);
        assert_eq!(a.metrics_history.len(), b.metrics_history.len());
        for (ma, mb) in a.metrics_history.iter().zip(&b.metrics_history) {
            assert_eq!(ma.n_dissatisfied, mb.n_dissatisfied);
            assert_eq!(ma.n_moved, mb.n_moved);
            assert!((ma.avg_same_ratio - mb.avg_same_ratio).abs() < 1e-12);
        }
    }

    /// Previous implementation semantics (standard mode): moves per step do not exceed the
    /// number dissatisfied at the start of the step.
    #[test]
    fn moved_never_exceeds_dissatisfied() {
        let result = run(&test_config(temp_output_dir()));
        for m in &result.metrics_history {
            assert!(
                m.n_moved <= m.n_dissatisfied,
                "step {}: n_moved={} > n_dissatisfied={}",
                m.step,
                m.n_moved,
                m.n_dissatisfied
            );
        }
    }

    /// Strict mode (Fig.8) is also fully reproducible with the same seed.
    #[test]
    fn strict_mode_is_deterministic() {
        let a = run(&strict_config(temp_output_dir()));
        let b = run(&strict_config(temp_output_dir()));
        assert_eq!(a.converged, b.converged);
        assert_eq!(a.final_iteration, b.final_iteration);
        assert_eq!(a.metrics_history.len(), b.metrics_history.len());
        for (ma, mb) in a.metrics_history.iter().zip(&b.metrics_history) {
            assert_eq!(ma.n_moved, mb.n_moved);
            assert!((ma.avg_same_ratio - mb.avg_same_ratio).abs() < 1e-12);
        }
    }

    /// Because satisfied agents also move speculatively in strict mode, segregation (mean
    /// same-color ratio) is higher than in standard mode, corresponding to the paper's stronger
    /// segregation in Fig.8 than in Fig.9. After stopping, the state is stable: no agent can
    /// improve its same-color ratio.
    #[test]
    fn strict_mode_segregates_more_than_standard() {
        let standard = run(&test_config(temp_output_dir()));
        let strict = run(&strict_config(temp_output_dir()));
        let std_final = standard.metrics_history.last().unwrap().avg_same_ratio;
        let strict_final = strict.metrics_history.last().unwrap().avg_same_ratio;
        assert!(
            strict_final >= std_final,
            "strict avg_same={} should be >= standard avg_same={}",
            strict_final,
            std_final
        );
    }

    /// In strict mode, speculative moves can produce steps where
    /// `n_moved > n_dissatisfied` because moves by satisfied agents are included. These semantics
    /// differ from the inequality in standard mode.
    #[test]
    fn strict_mode_allows_speculative_moves() {
        let result = run(&strict_config(temp_output_dir()));
        let any_speculative = result
            .metrics_history
            .iter()
            .any(|m| m.n_moved > m.n_dissatisfied);
        assert!(
            any_speculative,
            "strict mode should exhibit at least one step with speculative moves"
        );
    }

    /// The BestLocal strategy is also fully reproducible with the same seed.
    #[test]
    fn best_local_is_deterministic() {
        let a = run(&unequal_config(temp_output_dir(), MoveStrategy::BestLocal));
        let b = run(&unequal_config(temp_output_dir(), MoveStrategy::BestLocal));
        assert_eq!(a.final_iteration, b.final_iteration);
        assert_eq!(a.metrics_history.len(), b.metrics_history.len());
        for (ma, mb) in a.metrics_history.iter().zip(&b.metrics_history) {
            assert!((ma.avg_same_ratio_b - mb.avg_same_ratio_b).abs() < 1e-12);
        }
    }

    /// With unequal numbers (Fig.12), BestLocal raises the minority (B) same-color ratio at least
    /// as much as Nearest by clustering agents in more homogeneous areas.
    #[test]
    fn best_local_improves_minority_clustering() {
        let nearest = run(&unequal_config(temp_output_dir(), MoveStrategy::Nearest));
        let best = run(&unequal_config(temp_output_dir(), MoveStrategy::BestLocal));
        let near_b = nearest.metrics_history.last().unwrap().avg_same_ratio_b;
        let best_b = best.metrics_history.last().unwrap().avg_same_ratio_b;
        assert!(
            best_b >= near_b,
            "best-local minority B avg_same={} should be >= nearest={}",
            best_b,
            near_b
        );
    }
}
