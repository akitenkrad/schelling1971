//! World state for the Schelling segregation model on the socsim framework.
//!
//! `SchellingWorld` implements socsim's [`WorldState`], storing spatial occupancy in
//! [`socsim_grid::GridIndex`] and each agent's group (color) in the `colors` map. Vacant cells
//! appear in neither the occupancy map nor `colors`.
//!
//! The previous implementation's handwritten neighborhood calculations (moore_neighbors,
//! vacant_cells, chebyshev, etc.) are replaced by socsim-grid's [`Grid`] and [`GridIndex`]. This
//! module implements only Schelling-specific decisions (satisfaction, same-color ratio, and
//! destination search) as domain helpers.

use std::collections::BTreeMap;

use socsim_core::{AgentId, SimClock, WorldState};
use socsim_grid::{GridIndex, Metric, Neighborhood};

use crate::config::{MoveMode, MoveStrategy, SatisfactionRule};
use crate::grid::Cell;

/// World state for the Schelling segregation model.
pub struct SchellingWorld {
    /// Simulation clock.
    pub clock: SimClock,
    /// Spatial occupancy index, the source of truth for neighbor and vacant-cell searches.
    pub index: GridIndex,
    /// Each agent's group (color). Vacant cells are absent; keys are occupied agents only.
    pub colors: BTreeMap<AgentId, Cell>,
    /// Satisfaction rule.
    pub rule: SatisfactionRule,
    /// Movement mode (standard / strict Fig.8).
    pub move_mode: MoveMode,
    /// Destination-selection strategy (Nearest / BestLocal).
    pub move_strategy: MoveStrategy,
}

impl SchellingWorld {
    /// Constructs world state from a grid index and color map (standard mode, Nearest strategy).
    #[allow(dead_code)]
    pub fn new(
        index: GridIndex,
        colors: BTreeMap<AgentId, Cell>,
        rule: SatisfactionRule,
        t_max: u64,
    ) -> Self {
        Self::with_modes(
            index,
            colors,
            rule,
            MoveMode::Standard,
            MoveStrategy::Nearest,
            t_max,
        )
    }

    /// Constructs world state with an explicit movement mode (Nearest strategy).
    #[allow(dead_code)]
    pub fn with_move_mode(
        index: GridIndex,
        colors: BTreeMap<AgentId, Cell>,
        rule: SatisfactionRule,
        move_mode: MoveMode,
        t_max: u64,
    ) -> Self {
        Self::with_modes(index, colors, rule, move_mode, MoveStrategy::Nearest, t_max)
    }

    /// Constructs world state with explicit movement mode and destination strategy.
    pub fn with_modes(
        index: GridIndex,
        colors: BTreeMap<AgentId, Cell>,
        rule: SatisfactionRule,
        move_mode: MoveMode,
        move_strategy: MoveStrategy,
        t_max: u64,
    ) -> Self {
        SchellingWorld {
            clock: SimClock::new(t_max),
            index,
            colors,
            rule,
            move_mode,
            move_strategy,
        }
    }

    /// Number of grid rows.
    pub fn rows(&self) -> usize {
        self.index.grid().rows()
    }

    /// Number of grid columns.
    pub fn cols(&self) -> usize {
        self.index.grid().cols()
    }

    /// Returns the color of the agent at `(r, c)`, or `Cell::Empty` for a vacant cell.
    pub fn cell_color(&self, r: usize, c: usize) -> Cell {
        match self.index.occupant(r, c) {
            Some(id) => self.colors[&id],
            None => Cell::Empty,
        }
    }

    /// Returns (same-color occupied neighbors, occupied neighbors) for `(r, c)`.
    /// Returns (0, 0) for a vacant cell.
    ///
    /// Reuses the caller-owned buffer `buf` for neighbor scans to avoid heap allocation on every
    /// call (`neighbors_into` fills neighbors in the same order as `neighbors`).
    pub fn neighbor_counts_buf(
        &self,
        r: usize,
        c: usize,
        buf: &mut Vec<(usize, usize)>,
    ) -> (usize, usize) {
        let agent = self.cell_color(r, c);
        if agent == Cell::Empty {
            return (0, 0);
        }
        let mut same = 0usize;
        let mut total = 0usize;
        self.index
            .grid()
            .neighbors_into(r, c, Neighborhood::Moore, buf);
        for &(nr, nc) in buf.iter() {
            if let Some(id) = self.index.occupant(nr, nc) {
                total += 1;
                if self.colors[&id] == agent {
                    same += 1;
                }
            }
        }
        (same, total)
    }

