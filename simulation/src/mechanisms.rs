//! Schelling movement mechanism for the socsim framework.
//!
//! Implements the Schelling (1971) movement rule as a socsim [`Mechanism`]. It fires in the
//! `Decision` phase and processes each agent in the activation order supplied by
//! [`StepContext::agent_order`](socsim_core::StepContext), i.e. the order shuffled by the scheduler:
//!
//! 1. **Only agents dissatisfied at the start of the step** are eligible to move, preserving the
//!    previous implementation's semantics of collecting dissatisfied agents before processing.
//!    Agents made dissatisfied by another agent's move during the step do not move in that step.
//! 2. An eligible agent is skipped if it is already satisfied when processed, for example because
//!    another agent's move satisfied it.
//! 3. If dissatisfied, it searches vacant cells in ascending Chebyshev distance and moves to the
//!    first cell where it would be satisfied.
//!
//! This preserves `n_moved <= n_dissatisfied(at the start of the step)`.
//!
//! Destination selection uses no randomness (greedy nearest-neighbor search). Randomness comes
//! only from shuffling the activation order (`RandomActivationScheduler`). Although `agent_order`
//! shuffles all agents, filtering it by the set dissatisfied at the start produces an order
//! statistically equivalent to the previous implementation's direct shuffle of that set.
//!
//! Step results (number moved, number dissatisfied at the start, and convergence flag) are written
//! to [`StepContext::scratch`] and read by the driver through
//! [`Simulation::scratch`](socsim_engine::Simulation::scratch). On detecting convergence (none
//! dissatisfied at the start) or deadlock (`n_moved == 0`), the mechanism requests engine
//! termination through [`StepContext::request_stop`].

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use socsim_core::{AgentId, Mechanism, Phase, Result, StepContext};

use crate::config::MoveMode;
use crate::world::SchellingWorld;

/// Observer called once for each movement decision for one agent.
///
/// Time is spent on **individual movement decisions**, not steps. Because each dissatisfied agent
/// scans vacant cells in ascending Chebyshev distance, the cost of one decision grows with grid
/// area. Measurements: a 400x400 `run` took 70.6 seconds but only 8 steps to converge (8.8 seconds
/// per step). A 600x600 run did not finish after 20 minutes, with each step taking minutes. A
/// step-based counter therefore remains unchanged throughout that time.
///
/// This is shared rather than borrowed because the mechanism enters the engine as a
/// `Box<dyn Mechanism<_>>` (= `'static`) and therefore cannot borrow the caller's `Stage`.
pub type DecisionObserver = Rc<RefCell<dyn FnMut()>>;

/// Observer that counts nothing, for callers that do not report progress.
pub fn no_observer() -> DecisionObserver {
    Rc::new(RefCell::new(|| {}))
}

/// Mechanism that moves dissatisfied agents to the nearest satisfactory vacant cell.
pub struct SchellingMoveMechanism {
    /// Observer called for each movement decision.
    on_decision: DecisionObserver,
}

impl SchellingMoveMechanism {
    /// Constructs the mechanism with an observer. Pass [`no_observer`] when not reporting progress.
    pub fn new(on_decision: DecisionObserver) -> Self {
        Self { on_decision }
    }
}

impl Mechanism<SchellingWorld> for SchellingMoveMechanism {
    fn name(&self) -> &str {
        "schelling_move"
    }

