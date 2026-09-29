//! Tipping model (Schelling 1971 §4, pp.181--186).
//!
//! Applies the bounded-neighborhood model (BNM) to the housing market. Adds the following to the basic BNM dynamics:
//! - **Speculative exit (Speculation)**: Expectation-based early exit, allowing agents to exit based on forecasts even when the current ratio is tolerable.
//! - **Flow-rate asymmetry (FlowAsymmetry)**: Entry and exit rates differ by group and direction.
//! - **Channeling (channeling)**: Represents the concentration of tipping in small neighborhoods with clear boundaries
//!   as a reduction in effective capacity.
//! - **Tipping-type classification (TippingType)**: Classifies cases into four types based on the presence of in-tipping / out-tipping.

use serde::{Deserialize, Serialize};

use super::dynamics::{integrate_observed, DynamicsConfig, FlowModel, Trajectory, TrajectoryPoint};
use super::phase::{EquilibriumKind, PhaseConfig};
use super::reaction::ReactionCurve;

// ---------------------------------------------------------------------------
// Speculative exit
// ---------------------------------------------------------------------------

/// Expectation-formation model.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Speculation {
    /// No speculation. Tolerance is judged using only the current ratio.
    #[default]
    None,
    /// Linear extrapolation: expected $B_t^e = B_t + \alpha \cdot \dot B_{t-1}$.
    /// Uses $B_t^e$ instead of $B_t$ for the tolerance judgment, allowing early exit when future deterioration is forecast.
    Linear { alpha: f64 },
    /// Extrapolates by linear regression over the previous window steps. weight is in 0..=1,
    /// $B_t^e = B_t + \text{weight} \cdot \text{trend}$.
    Trend { window: usize, weight: f64 },
}

// ---------------------------------------------------------------------------
// Flow-rate asymmetry
// ---------------------------------------------------------------------------

/// Specifies entry and exit rates by group and direction.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FlowAsymmetry {
    pub w_inflow: f64,
    pub w_outflow: f64,
    pub b_inflow: f64,
    pub b_outflow: f64,
}

