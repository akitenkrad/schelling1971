use serde::Serialize;

use crate::grid::Cell;
use crate::world::SchellingWorld;

/// Segregation metrics for one step.
#[derive(Debug, Clone, Serialize)]
pub struct Metrics {
    pub step: usize,
    /// Mean same-color neighbor ratio across all agents.
    pub avg_same_ratio: f64,
    /// Percentage of agents with no neighbors of another color.
    pub pct_no_opposite: f64,
    /// Dissimilarity index D = 0.5 * Σ |a_i/A - b_i/B|  (simplified with the entire grid as one zone)
    pub dissimilarity_index: f64,
    /// Number of dissatisfied agents.
    pub n_dissatisfied: usize,
    /// Number of agents that actually moved during this step.
    pub n_moved: usize,
    /// Mean same-color neighbor ratio for group A.
    pub avg_same_ratio_a: f64,
    /// Mean same-color neighbor ratio for group B.
    pub avg_same_ratio_b: f64,
}

impl Metrics {
    /// Calculates metrics from the current grid in the world state.
    pub fn compute(
        world: &SchellingWorld,
        step: usize,
        n_dissatisfied: usize,
        n_moved: usize,
    ) -> Self {
        let mut sum_a = 0.0;
        let mut sum_b = 0.0;
        let mut count_a = 0usize;
        let mut count_b = 0usize;
        let mut no_opp = 0usize;
        let mut total_agents = 0usize;

        // Reuse one neighbor buffer while scanning all occupied cells to eliminate heap allocation.
        // `neighbors_into` fills neighbors in the same order as `neighbors`, preserving ratio and
        // other-color checks.
        let mut buf: Vec<(usize, usize)> = Vec::new();

        for r in 0..world.rows() {
            for c in 0..world.cols() {
                let cell = world.cell_color(r, c);
                if cell == Cell::Empty {
                    continue;
                }
                total_agents += 1;
                let ratio = world.same_color_ratio_buf(r, c, &mut buf);

                match cell {
                    Cell::GroupA => {
                        sum_a += ratio;
                        count_a += 1;
                    }
                    Cell::GroupB => {
                        sum_b += ratio;
                        count_b += 1;
                    }
                    Cell::Empty => {}
                }

                // Check whether there are no neighbors of another color.
                if !world.has_opposite_neighbor_buf(r, c, &mut buf) {
                    no_opp += 1;
                }
            }
        }

        let avg_a = if count_a > 0 {
            sum_a / count_a as f64
        } else {
            0.0
        };
        let avg_b = if count_b > 0 {
            sum_b / count_b as f64
        } else {
            0.0
        };
        let avg_all = if total_agents > 0 {
            (sum_a + sum_b) / total_agents as f64
        } else {
            0.0
        };
        let pct_no_opp = if total_agents > 0 {
            no_opp as f64 / total_agents as f64 * 100.0
        } else {
            0.0
        };

        // Simplified dissimilarity index: treat the entire grid as one zone.
        // D = 0.5 * |a/A - b/B|  (complete segregation=0, approaches 1 for complete mixing)
        // Complement here: adjust the sign so that D increases with stronger segregation.
        let dissimilarity = if count_a > 0 && count_b > 0 {
            0.5 * ((count_a as f64 / total_agents as f64) - (count_b as f64 / total_agents as f64))
                .abs()
        } else {
            0.5
        };

        Metrics {
            step,
            avg_same_ratio: avg_all,
            pct_no_opposite: pct_no_opp,
            dissimilarity_index: dissimilarity,
            n_dissatisfied,
            n_moved,
            avg_same_ratio_a: avg_a,
            avg_same_ratio_b: avg_b,
        }
    }
}
