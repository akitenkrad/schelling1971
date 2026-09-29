/// Satisfaction rules corresponding to the three preference forms in Schelling (1971).
///
/// - `Ratio`         : Satisfied when the same-color neighbor ratio meets or exceeds the threshold (default segregation form, Fig. 7-14)
/// - `MinSame`       : Satisfied when the absolute number of same-color neighbors meets or exceeds the minimum (congregation form, Fig. 16)
/// - `Bounded`       : Satisfied when the absolute number of same-color neighbors is within [min_same, max_same] (integration form, Fig. 17)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SatisfactionRule {
    Ratio { threshold: f64 },
    MinSame { min_same: usize },
    Bounded { min_same: usize, max_same: usize },
}

impl SatisfactionRule {
    /// Evaluates satisfaction from the numbers of same-color and occupied neighbors.
    pub fn evaluate(&self, same: usize, total_occupied: usize) -> bool {
        match *self {
            SatisfactionRule::Ratio { threshold } => {
                // Satisfied when there are no occupied neighbors (preserves existing behavior)
                if total_occupied == 0 {
                    return true;
                }
                (same as f64) / (total_occupied as f64) >= threshold
            }
            SatisfactionRule::MinSame { min_same } => same >= min_same,
            SatisfactionRule::Bounded { min_same, max_same } => {
                same >= min_same && same <= max_same
            }
        }
    }

    /// Label for CLI and log output
    pub fn label(&self) -> String {
        match *self {
            SatisfactionRule::Ratio { threshold } => format!("ratio:{:.3}", threshold),
            SatisfactionRule::MinSame { min_same } => format!("min-same:{}", min_same),
            SatisfactionRule::Bounded { min_same, max_same } => {
                format!("bounded:{}:{}", min_same, max_same)
            }
        }
    }
}

/// Movement operation modes corresponding to the two operation forms distinguished in Schelling (1971) p.155.
///
/// - `Standard` : Loose operation. Only dissatisfied agents move (default for Fig.9–14).
/// - `Strict`   : Strict operation (Fig.8). In addition to dissatisfied agents, satisfied
///   agents also move speculatively to vacant cells that strictly improve the same-color ratio.
///   Segregation becomes greater than under loose operation because satisfied agents always seek more homogeneous neighborhoods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MoveMode {
    /// Loose operation: only dissatisfied agents move (existing behavior).
    #[default]
    Standard,
    /// Strict operation (Fig.8): satisfied agents also move if they can strictly improve the same-color ratio.
    Strict,
}

impl MoveMode {
    /// Parses a CLI string.
    pub fn parse(s: &str) -> Option<MoveMode> {
        match s {
            "standard" => Some(MoveMode::Standard),
            "strict" => Some(MoveMode::Strict),
            _ => None,
        }
    }

    /// Label for CLI and log output.
    pub fn label(self) -> &'static str {
        match self {
            MoveMode::Standard => "standard",
            MoveMode::Strict => "strict",
        }
    }
}

/// Destination-selection strategy that determines which satisfactory vacant cell a dissatisfied agent moves to.
///
/// - `Nearest`   : Moves to the first satisfactory vacant cell found at the shortest Chebyshev distance
///   (existing behavior). Default for Schelling's lattice diagrams Fig.7–14.
/// - `BestLocal` : Moves to the cell with the highest post-move same-color ratio among all satisfactory vacant cells.
///   Because the minority gathers in the "most homogeneous area," the minority cluster
///   ratio for unequal numbers (Fig.12) approaches the paper's value (>80%). Ties are resolved by
///   shortest distance, then row-major order (selecting the nearer, upper-left cell; deterministic).
///   The set of vacant cells searched is identical to Nearest; only the selection criterion differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MoveStrategy {
    /// First satisfactory vacant cell at the shortest distance (existing behavior).
    #[default]
    Nearest,
    /// Cell with the highest post-move same-color ratio in the nearest distance band (Fig.12 cluster improvement).
    BestLocal,
}

impl MoveStrategy {
    /// Parses a CLI string.
    pub fn parse(s: &str) -> Option<MoveStrategy> {
        match s {
            "nearest" => Some(MoveStrategy::Nearest),
            "best-local" => Some(MoveStrategy::BestLocal),
            _ => None,
        }
    }

    /// Label for CLI and log output.
    pub fn label(self) -> &'static str {
        match self {
            MoveStrategy::Nearest => "nearest",
            MoveStrategy::BestLocal => "best-local",
        }
    }
}

/// Simulation configuration
#[derive(Debug, Clone)]
pub struct Config {
    /// Number of grid rows
    pub rows: usize,
    /// Number of grid columns
    pub cols: usize,
    /// Number of agents in group A
    pub n_a: usize,
    /// Number of agents in group B
    pub n_b: usize,
    /// Satisfaction rule
    pub rule: SatisfactionRule,
    /// Movement operation mode (loose operation = Standard / strict operation = Strict, Fig.8)
    pub move_mode: MoveMode,
    /// Destination-selection strategy (Nearest = existing / BestLocal = Fig.12 cluster improvement)
    pub move_strategy: MoveStrategy,
    /// Maximum number of iterations
    pub max_iterations: usize,
    /// Random seed (random when None)
    pub seed: Option<u64>,
    /// Step interval for saving snapshots (0 = do not save)
    pub snapshot_interval: usize,
    /// Results output directory
    pub output_dir: String,
}

impl Default for Config {
    /// Standard configuration close to that in Schelling's paper (Figure 7--10)
    fn default() -> Self {
        // 13 rows by 16 columns = 208 cells, approximately 30% vacant → 146 agents total
        let rows = 13;
        let cols = 16;
        let total = rows * cols;
        let n_vacant = (total as f64 * 0.30).round() as usize;
        let n_agents = total - n_vacant;
        let n_a = n_agents / 2;
        let n_b = n_agents - n_a;

        Config {
            rows,
            cols,
            n_a,
            n_b,
            rule: SatisfactionRule::Ratio {
                threshold: 1.0 / 3.0,
            },
            move_mode: MoveMode::Standard,
            move_strategy: MoveStrategy::Nearest,
            max_iterations: 500,
            seed: Some(42),
            snapshot_interval: 1,
            output_dir: "results".to_string(),
        }
    }
}
