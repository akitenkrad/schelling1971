//! Dynamics engine. Evolves the system on the $(W, B)$ phase plane and returns the trajectory and convergence destination.
//!
//! Provides two implementations:
//! - [`FlowModel::Continuous`]: Continuous-time ODE. The signs of $\dot W, \dot B$ are determined by the region classification,
//!   and their magnitudes are proportional to the flow coefficients $k_W, k_B$ and the distance from the current point to the target reaction curve. Discretized by the Euler method.
//! - [`FlowModel::DiscreteBatch`]: At each step, all excess agents exit and all available capacity is filled in one batch,
//!   a formulation close to the narrative description in the paper.

use serde::{Deserialize, Serialize};

use super::phase::{Equilibrium, EquilibriumKind, PhaseConfig};
use super::reaction::ReactionCurve;

/// Flow model.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FlowModel {
    /// Continuous-time Euler method. Flow rate is proportional to the "distance" from the current point to the reaction curve.
    /// For example, $\dot W = k_W \cdot \text{sign}(B_W(W) - B) \cdot |B_W(W) - B|^{0+}$.
    /// Simplification: use the distance itself as the flow rate, with $\dot W = k_W \cdot \text{sign}_W \cdot |\text{distance}|$.
    Continuous { k_w: f64, k_b: f64, dt: f64 },

    /// Discrete batch. At each step:
    /// - If $B > B_W(W)$, all excess W agents who can no longer tolerate the composition exit.
    /// - If $W \le W_B(B)$ and capacity is available, external W agents enter.
    /// - Symmetrically for B.
    DiscreteBatch,
}

impl Default for FlowModel {
    fn default() -> Self {
        FlowModel::Continuous {
            k_w: 1.0,
            k_b: 1.0,
            dt: 0.1,
        }
    }
}

/// Dynamics configuration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DynamicsConfig {
    pub flow: FlowModel,
    pub max_steps: usize,
    /// Convergence criterion: **speed** $\|(\dot W, \dot B)\|_\infty = \|(\Delta W, \Delta B)\|_\infty / dt
    /// < \text{convergence\_tol}$.
    ///
    /// Do not compare against the one-step displacement $\|(\Delta W, \Delta B)\|_\infty$ itself.
    /// Because displacement is proportional to $dt$, reducing $dt$ makes it more likely to fall below the threshold,
    /// so **refining the time step would incorrectly make the system appear "converged"**
    /// (for example, when running the degenerate fig20 case from $(68.67, 64.67)$, $dt = 0.1$
    /// correctly tips to the all-W endpoint, whereas $dt = 0.01$ was treated as converged after
    /// barely moving from the initial value). Using speed makes the criterion independent of $dt$.
    pub convergence_tol: f64,
}

impl Default for DynamicsConfig {
    fn default() -> Self {
        Self {
            flow: FlowModel::default(),
            max_steps: 5000,
            convergence_tol: 1e-4,
        }
    }
}

/// Trajectory: history of $(W, B)$ at each time point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trajectory {
    pub history: Vec<TrajectoryPoint>,
    pub converged: bool,
    pub converged_step: Option<usize>,
    pub final_equilibrium: Option<Equilibrium>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TrajectoryPoint {
    pub t: f64,
    pub w: f64,
    pub b: f64,
}

/// Integrates the dynamics from the initial value $(W_0, B_0)$.
pub fn integrate(phase: &PhaseConfig, cfg: &DynamicsConfig, init: (f64, f64)) -> Trajectory {
    integrate_observed(phase, cfg, init, |_| {})
}