    /// Calculates the same-color neighbor ratio for a cell, reusing neighbor-scan buffer `buf`.
    /// Returns 1.0 (satisfied) when there are no occupied neighbors, preserving the previous
    /// implementation's convention.
    pub fn same_color_ratio_buf(&self, r: usize, c: usize, buf: &mut Vec<(usize, usize)>) -> f64 {
        let (same, total) = self.neighbor_counts_buf(r, c, buf);
        if total == 0 {
            return 1.0;
        }
        same as f64 / total as f64
    }

    /// Determines whether an agent satisfies the rule, reusing neighbor-scan buffer `buf`.
    pub fn is_satisfied_buf(&self, r: usize, c: usize, buf: &mut Vec<(usize, usize)>) -> bool {
        if self.cell_color(r, c) == Cell::Empty {
            return true;
        }
        let (same, total) = self.neighbor_counts_buf(r, c, buf);
        self.rule.evaluate(same, total)
    }

    /// Determines whether `(r, c)` has an occupied neighbor of another color, for metrics, reusing
    /// neighbor-scan buffer `buf`.
    pub fn has_opposite_neighbor_buf(
        &self,
        r: usize,
        c: usize,
        buf: &mut Vec<(usize, usize)>,
    ) -> bool {
        let agent = self.cell_color(r, c);
        if agent == Cell::Empty {
            return false;
        }
        self.index
            .grid()
            .neighbors_into(r, c, Neighborhood::Moore, buf);
        buf.iter()
            .any(|&(nr, nc)| match self.index.occupant(nr, nc) {
                Some(id) => self.colors[&id] != agent,
                None => false,
            })
    }

    /// Determines whether an agent would be satisfied after moving from `from` to `to`, reusing
    /// neighbor-scan buffer `buf`.
    ///
    /// When counting neighbors of `to`, the source cell `from` is not considered occupied because
    /// the agent leaves it. The moving agent uses the current color at `from`. `neighbors_into`
    /// fills neighbors in the same order as `neighbors`, preserving occupancy and color-comparison
    /// order.
    pub fn will_be_satisfied_after_move_buf(
        &self,
        from: (usize, usize),
        to: (usize, usize),
        buf: &mut Vec<(usize, usize)>,
    ) -> bool {
        let agent = self.cell_color(from.0, from.1);
        let mut same = 0usize;
        let mut total = 0usize;
        self.index
            .grid()
            .neighbors_into(to.0, to.1, Neighborhood::Moore, buf);
        for &(nr, nc) in buf.iter() {
            if (nr, nc) == from {
                continue; // The original position becomes vacant.
            }
            if let Some(id) = self.index.occupant(nr, nc) {
                total += 1;
                if self.colors[&id] == agent {
                    same += 1;
                }
            }
        }
        self.rule.evaluate(same, total)
    }