impl Default for FlowAsymmetry {
    fn default() -> Self {
        FlowAsymmetry {
            w_inflow: 1.0,
            w_outflow: 1.0,
            b_inflow: 1.0,
            b_outflow: 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Tipping configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TippingConfig {
    pub phase: PhaseConfig,
    pub dynamics: DynamicsConfig,
    pub speculation: Speculation,
    pub asymmetry: Option<FlowAsymmetry>,
    /// Reduces effective capacity to `phase.capacity * channeling`.
    /// Disabled by None or 1.0. Smaller values represent clearer neighborhood boundaries and make tipping more likely.
    pub channeling: Option<f64>,
}

impl TippingConfig {
    /// Integrates the trajectory. The speculation term and asymmetry are handled by a dedicated routine rather than dynamics::integrate.
    ///
    /// Unobserved entry point used by tests (because this is a binary crate, it appears unused to
    /// `cargo build`).
    #[allow(dead_code)]
    pub fn integrate(&self, init: (f64, f64)) -> Trajectory {
        self.integrate_observed(init, |_| {})
    }

    /// Performs the same computation as [`TippingConfig::integrate`] and calls `on_step`
    /// once per step. The argument is the zero-based step number.
    ///
    /// The path without extensions (no speculation or asymmetry) is also delegated to BNM's
    /// [`integrate_observed`], so both paths count at the same granularity.
    pub fn integrate_observed(&self, init: (f64, f64), on_step: impl FnMut(usize)) -> Trajectory {
        // Currently, fall back to BNM integration when speculation and asymmetry are unspecified.
        // Use the capacity constraint after applying channeling.
        let mut phase = self.phase.clone();
        if let Some(c) = self.channeling {
            if (0.0..=1.0).contains(&c) {
                phase.capacity = phase.capacity.map(|cap| cap * c);
            }
        }

        match (self.speculation, self.asymmetry) {
            (Speculation::None, None) => integrate_observed(&phase, &self.dynamics, init, on_step),
            _ => self.integrate_with_extensions(&phase, init, on_step),
        }
    }

    fn integrate_with_extensions(
        &self,
        phase: &PhaseConfig,
        init: (f64, f64),
        mut on_step: impl FnMut(usize),
    ) -> Trajectory {
        let (mut w, mut b) = init;
        let w_max = phase.w_schedule.pop_max();
        let b_max = phase.b_schedule.pop_max();
        w = w.clamp(0.0, w_max);
        b = b.clamp(0.0, b_max);

        let dt = match self.dynamics.flow {
            FlowModel::Continuous { dt, .. } => dt,
            FlowModel::DiscreteBatch => 1.0,
        };
        let (k_w, k_b) = match self.dynamics.flow {
            FlowModel::Continuous { k_w, k_b, .. } => (k_w, k_b),
            FlowModel::DiscreteBatch => (1.0, 1.0),
        };

        let mut history = Vec::with_capacity(self.dynamics.max_steps + 1);
        let mut t = 0.0;
        history.push(TrajectoryPoint { t, w, b });

        let mut converged = false;
        let mut converged_step: Option<usize> = None;

        // Past history for expectation formation (for the window).
        let mut prev_w = w;
        let mut prev_b = b;

        for step in 0..self.dynamics.max_steps {
            // Expected values (after applying the speculation model).
            let (b_eff, w_eff) = self.apply_speculation(&history, prev_w, prev_b, w, b);

            // Reaction-curve targets. Evaluate the absent group using its effective value.
            let w_target = directional_target(&phase.w_reaction(), b_eff, w, w_max);
            let b_target = directional_target(&phase.b_reaction(), w_eff, b, b_max);

            // Flow rates (asymmetry).
            let asym = self.asymmetry.unwrap_or_default();
            let w_rate = if w_target >= w {
                k_w * asym.w_inflow * (w_target - w)
            } else {
                k_w * asym.w_outflow * (w_target - w) // Negative value.
            };
            let b_rate = if b_target >= b {
                k_b * asym.b_inflow * (b_target - b)
            } else {
                k_b * asym.b_outflow * (b_target - b)
            };

            let dw = w_rate * dt;
            let db = b_rate * dt;
            let w_next = (w + dw).clamp(0.0, w_max);
            let b_next = (b + db).clamp(0.0, b_max);

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
            // Use speed rather than displacement for the criterion (to make it independent of $dt$;
            // see [`super::dynamics::DynamicsConfig::convergence_tol`]).
            let speed = delta / dt;
            prev_w = w;
            prev_b = b;
            w = w_next;
            b = b_next;
            t += dt;
            history.push(TrajectoryPoint { t, w, b });
            on_step(step);

            if speed < self.dynamics.convergence_tol {
                converged = true;
                converged_step = Some(step + 1);
                break;
            }
        }

        // Equilibrium nearest to the endpoint (simple reimplementation because the dynamics helper is private).
        let final_eq = phase
            .equilibria()
            .into_iter()
            .map(|e| {
                let d2 = (e.w - w).powi(2) + (e.b - b).powi(2);
                (d2, e)
            })
            .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
            .filter(|(d2, _)| {
                let scale = (phase.w_schedule.pop_max() + phase.b_schedule.pop_max()).max(1.0);
                d2.sqrt() < 0.05 * scale
            })
            .map(|(_, e)| e);

        Trajectory {
            history,
            converged,
            converged_step,
            final_equilibrium: final_eq,
        }
    }

    /// Computes the "effective value of the other group used for the tolerance judgment" through the speculation term.
    /// Returns (effective B value, effective W value). In the ordinary BNM, these equal (B, W).
    fn apply_speculation(
        &self,
        history: &[TrajectoryPoint],
        prev_w: f64,
        prev_b: f64,
        w: f64,
        b: f64,
    ) -> (f64, f64) {
        match self.speculation {
            Speculation::None => (b, w),
            Speculation::Linear { alpha } => {
                let dw = w - prev_w;
                let db = b - prev_b;
                let b_eff = b + alpha * db;
                let w_eff = w + alpha * dw;
                (b_eff, w_eff)
            }
            Speculation::Trend { window, weight } => {
                if history.len() < window.max(2) {
                    return (b, w);
                }
                let recent = &history[history.len() - window..];
                let n = recent.len() as f64;
                let mean_t: f64 = recent.iter().map(|p| p.t).sum::<f64>() / n;
                let mean_w: f64 = recent.iter().map(|p| p.w).sum::<f64>() / n;
                let mean_b: f64 = recent.iter().map(|p| p.b).sum::<f64>() / n;
                let denom: f64 = recent.iter().map(|p| (p.t - mean_t).powi(2)).sum();
                if denom < 1e-12 {
                    return (b, w);
                }
                let slope_w: f64 = recent
                    .iter()
                    .map(|p| (p.t - mean_t) * (p.w - mean_w))
                    .sum::<f64>()
                    / denom;
                let slope_b: f64 = recent
                    .iter()
                    .map(|p| (p.t - mean_t) * (p.b - mean_b))
                    .sum::<f64>()
                    / denom;
                let b_eff = b + weight * slope_b;
                let w_eff = w + weight * slope_w;
                (b_eff, w_eff)
            }
        }
    }
}

// Provide a public wrapper for reuse here because dynamics::directional_target is private.
fn directional_target(rc: &ReactionCurve, other_now: f64, own_now: f64, own_pop_max: f64) -> f64 {
    if other_now <= 0.0 {
        return own_pop_max;
    }
    let (w_peak, b_peak) = rc.peak();
    if other_now > b_peak {
        return 0.0;
    }
    // Upper root
    let mut lo = w_peak;
    let mut hi = own_pop_max;
    let upper = if rc.max_other(hi) >= other_now {
        hi
    } else {
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if rc.max_other(mid) >= other_now {
                lo = mid;
            } else {
                hi = mid;
            }
            if (hi - lo) < 1e-9 * own_pop_max.max(1.0) {
                break;
            }
        }
        lo
    };
    // Lower root
    let mut lo = 0.0;
    let mut hi = w_peak;
    let lower = if rc.max_other(lo) >= other_now {
        lo
    } else {
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if rc.max_other(mid) >= other_now {
                hi = mid;
            } else {
                lo = mid;
            }
            if (hi - lo) < 1e-9 * w_peak.max(1.0) {
                break;
            }
        }
        hi
    };
    if own_now < lower {
        0.0
    } else {
        upper
    }
}

// ---------------------------------------------------------------------------
// Tipping-type classification
// ---------------------------------------------------------------------------

/// Tipping types (the four types in Schelling Fig.30--32).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TippingType {
    /// In-tipping only: the all-W endpoint is unstable, and the minority (B) begins entering spontaneously.
    InTippingOnly,
    /// Out-tipping only: the all-W endpoint is stable, but W agents exit in a cascade when B exceeds the threshold.
    OutTippingOnly,
    /// Both: spontaneous entry and an exit cascade both occur (typical white flight).
    Both,
    /// Neither: a stable mixed equilibrium exists, and the endpoints are also stable.
    Neither,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TippingClassification {
    pub tipping_type: TippingType,
    pub all_a_stable: bool,
    pub mixed_stable_exists: bool,
}

/// Determines the presence of in/out tipping from the geometry of the reaction curves.
///
/// **In-tipping (geometric condition)**: The peak of the B reaction curve $W_B(B)$ is at least $W_{\max}$.
/// That is, $\max_B W_B(B) \ge W_{\max}$. If this holds, there is a B value at which
/// "B can tolerate the composition even when W = W_max," and B entry beyond that level becomes
/// self-sustaining (a little more $B$ allows $W$ to decrease).
///
/// **Out-tipping**: A stable mixed equilibrium **does not exist**.
/// If stable mixing exists, B entry stops at the mixed equilibrium and no exit cascade occurs.
/// Without stable mixing, W agents exit in a cascade when B exceeds the threshold.
pub fn classify_tipping(phase: &PhaseConfig) -> TippingClassification {
    let eqs = phase.equilibria();

    // Stability of the all-W endpoint (informational: based on linearization).
    let all_white = eqs.iter().find(|e| e.kind == EquilibriumKind::AllWhite);
    let all_white_stable = all_white
        .map(|e| e.stability == super::phase::Stability::Stable)
        .unwrap_or(true);

    // Existence of a stable mixed equilibrium.
    let mixed_stable_exists = eqs.iter().any(|e| {
        e.kind == EquilibriumKind::Mixed && e.stability == super::phase::Stability::Stable
    });

    // Geometric in-tipping: the B reaction-curve peak is at least W_max (there is a path along which B can cover W_max).
    let w_max = phase.w_schedule.pop_max();
    let (_, b_curve_peak_w) = phase.b_reaction().peak();
    let in_tipping = b_curve_peak_w >= w_max - 1e-9;

    // Geometric out-tipping: without stable mixing, W exits in a cascade when B exceeds the threshold.
    let out_tipping = !mixed_stable_exists;

    let tipping_type = match (in_tipping, out_tipping) {
        (true, true) => TippingType::Both,
        (true, false) => TippingType::InTippingOnly,
        (false, true) => TippingType::OutTippingOnly,
        (false, false) => TippingType::Neither,
    };

    TippingClassification {
        tipping_type,
        all_a_stable: all_white_stable,
        mixed_stable_exists,
    }
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

    fn fig19_phase() -> PhaseConfig {
        PhaseConfig {
            w_schedule: ToleranceSchedule::Affine {
                intercept_pop: 20.0,
                slope: 20.0,
                pop_max: 100.0,
            },
            b_schedule: ToleranceSchedule::Affine {
                intercept_pop: 20.0,
                slope: 20.0,
                pop_max: 100.0,
            },
            capacity: None,
        }
    }

    /// Fig.18: Stable all-W + unstable mixing → out-tipping only (W exits in a cascade when B exceeds the threshold).
    #[test]
    fn fig18_classifies_as_out_tipping() {
        let phase = fig18_phase();
        let cls = classify_tipping(&phase);
        assert_eq!(cls.tipping_type, TippingType::OutTippingOnly);
        assert!(cls.all_a_stable);
        assert!(!cls.mixed_stable_exists);
    }

    /// Fig.19: Stable all-W + stable mixing → neither type of tipping.
    #[test]
    fn fig19_classifies_as_neither() {
        let phase = fig19_phase();
        let cls = classify_tipping(&phase);
        assert_eq!(cls.tipping_type, TippingType::Neither);
        assert!(cls.all_a_stable);
        assert!(cls.mixed_stable_exists);
    }

    /// No speculation or asymmetry → the same trajectory as BNM.
    #[test]
    fn no_extensions_matches_bnm() {
        let cfg = TippingConfig {
            phase: fig18_phase(),
            dynamics: DynamicsConfig::default(),
            speculation: Speculation::None,
            asymmetry: None,
            channeling: None,
        };
        let traj = cfg.integrate((90.0, 5.0));
        assert!(traj.converged);
        assert_eq!(
            traj.final_equilibrium.map(|e| e.kind),
            Some(EquilibriumKind::AllWhite)
        );
    }

    /// With speculation: for alpha around 0.5, the trajectory still reaches the correct convergence destination (without diverging).
    #[test]
    fn linear_speculation_does_not_diverge() {
        let cfg = TippingConfig {
            phase: fig18_phase(),
            dynamics: DynamicsConfig::default(),
            speculation: Speculation::Linear { alpha: 0.5 },
            asymmetry: None,
            channeling: None,
        };
        let traj = cfg.integrate((90.0, 5.0));
        let last = traj.history.last().unwrap();
        // The endpoint should be near an extreme point.
        assert!(last.w > 80.0 || last.b > 40.0);
    }

    /// Flow-rate asymmetry: slowing B inflow delays B entry so much that it is overwhelmed by W,
    /// strengthening the tendency toward all_white (an extreme case).
    #[test]
    fn asymmetric_flow_changes_outcome_for_borderline_init() {
        // No speculation; inflow W=2.0, B=0.1 → W has an overwhelming advantage.
        let cfg = TippingConfig {
            phase: fig18_phase(),
            dynamics: DynamicsConfig::default(),
            speculation: Speculation::None,
            asymmetry: Some(FlowAsymmetry {
                w_inflow: 2.0,
                w_outflow: 1.0,
                b_inflow: 0.1,
                b_outflow: 1.0,
            }),
            channeling: None,
        };
        let traj = cfg.integrate((20.0, 20.0));
        // Even from initial conditions near the center, W-dominant inflow leads toward all_white.
        let last = traj.history.last().unwrap();
        assert!(last.w > last.b);
    }
}