/// Performs the same computation as [`integrate`] and calls `on_step` once per step.
///
/// The argument is the zero-based step number. `--max-steps` is an upper bound, not a destination
/// (the loop `break`s on convergence), so a bounded stage using it as the denominator gives a
/// confidently wrong remaining time. The caller should count using an unbounded stage.
pub fn integrate_observed(
    phase: &PhaseConfig,
    cfg: &DynamicsConfig,
    init: (f64, f64),
    mut on_step: impl FnMut(usize),
) -> Trajectory {
    let (mut w, mut b) = init;
    let w_max = phase.w_schedule.pop_max();
    let b_max = phase.b_schedule.pop_max();
    w = w.clamp(0.0, w_max);
    b = b.clamp(0.0, b_max);

    let mut history = Vec::with_capacity(cfg.max_steps + 1);
    let mut t = 0.0;
    history.push(TrajectoryPoint { t, w, b });

    let mut converged = false;
    let mut converged_step: Option<usize> = None;

    for step in 0..cfg.max_steps {
        let (dw, db, dt) = step_velocity(phase, cfg.flow, w, b);
        let w_next = (w + dw).clamp(0.0, w_max);
        let b_next = (b + db).clamp(0.0, b_max);

        // Capacity constraint. If exceeded, clip by proportional allocation.
        let (w_next, b_next) = if let Some(c) = phase.capacity {
            if w_next + b_next > c {
                let scale = c / (w_next + b_next);
                (w_next * scale, b_next * scale)
            } else {
                (w_next, b_next)
            }
        } else {
            (w_next, b_next)
        };

        let delta = (w_next - w).abs().max((b_next - b).abs());
        // Use speed rather than displacement for the criterion (to make it independent of $dt$; see [`DynamicsConfig`]).
        let speed = delta / dt;
        w = w_next;
        b = b_next;
        t += dt;
        history.push(TrajectoryPoint { t, w, b });
        on_step(step);

        if speed < cfg.convergence_tol {
            converged = true;
            converged_step = Some(step + 1);
            break;
        }
    }

    let final_equilibrium = nearest_equilibrium(phase, w, b);

    Trajectory {
        history,
        converged,
        converged_step,
        final_equilibrium,
    }
}

/// Returns $(\dot W \cdot dt, \dot B \cdot dt, dt)$ for one step.
///
/// Interpretation of the dynamics (Schelling 1971 §3):
/// Define a "satisfaction interval" $W \in [W_{lower}(B), W_{upper}(B)]$ on the reaction curve $B_W(W)$.
/// Within the interval, the most tolerant external W agents enter, approaching the upper bound $W_{upper}$.
/// Outside the interval (below = too few agents and too high an out-group ratio / above = too many agents and a tolerance-limit violation), agents exit:
/// - $W < W_{lower}$: move toward 0 (W cannot increase enough to achieve satisfaction / far too few).
/// - $W > W_{upper}$: move toward $W_{upper}$ (the excess exits).
/// - If $B$ exceeds the peak of the reaction curve: all W agents exit → 0.
///
/// This formulation yields: (i) stable convergence to endpoint equilibria by asymptotic approach, (ii) smooth convergence
/// to reaction-curve intersections (mixed equilibria) from both directions without chattering, and (iii) saddle points that are unstable under linearization.
fn step_velocity(phase: &PhaseConfig, flow: FlowModel, w: f64, b: f64) -> (f64, f64, f64) {
    let w_pop_max = phase.w_schedule.pop_max();
    let b_pop_max = phase.b_schedule.pop_max();
    let w_target = directional_target(&phase.w_reaction(), b, w, w_pop_max);
    let b_target = directional_target(&phase.b_reaction(), w, b, b_pop_max);

    match flow {
        FlowModel::Continuous { k_w, k_b, dt } => {
            let dw = k_w * (w_target - w) * dt;
            let db = k_b * (b_target - b) * dt;
            (dw, db, dt)
        }
        FlowModel::DiscreteBatch => {
            // Jump directly to the target value in one step.
            (w_target - w, b_target - b, 1.0)
        }
    }
}

/// Target of the dynamics.
/// The destination is determined by the relationship between `own_now` and the "satisfaction interval" $[W_{lower}, W_{upper}]$.
fn directional_target(rc: &ReactionCurve, other_now: f64, own_now: f64, own_pop_max: f64) -> f64 {
    // If other_now <= 0, there is no constraint → move toward the full population.
    if other_now <= 0.0 {
        return own_pop_max;
    }
    let (w_peak, b_peak) = rc.peak();
    if other_now > b_peak {
        // Out-group population above the reaction-curve peak → no W level can be satisfactory → all exit.
        return 0.0;
    }
    let upper = upper_root(rc, other_now, own_pop_max, w_peak);
    let lower = lower_root(rc, other_now, w_peak);
    if own_now < lower {
        // Below the satisfaction interval → too few / overcrowded ratio makes everyone dissatisfied → move toward 0.
        0.0
    } else {
        // Within or above the interval → move toward the upper bound W_upper.
        // Within the interval: entry increases the population to W_upper.
        // Above the interval: excess agents exit until the population returns to W_upper.
        upper
    }
}