    fn phases(&self) -> &'static [Phase] {
        &[Phase::Decision]
    }

    fn apply(&mut self, _phase: Phase, ctx: &mut StepContext<'_, SchellingWorld>) -> Result<()> {
        // Reusable neighbor-scan buffer, eliminating heap allocation from the satisfaction hot path.
        // `neighbors_into` fills neighbors in the same order as `neighbors`, so satisfaction checks,
        // destination selection, and therefore results are unchanged.
        let mut buf: Vec<(usize, usize)> = Vec::new();

        // Snapshot the agents dissatisfied at the start of the step.
        // Only members of this set may move during this step.
        let dissatisfied: HashSet<AgentId> = ctx
            .world
            .colors
            .keys()
            .copied()
            .filter(|id| {
                let (r, c) = ctx.world.index.position(*id).unwrap();
                !ctx.world.is_satisfied_buf(r, c, &mut buf)
            })
            .collect();

        let mut n_moved = 0usize;

        for id in ctx.agent_order {
            // Count each scanned agent as one decision first. Keep this at the top of the loop so
            // subsequent `continue` statements cannot skip the observation.
            (self.on_decision.borrow_mut())();

            // Agents satisfied at the start do not move during this step.
            if !dissatisfied.contains(id) {
                continue;
            }

            // Get the current position.
            let (r, c) = match ctx.world.index.position(*id) {
                Some(pos) => pos,
                None => continue, // Defensive fallback; the position should always exist.
            };

            // Skip agents already satisfied by another agent's move.
            if ctx.world.is_satisfied_buf(r, c, &mut buf) {
                continue;
            }

            // Search vacant cells nearest first and move to the first satisfactory one.
            if let Some(v) = ctx.world.nearest_satisfying_vacant((r, c)) {
                ctx.world
                    .index
                    .move_to(*id, v.0, v.1)
                    .expect("failed to move to vacant cell");
                n_moved += 1;
            }
        }

        // Strict mode (Fig.8): satisfied agents also move speculatively to vacant cells that strictly
        // improve their same-color ratio. Only agents satisfied at the start of the step are eligible;
        // those satisfied by a dissatisfied agent's move wait until the next step. Skip an agent if it
        // has become dissatisfied by the time it is processed (it may qualify for a standard move,
        // but not during this step).
        let mut n_speculative = 0usize;
        if ctx.world.move_mode == MoveMode::Strict {
            for id in ctx.agent_order {
                // Count each speculative scan as one decision as well. Strict mode makes roughly
                // twice as many decisions per step as standard mode.
                (self.on_decision.borrow_mut())();

                // Agents dissatisfied at the start are ineligible for speculation; handled above.
                if dissatisfied.contains(id) {
                    continue;
                }
                let (r, c) = match ctx.world.index.position(*id) {
                    Some(pos) => pos,
                    None => continue,
                };
                // If dissatisfied when processed, the agent needs a standard rather than speculative
                // move and therefore does not move during this step.
                if !ctx.world.is_satisfied_buf(r, c, &mut buf) {
                    continue;
                }
                if let Some(v) = ctx.world.best_speculative_vacant((r, c)) {
                    ctx.world
                        .index
                        .move_to(*id, v.0, v.1)
                        .expect("failed to move speculatively to vacant cell");
                    n_speculative += 1;
                }
            }
        }

        let total_moved = n_moved + n_speculative;
        let converged = dissatisfied.is_empty();

        // Write step results to scratch for the driver to read.
        // `n_moved` totals standard and speculative moves; speculative moves are always zero in
        // standard mode.
        ctx.scratch.insert("n_moved", total_moved);
        ctx.scratch.insert("n_dissatisfied", dissatisfied.len());
        ctx.scratch.insert("converged", converged);

        // Stopping conditions:
        // - Standard mode: convergence (none dissatisfied at the start) or deadlock (zero moves).
        //   Because speculative moves are always zero, `total_moved == 0` equals the former
        //   `n_moved == 0`, preserving behavior bit for bit.
        // - Strict mode: stop when neither dissatisfied nor speculative moves occur, a stable state
        //   where everyone is satisfied and no one can improve their same-color ratio.
        let should_stop = match ctx.world.move_mode {
            MoveMode::Standard => converged || total_moved == 0,
            MoveMode::Strict => converged && total_moved == 0,
        };
        if should_stop {
            ctx.request_stop();
        }

        Ok(())
    }
}