    /// Scans vacant cells from `from` in nearest-first (Chebyshev-distance) order and returns the
    /// first cell where the agent would be satisfied after moving.
    ///
    /// Starting from the row-major order returned by [`GridIndex::vacant_cells`], vacant cells are
    /// **stably sorted** by Chebyshev distance (integer distances preserve row-major order among
    /// ties). This matches the previous implementation's `sort_by_key(chebyshev)` behavior.
    ///
    /// Because the innermost scan (satisfaction testing for each vacant cell) is concentrated here,
    /// one neighbor buffer is allocated and reused across all candidates, eliminating per-candidate
    /// `neighbors` heap allocation. The scanned cells, their order, and RNG draws are unchanged.
    pub fn nearest_satisfying_vacant(&self, from: (usize, usize)) -> Option<(usize, usize)> {
        let mut vacants = self.index.vacant_cells();
        vacants.sort_by(|&a, &b| {
            let da = self.index.grid().distance(Metric::Chebyshev, from, a);
            let db = self.index.grid().distance(Metric::Chebyshev, from, b);
            da.partial_cmp(&db).unwrap()
        });
        let mut buf = Vec::new();
        match self.move_strategy {
            // Existing behavior: the nearest first satisfactory vacant cell.
            MoveStrategy::Nearest => vacants
                .into_iter()
                .find(|&v| self.will_be_satisfied_after_move_buf(from, v, &mut buf)),
            // BestLocal: among all satisfactory vacant cells, move to the one with the highest
            // same-color ratio after the move. This corresponds to minorities clustering in the
            // "most homogeneous area" in Schelling's manual simulation (Fig.12). Return None if
            // there is no candidate. Equal ratios are resolved first by ascending distance, then
            // row-major order inherited from the stable sort (choosing the nearer, then upper-left
            // cell deterministically). The set of vacant cells searched is identical to Nearest;
            // only its ascending-distance order has already been fixed.
            MoveStrategy::BestLocal => {
                let mut best: Option<((usize, usize), f64)> = None;
                for v in vacants {
                    if !self.will_be_satisfied_after_move_buf(from, v, &mut buf) {
                        continue;
                    }
                    let ratio = self.ratio_after_move_buf(from, v, &mut buf);
                    match best {
                        Some((_, br)) if ratio <= br => {}
                        _ => best = Some((v, ratio)),
                    }
                }
                best.map(|(v, _)| v)
            }
        }
    }

    /// Returns the same-color neighbor ratio if the agent at `from` were moved to `to`.
    ///
    /// When counting neighbors of `to`, the source cell `from` is not considered occupied because
    /// the agent leaves it. Returns 1.0 (satisfied) when there are no occupied neighbors, consistent
    /// with the convention of `same_color_ratio_buf`.
    pub fn ratio_after_move_buf(
        &self,
        from: (usize, usize),
        to: (usize, usize),
        buf: &mut Vec<(usize, usize)>,
    ) -> f64 {
        let agent = self.cell_color(from.0, from.1);
        let mut same = 0usize;
        let mut total = 0usize;
        self.index
            .grid()
            .neighbors_into(to.0, to.1, Neighborhood::Moore, buf);
        for &(nr, nc) in buf.iter() {
            if (nr, nc) == from {
                continue; // The original position becomes vacant.
            }
            if let Some(id) = self.index.occupant(nr, nc) {
                total += 1;
                if self.colors[&id] == agent {
                    same += 1;
                }
            }
        }
        if total == 0 {
            return 1.0;
        }
        same as f64 / total as f64
    }

    /// Returns a speculative destination for strict mode (Fig.8).
    ///
    /// Finds the nearest vacant cell that strictly exceeds the current same-color ratio at `from`
    /// while keeping the agent satisfied after the move. Returns `None` if there is no candidate.
    /// As in `nearest_satisfying_vacant`, cells are stably sorted by Chebyshev distance before the
    /// scan, so the first row-major candidate is chosen among equal-distance cells (deterministic).
    ///
    /// Requiring a "strict improvement" prevents lateral moves with unchanged ratios, so the number
    /// of speculative moves monotonically plateaus across steps, preventing oscillation and
    /// infinite loops.
    pub fn best_speculative_vacant(&self, from: (usize, usize)) -> Option<(usize, usize)> {
        let mut buf = Vec::new();
        let current = self.same_color_ratio_buf(from.0, from.1, &mut buf);
        let mut vacants = self.index.vacant_cells();
        vacants.sort_by(|&a, &b| {
            let da = self.index.grid().distance(Metric::Chebyshev, from, a);
            let db = self.index.grid().distance(Metric::Chebyshev, from, b);
            da.partial_cmp(&db).unwrap()
        });
        const EPS: f64 = 1e-9;
        vacants.into_iter().find(|&v| {
            let after = self.ratio_after_move_buf(from, v, &mut buf);
            after > current + EPS && self.will_be_satisfied_after_move_buf(from, v, &mut buf)
        })
    }
}

impl WorldState for SchellingWorld {
    fn agent_ids(&self) -> Vec<AgentId> {
        // BTreeMap keys are ascending, satisfying the sort order required for determinism.
        self.colors.keys().copied().collect()
    }

    fn clock(&self) -> &SimClock {
        &self.clock
    }

    fn clock_mut(&mut self) -> &mut SimClock {
        &mut self.clock
    }
}