/// Upper root satisfying the reaction curve $rc(W) = \text{target}$ (the solution to the right of the peak).
fn upper_root(rc: &ReactionCurve, target: f64, pop_max: f64, w_peak: f64) -> f64 {
    let mut lo = w_peak;
    let mut hi = pop_max;
    if rc.max_other(hi) >= target {
        return hi;
    }
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if rc.max_other(mid) >= target {
            lo = mid;
        } else {
            hi = mid;
        }
        if (hi - lo) < 1e-9 * pop_max.max(1.0) {
            break;
        }
    }
    lo
}

/// Lower root satisfying the reaction curve $rc(W) = \text{target}$ (the solution to the left of the peak).
fn lower_root(rc: &ReactionCurve, target: f64, w_peak: f64) -> f64 {
    let mut lo = 0.0;
    let mut hi = w_peak;
    if rc.max_other(lo) >= target {
        return lo;
    }
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if rc.max_other(mid) >= target {
            hi = mid;
        } else {
            lo = mid;
        }
        if (hi - lo) < 1e-9 * w_peak.max(1.0) {
            break;
        }
    }
    hi
}

/// Returns the equilibrium nearest to the endpoint.
fn nearest_equilibrium(phase: &PhaseConfig, w: f64, b: f64) -> Option<Equilibrium> {
    let eqs = phase.equilibria();
    let scale = (phase.w_schedule.pop_max() + phase.b_schedule.pop_max()).max(1.0);
    eqs.into_iter()
        .map(|e| {
            let d2 = (e.w - w).powi(2) + (e.b - b).powi(2);
            (d2, e)
        })
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
        .filter(|(d2, _)| d2.sqrt() < 0.05 * scale) // Within 5%.
        .map(|(_, e)| e)
}

/// Sweeps a grid of initial conditions to construct a basin-of-attraction map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasinSample {
    pub w0: f64,
    pub b0: f64,
    pub final_w: f64,
    pub final_b: f64,
    pub converged: bool,
    pub converged_kind: Option<EquilibriumKind>,
    pub steps: usize,
}

// Unobserved entry point used by tests (because this is a binary crate, it appears unused to `cargo build`).
#[allow(dead_code)]
pub fn basin_of_attraction(
    phase: &PhaseConfig,
    cfg: &DynamicsConfig,
    n_w: usize,
    n_b: usize,
) -> Vec<BasinSample> {
    basin_of_attraction_observed(phase, cfg, n_w, n_b, || {})
}

/// Performs the same computation as [`basin_of_attraction`] and calls `on_sample` once
/// for each initial condition.
///
/// It is called even for points outside the capacity constraint that were not integrated—the count
/// represents attempted conditions. This is why a bounded stage using the total number of grid points
/// `(n_w + 1) * (n_b + 1)` as its denominator closes at exactly 100%.
pub fn basin_of_attraction_observed(
    phase: &PhaseConfig,
    cfg: &DynamicsConfig,
    n_w: usize,
    n_b: usize,
    mut on_sample: impl FnMut(),
) -> Vec<BasinSample> {
    let w_max = phase.w_schedule.pop_max();
    let b_max = phase.b_schedule.pop_max();
    let mut out = Vec::with_capacity((n_w + 1) * (n_b + 1));
    for i in 0..=n_w {
        for j in 0..=n_b {
            let w0 = w_max * (i as f64) / (n_w as f64);
            let b0 = b_max * (j as f64) / (n_b as f64);
            // Keep rejection inside the if block so `continue` does not skip observation.
            if phase.within_capacity(w0, b0) {
                let traj = integrate(phase, cfg, (w0, b0));
                let last = traj.history.last().copied().unwrap_or(TrajectoryPoint {
                    t: 0.0,
                    w: w0,
                    b: b0,
                });
                out.push(BasinSample {
                    w0,
                    b0,
                    final_w: last.w,
                    final_b: last.b,
                    converged: traj.converged,
                    converged_kind: traj.final_equilibrium.map(|e| e.kind),
                    steps: traj.converged_step.unwrap_or(cfg.max_steps),
                });
            }
            on_sample();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytic::tolerance::ToleranceSchedule;

    fn fig18_phase() -> PhaseConfig {
        PhaseConfig {
            w_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 100.0,
            },
            b_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 50.0,
            },
            capacity: None,
        }
    }

    /// Fig.18: Initial state (90, 5) should converge to the all-W endpoint (B is an overwhelming minority).
    #[test]
    fn fig18_high_white_converges_to_all_white() {
        let phase = fig18_phase();
        let cfg = DynamicsConfig::default();
        let traj = integrate(&phase, &cfg, (90.0, 5.0));
        assert!(traj.converged, "trajectory should converge");
        let last = traj.history.last().unwrap();
        assert!(last.w > 80.0, "reaches the all-W endpoint: w={}", last.w);
        assert!(last.b < 5.0, "B is nearly zero: b={}", last.b);
        assert_eq!(
            traj.final_equilibrium.map(|e| e.kind),
            Some(EquilibriumKind::AllWhite)
        );
    }

    /// Fig.18: Initial state (5, 40) should converge to the all-B endpoint (W is an overwhelming minority).
    #[test]
    fn fig18_high_black_converges_to_all_black() {
        let phase = fig18_phase();
        let cfg = DynamicsConfig::default();
        let traj = integrate(&phase, &cfg, (5.0, 40.0));
        assert!(traj.converged);
        let last = traj.history.last().unwrap();
        assert!(last.b > 40.0, "reaches the all-B endpoint: b={}", last.b);
        assert!(last.w < 5.0, "W is nearly zero: w={}", last.w);
        assert_eq!(
            traj.final_equilibrium.map(|e| e.kind),
            Some(EquilibriumKind::AllBlack)
        );
    }

    /// Symmetric linear case: changing the initial conditions leads to one endpoint or the other,
    /// while the linear form exhibits Schelling's "mixing is statically possible but dynamically unstable" result.
    #[test]
    fn discrete_batch_converges_in_few_steps() {
        let phase = fig18_phase();
        let cfg = DynamicsConfig {
            flow: FlowModel::DiscreteBatch,
            max_steps: 50,
            convergence_tol: 1e-3,
        };
        let traj = integrate(&phase, &cfg, (50.0, 25.0));
        // The batch model should reach an endpoint in one to several steps.
        assert!(traj.history.len() <= 10);
    }

    /// Case with a capacity constraint: capacity is never exceeded.
    #[test]
    fn capacity_constraint_respected() {
        let phase = PhaseConfig {
            w_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 100.0,
            },
            b_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 100.0,
            },
            capacity: Some(120.0),
        };
        let cfg = DynamicsConfig::default();
        let traj = integrate(&phase, &cfg, (60.0, 50.0));
        for p in &traj.history {
            assert!(
                p.w + p.b <= 120.0 + 1e-6,
                "capacity exceeded: w+b={}",
                p.w + p.b
            );
        }
    }

    /// The convergence criterion must not depend on the time step $dt$.
    ///
    /// If convergence is judged by one-step displacement, the displacement's proportionality to $dt$
    /// causes smaller $dt$ values to be incorrectly classified as converged (refining the time step
    /// makes the result worse). The degenerate case (equivalent to fig20, $R_{\max} = 3$) diverges slowly
    /// due to the cubic term and exposes this bug most clearly, so it is used as a regression test.
    #[test]
    fn convergence_verdict_is_independent_of_dt() {
        let phase = PhaseConfig {
            w_schedule: ToleranceSchedule::Linear {
                r_max: 3.0,
                pop_max: 100.0,
            },
            b_schedule: ToleranceSchedule::Linear {
                r_max: 3.0,
                pop_max: 100.0,
            },
            capacity: None,
        };
        let w_star = 200.0 / 3.0;
        let init = (w_star + 2.0, w_star - 2.0); // Asymmetric perturbation with v = 4.

        for dt in [0.1, 0.01, 0.001] {
            let cfg = DynamicsConfig {
                flow: FlowModel::Continuous {
                    k_w: 1.0,
                    k_b: 1.0,
                    dt,
                },
                max_steps: 20_000_000,
                convergence_tol: 1e-4,
            };
            let traj = integrate(&phase, &cfg, init);
            let last = traj.history.last().unwrap();
            assert!(
                last.w > 95.0 && last.b < 5.0,
                "tips to the all-W endpoint even with dt={dt}: ({}, {})",
                last.w,
                last.b
            );
        }
    }

    /// Basin of attraction: the four corner samples are classified correctly.
    #[test]
    fn basin_sample_identifies_endpoints() {
        let phase = fig18_phase();
        let cfg = DynamicsConfig::default();
        let basin = basin_of_attraction(&phase, &cfg, 4, 4);
        assert!(!basin.is_empty());
        // At least one point converges to AllWhite and one to AllBlack.
        let has_white = basin
            .iter()
            .any(|s| s.converged_kind == Some(EquilibriumKind::AllWhite));
        let has_black = basin
            .iter()
            .any(|s| s.converged_kind == Some(EquilibriumKind::AllBlack));
        assert!(
            has_white && has_black,
            "basins of attraction for both endpoints should be observed"
        );
    }
}
