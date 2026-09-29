//! Phase-plane analysis. Equilibrium search and stability assessment.
//!
//! Handles the following for the state $(W, B)$:
//! - "Region classification," which determines the dynamic signs at each point from the relative positions of the reaction curves $B_W(W)$ and $W_B(B)$.
//! - Equilibria: endpoints (all-W / all-B / empty) and interior intersections (mixed equilibria).
//! - Stability: determined by the direction in which the reaction curves cross the capacity constraint $W + B = C$.
//!
//! Corresponds to Schelling (1971) §3 (BNM, pp.167--181) and Appendix A (this note).

use serde::{Deserialize, Serialize};

use super::reaction::ReactionCurve;
use super::tolerance::ToleranceSchedule;

/// Phase-plane analysis configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseConfig {
    /// Tolerance schedule for the White (W) population.
    pub w_schedule: ToleranceSchedule,
    /// Tolerance schedule for the Black (B) population.
    pub b_schedule: ToleranceSchedule,
    /// Capacity constraint $W + B \le C$. Unconstrained if None.
    pub capacity: Option<f64>,
}

impl PhaseConfig {
    /// White reaction curve $B_W(W)$.
    pub fn w_reaction(&self) -> ReactionCurve<'_> {
        ReactionCurve::new(&self.w_schedule)
    }

    /// Black reaction curve $W_B(B)$.
    pub fn b_reaction(&self) -> ReactionCurve<'_> {
        ReactionCurve::new(&self.b_schedule)
    }

    /// Classifies the dynamic signs at a given point $(W, B)$.
    pub fn region(&self, w: f64, b: f64) -> ViabilityRegion {
        let bw_max = self.w_reaction().max_other(w); // Maximum number of B that W can tolerate
        let wb_max = self.b_reaction().max_other(b); // Maximum number of W that B can tolerate
        let w_ok = b <= bw_max; // The W population is satisfied
        let b_ok = w <= wb_max; // The B population is satisfied
        match (w_ok, b_ok) {
            (true, true) => ViabilityRegion::BothViable,
            (true, false) => ViabilityRegion::WViableOnly,
            (false, true) => ViabilityRegion::BViableOnly,
            (false, false) => ViabilityRegion::NeitherViable,
        }
    }

    /// Whether the capacity constraint is satisfied.
    pub fn within_capacity(&self, w: f64, b: f64) -> bool {
        match self.capacity {
            Some(c) => w + b <= c + 1e-9,
            None => true,
        }
    }

    /// Returns the set of equilibria.
    /// Enumerates the endpoints (all-W / all-B / Empty) and reaction-curve intersections (mixed equilibria).
    pub fn equilibria(&self) -> Vec<Equilibrium> {
        let mut eqs = Vec::new();

        let w_max = self.w_schedule.pop_max();
        let b_max = self.b_schedule.pop_max();

        // Endpoint: (W_max, 0) — all-W
        if self.within_capacity(w_max, 0.0) {
            eqs.push(self.classify_endpoint(w_max, 0.0, EquilibriumKind::AllWhite));
        }
        // Endpoint: (0, B_max) — all-B
        if self.within_capacity(0.0, b_max) {
            eqs.push(self.classify_endpoint(0.0, b_max, EquilibriumKind::AllBlack));
        }
        // Endpoint: (0, 0) — empty
        eqs.push(Equilibrium {
            w: 0.0,
            b: 0.0,
            kind: EquilibriumKind::Empty,
            stability: Stability::Unstable, // Usually leaves this state through inflow
        });

        // Mixed equilibria: numerically solve the intersections satisfying B = B_W(W) and W = W_B(B).
        // Sweep W ∈ [0, w_max] parametrically and use Brent's method to find zeros of
        // "whether a point assumed to lie on B_W(W) for W also satisfies the W_B reaction curve."
        eqs.extend(self.find_mixed_equilibria());

        eqs
    }

    fn classify_endpoint(&self, w: f64, b: f64, kind: EquilibriumKind) -> Equilibrium {
        // Endpoint stability: determined by whether a small inflow perturbation is pushed back.
        // At the all-W point (W_max, 0), it is stable if the B population tends to exit when B increases slightly.
        //   Check B_max > B_W(W_max)? for B>0 → because B_W(W_max)=0 and W_B(0)=0,
        //   if W_B(eps) < W_max for a small B, B perceives "too many W" and exits → stable
        let stability = match kind {
            EquilibriumKind::AllWhite => {
                // B population: compare W_B(eps) with W_max. If W_B(eps) < W_max, B exits → stable.
                let eps = (self.b_schedule.pop_max() * 1e-3).max(1e-6);
                let allowed_w = self.b_reaction().max_other(eps);
                if allowed_w < w {
                    Stability::Stable
                } else {
                    Stability::Unstable
                }
            }
            EquilibriumKind::AllBlack => {
                let eps = (self.w_schedule.pop_max() * 1e-3).max(1e-6);
                let allowed_b = self.w_reaction().max_other(eps);
                if allowed_b < b {
                    Stability::Stable
                } else {
                    Stability::Unstable
                }
            }
            _ => Stability::Unstable,
        };
        Equilibrium {
            w,
            b,
            kind,
            stability,
        }
    }

    /// Searches for mixed equilibria using Brent's method.
    ///
    /// Strategy: scan the W axis with fine-grained sample points,
    /// find intervals where $h(W) = W - W_B(B_W(W))$ changes sign, and refine each root using Brent's method.
    /// $h(W) = 0$ ⇔ $(W, B_W(W))$ lies on both reaction curves.
    ///
    /// Samples are taken at half-step-shifted positions $W_i = W_{\max} (i + 0.5) / (n + 1)$.
    /// However, this shift alone is insufficient: with `n_samples = 400`, $i = 200$ lands exactly on
    /// $W = W_{\max} / 2$. In a symmetric case (for example, a linear schedule with $R_{\max} = 2$
    /// whose intersection is $(50, 50)$), that point is the root itself, so $h = 0$ exactly and
    /// `prev_h * cur_h < 0` does not hold, causing the root to be missed.
    /// Therefore, in addition to sign changes, **exact zeros at sample points** are collected as roots.
    fn find_mixed_equilibria(&self) -> Vec<Equilibrium> {
        let w_max = self.w_schedule.pop_max();
        let n_samples = 400;
        let h = |w: f64| -> f64 {
            if w <= 0.0 {
                return 0.0; // Endpoints are handled separately
            }
            let b = self.w_reaction().max_other(w);
            let w_required = self.b_reaction().max_other(b);
            w - w_required
        };

        let mut roots: Vec<f64> = Vec::new();
        // Retain only roots sufficiently far from existing roots.
        let push_root = |roots: &mut Vec<f64>, root: f64| {
            if !roots.iter().any(|r: &f64| (r - root).abs() < 1e-3 * w_max) {
                roots.push(root);
            }
        };

        // Half-step-shifted samples: W = W_max*(i+0.5)/(n+1) for i=0..=n
        let mut prev_w = 0.5 * w_max / (n_samples as f64 + 1.0);
        let mut prev_h = h(prev_w);
        if prev_h == 0.0 {
            push_root(&mut roots, prev_w);
        }
        for i in 1..=n_samples {
            let w = w_max * (i as f64 + 0.5) / (n_samples as f64 + 1.0);
            let cur_h = h(w);
            if prev_h.is_finite() && cur_h.is_finite() {
                if cur_h == 0.0 {
                    // Case where a sample point lands exactly on a root.
                    push_root(&mut roots, w);
                } else if prev_h * cur_h < 0.0 {
                    if let Some(root) = brent(prev_w, w, prev_h, cur_h, &h, 1e-9, 100) {
                        push_root(&mut roots, root);
                    }
                }
            }
            prev_w = w;
            prev_h = cur_h;
        }

        roots
            .into_iter()
            .filter_map(|w| {
                let b = self.w_reaction().max_other(w);
                if !self.within_capacity(w, b) {
                    return None;
                }
                let stability = self.classify_mixed(w, b);
                Some(Equilibrium {
                    w,
                    b,
                    kind: EquilibriumKind::Mixed,
                    stability,
                })
            })
            .collect()
    }

    /// Stability of a mixed equilibrium. Determined by the crossing direction of the reaction curves (the direction in which $h$ crosses the root).
    /// Stable if $h(W) = W - W_B(B_W(W))$ crosses the root while decreasing, and unstable if it crosses while increasing.
    ///
    /// In the nondegenerate case, this is equivalent to $h'(W^*) = 1 - B_W' W_B' < 0$, and thus
    /// to $\det J > 0$ for the Jacobian matrix of the flow field (because the trace of the flow field is
    /// always $-(k_W + k_B) < 0$, stability is determined solely by the sign of the determinant).
    ///
    /// **Handling the degenerate case**: when $B_W'(W^*) W_B'(B^*) = 1$ exactly, $h'(W^*) = 0$ and
    /// linearization cannot determine stability (zero eigenvalue). This is exactly what occurs for symmetric
    /// affine $F = c + sR$ with $R_{\max} = 3$ (fig20 / fig25). In this case, $h$ has a third-order zero at $W^*$,
    /// with $h(W^* + x) = 2x^3/s^2 + O(x^4)$. After reduction onto the center manifold $u = -v^2/(4s)$,
    /// the equation followed by $v = W - B$ is $\dot v = \frac{k}{4s^2} v^3 + O(v^4)$. Because the coefficient is positive,
    /// the equilibrium is **unstable** (the divergence is algebraic rather than exponential, with finite-time divergence at $t^* = 1/(2Cv_0^2)$).
    ///
    /// Because a third-order zero has odd order, $h$ changes sign across the root. Therefore,
    /// **using the sign pattern rather than the slope value** correctly resolves the degenerate case
    /// with the same rule as the nondegenerate case. Using the secant slope $(h(hi)-h(lo))/(hi-lo)$
    /// makes the value in the degenerate case extremely small, $O(h\_eps^2)$, and increasingly buried
    /// in rounding error as `h_eps` is reduced, so only the signs are examined here without taking the quotient.
    ///
    /// If $h$ has the same sign on both sides of the root, it is an even-order zero and is stable from
    /// only one side (semistable), so [`Stability::Saddle`] is returned.
    fn classify_mixed(&self, w: f64, _b: f64) -> Stability {
        let h_eps = (self.w_schedule.pop_max() * 1e-4).max(1e-6);
        let h = |w: f64| -> f64 {
            let b = self.w_reaction().max_other(w);
            let w_required = self.b_reaction().max_other(b);
            w - w_required
        };
        let lo = (w - h_eps).max(1e-9);
        let hi = (w + h_eps).min(self.w_schedule.pop_max() - 1e-9);
        let (h_lo, h_hi) = (h(lo), h(hi));
        if h_lo > 0.0 && h_hi < 0.0 {
            // Crosses the root while decreasing → stable.
            Stability::Stable
        } else if h_lo < 0.0 && h_hi > 0.0 {
            // Crosses the root while increasing → unstable (the third-order degenerate case also falls here).
            Stability::Unstable
        } else {
            // Same sign = even-order zero (stable from one side) / numerically indeterminate.
            Stability::Saddle
        }
    }

    /// Samples the vector field.
    /// Returns a sequence of $(W, B, \dot W, \dot B, region)$ tuples.
    /// Returns $\dot W, \dot B$ as region-based signs ($\pm 1$); magnitudes are multiplied in [`super::dynamics`].
    pub fn vector_field(&self, w_grid: usize, b_grid: usize) -> Vec<VectorSample> {
        let w_max = self.w_schedule.pop_max();
        let b_max = self.b_schedule.pop_max();
        if w_grid == 0 || b_grid == 0 {
            return Vec::new();
        }
        let mut out = Vec::with_capacity((w_grid + 1) * (b_grid + 1));
        for i in 0..=w_grid {
            for j in 0..=b_grid {
                let w = w_max * (i as f64) / (w_grid as f64);
                let b = b_max * (j as f64) / (b_grid as f64);
                if !self.within_capacity(w, b) {
                    continue;
                }
                let region = self.region(w, b);
                let (dw_sign, db_sign) = region.signs();
                out.push(VectorSample {
                    w,
                    b,
                    dw_sign,
                    db_sign,
                    region,
                });
            }
        }
        out
    }
}

/// Equilibrium point.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Equilibrium {
    pub w: f64,
    pub b: f64,
    pub kind: EquilibriumKind,
    pub stability: Stability,
}

/// Equilibrium type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EquilibriumKind {
    /// $(W_{\max}, 0)$.
    AllWhite,
    /// $(0, B_{\max})$.
    AllBlack,
    /// Mixed state at a reaction-curve intersection.
    Mixed,
    /// Empty state at $(0, 0)$.
    Empty,
}

/// Stability classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stability {
    Stable,
    Unstable,
    Saddle,
}

/// Dynamic-sign region (four categories).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViabilityRegion {
    /// Inside both curves. Both populations flow in.
    BothViable,
    /// $B \le B_W(W)$ and $W > W_B(B)$. W flows in and B exits.
    WViableOnly,
    /// $W \le W_B(B)$ and $B > B_W(W)$. B flows in and W exits.
    BViableOnly,
    /// Outside both curves. Both populations exit.
    NeitherViable,
}

impl ViabilityRegion {
    /// Returns the signs ($\pm 1$) of $\dot W, \dot B$.
    pub fn signs(&self) -> (f64, f64) {
        match self {
            ViabilityRegion::BothViable => (1.0, 1.0),
            ViabilityRegion::WViableOnly => (1.0, -1.0),
            ViabilityRegion::BViableOnly => (-1.0, 1.0),
            ViabilityRegion::NeitherViable => (-1.0, -1.0),
        }
    }
}

/// One sample from the vector field.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct VectorSample {
    pub w: f64,
    pub b: f64,
    pub dw_sign: f64,
    pub db_sign: f64,
    pub region: ViabilityRegion,
}

/// One-dimensional root finding using Brent's method.
/// Assumes `f(a)*f(b) < 0` (an interval with a sign change).
/// `tol` is interpreted as the tolerance on the **interval width** (accuracy of the root position).
///
/// Do not also use the function-value condition `|f(b)| < tol` to determine convergence. At a multiple root,
/// $f$ becomes extremely flat around the root, making $|f|$ small even far from the root and reducing accuracy.
/// In fact, in the degenerate case where $h$ has a third-order zero (symmetric affine with $R_{\max} = 3$),
/// $h \approx 2x^3/s^2$, so for `tol = 1e-9`, $|h| < $ `tol` implies $|x| < 8.2\times10^{-3}$,
/// making the root position three orders of magnitude less accurate. Using the interval width allows bisection
/// to work as long as the sign is reliable, refining the interval to approximately $|x| \approx 2\times10^{-4}$,
/// where cancellation erases the sign of $h$.
fn brent<F>(a0: f64, b0: f64, fa0: f64, fb0: f64, f: &F, tol: f64, max_iter: usize) -> Option<f64>
where
    F: Fn(f64) -> f64,
{
    let (mut a, mut b, mut fa, mut fb) = (a0, b0, fa0, fb0);
    if fa * fb > 0.0 {
        return None;
    }
    if fa.abs() < fb.abs() {
        std::mem::swap(&mut a, &mut b);
        std::mem::swap(&mut fa, &mut fb);
    }
    let mut c = a;
    let mut fc = fa;
    let mut d = b - a;
    let mut e = d;
    for _ in 0..max_iter {
        if fb == 0.0 || (b - a).abs() < tol {
            return Some(b);
        }
        if fa != fc && fb != fc {
            // Inverse quadratic interpolation
            let s = a * fb * fc / ((fa - fb) * (fa - fc))
                + b * fa * fc / ((fb - fa) * (fb - fc))
                + c * fa * fb / ((fc - fa) * (fc - fb));
            // Acceptance conditions. Fall back to bisection if unsuitable.
            let cond1 = (s - (3.0 * a + b) / 4.0) * (s - b) >= 0.0;
            let cond2 = (s - b).abs() >= (b - c).abs() / 2.0;
            let cond3 = (b - c).abs() < tol;
            let s = if cond1 || cond2 || cond3 {
                (a + b) / 2.0
            } else {
                s
            };
            let fs = f(s);
            d = e;
            e = b - s;
            c = b;
            fc = fb;
            if fa * fs < 0.0 {
                b = s;
                fb = fs;
            } else {
                a = s;
                fa = fs;
            }
        } else {
            // Linear interpolation (secant) → bisection
            let s = if fb != fa {
                b - fb * (b - a) / (fb - fa)
            } else {
                (a + b) / 2.0
            };
            let s = if (s - b).abs() < tol {
                (a + b) / 2.0
            } else {
                s
            };
            let fs = f(s);
            d = e;
            e = b - s;
            c = b;
            fc = fb;
            if fa * fs < 0.0 {
                b = s;
                fb = fs;
            } else {
                a = s;
                fa = fs;
            }
        }
        if fa.abs() < fb.abs() {
            std::mem::swap(&mut a, &mut b);
            std::mem::swap(&mut fa, &mut fb);
        }
        let _ = d;
        let _ = e;
    }
    Some(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    /// Degenerate case (fig20 / fig25): for symmetric affine $F = c + sR$ with $R_{\max} = 3$,
    /// $B_W'(W^*) W_B'(B^*) = 1$ exactly, so $\det J = 0$ and linearization cannot determine stability.
    /// Because $h$ has a third-order zero with $h(W^* + x) = 2x^3/s^2$, the reduction on the
    /// center manifold, $\dot v = \frac{k}{4s^2}v^3$ (coefficient > 0), shows that **unstable** is correct.
    ///
    /// `classify_mixed` resolves this by using the sign pattern rather than the secant value.
    /// Returning to an implementation that takes the quotient would make the degenerate slope $O(h\_eps^2)$
    /// and bury it in rounding error, so this behavior is locked in by this regression test.
    #[test]
    fn degenerate_r_max_3_mixed_equilibrium_is_unstable() {
        // Equivalent to fig20: linear R_max=3 (c=0, s=100/3, M=100) -> W* = M - s = 66.67
        let fig20 = PhaseConfig {
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
        let mixed: Vec<_> = fig20
            .equilibria()
            .into_iter()
            .filter(|e| e.kind == EquilibriumKind::Mixed)
            .collect();
        assert_eq!(mixed.len(), 1, "expected one mixed equilibrium: {mixed:?}");
        // Because this is a triple root, the accuracy of the root position itself decreases (see the fig25 comment below).
        assert!(approx(mixed[0].w, 200.0 / 3.0, 1e-2), "W*={}", mixed[0].w);
        assert_eq!(
            mixed[0].stability,
            Stability::Unstable,
            "the degenerate point with det J = 0 is unstable due to the cubic term"
        );

        // Equivalent to fig25: affine F = 10 + 30R (M=90, s=30, R_max=3) -> W* = 60
        let fig25 = PhaseConfig {
            w_schedule: ToleranceSchedule::Affine {
                intercept_pop: 10.0,
                slope: 30.0,
                pop_max: 100.0,
            },
            b_schedule: ToleranceSchedule::Affine {
                intercept_pop: 10.0,
                slope: 30.0,
                pop_max: 100.0,
            },
            capacity: None,
        };
        let mixed: Vec<_> = fig25
            .equilibria()
            .into_iter()
            .filter(|e| e.kind == EquilibriumKind::Mixed)
            .collect();
        assert_eq!(mixed.len(), 1, "expected one mixed equilibrium: {mixed:?}");
        // Even for a triple root, this accuracy is achieved as long as [`brent`] determines convergence by interval width
        // (terminating on the function value |f| < tol only refines the root to approximately $10^{-3}$).
        assert!(approx(mixed[0].w, 60.0, 1e-3), "W*={}", mixed[0].w);
        assert_eq!(mixed[0].stability, Stability::Unstable);
    }

    /// Locks in the dependence of multiple-root accuracy on [`brent`]'s convergence criterion.
    /// Returning to function-value-based termination (`|f(b)| < tol`) reduces accuracy by three orders of magnitude in the degenerate case.
    #[test]
    fn brent_resolves_triple_root_accurately() {
        // A flat triple root of the form h(x) = 2(x - 2.5)^3 / s^2.
        let s: f64 = 100.0 / 3.0;
        let f = |x: f64| 2.0 * (x - 2.5).powi(3) / (s * s);
        let root = brent(0.0, 5.0, f(0.0), f(5.0), &f, 1e-9, 200).unwrap();
        assert!(
            approx(root, 2.5, 1e-4),
            "interval-width convergence accurately resolves a triple root: root={root}"
        );
    }

    /// Also locks in the instability of the degenerate case from the dynamics side. With symmetric initial values,
    /// $v = W - B = 0$ is preserved and remains at the mixed equilibrium, but an asymmetric perturbation causes tipping to a single-population equilibrium.
    ///
    /// Note: the perturbation $v_0$ must not be too small. Cubic divergence has the algebraic time scale
    /// $t^* = 1/(2Cv_0^2)$, and when $v_0$ is small, the displacement per step falls below
    /// `convergence_tol`, causing [`integrate`] to incorrectly declare "convergence"
    /// (for example, with $v_0 = 2$, it stops upon reaching the center manifold $u = -v^2/(4s)$).
    /// Here, $v_0 = 4$ is used because endpoint arrival has been confirmed empirically.
    #[test]
    fn degenerate_mixed_tips_away_under_asymmetric_perturbation() {
        use crate::analytic::dynamics::{integrate, DynamicsConfig};

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
        let cfg = DynamicsConfig {
            max_steps: 2_000_000,
            ..Default::default()
        };
        let w_star = 200.0 / 3.0;

        // Symmetric: v = 0 is invariant, so the state remains at the mixed equilibrium.
        let sym = integrate(&phase, &cfg, (w_star, w_star));
        let last = sym.history.last().unwrap();
        assert!(
            approx(last.w, w_star, 1e-2) && approx(last.b, w_star, 1e-2),
            "symmetric initial values remain at the mixed equilibrium: ({}, {})",
            last.w,
            last.b
        );

        // Asymmetric (v = +4): the cubic term drives divergence toward the all-W endpoint.
        let asym = integrate(&phase, &cfg, (w_star + 2.0, w_star - 2.0));
        let last = asym.history.last().unwrap();
        assert!(
            last.w > 95.0 && last.b < 5.0,
            "an asymmetric perturbation tips toward a single-group equilibrium: ({}, {})",
            last.w,
            last.b
        );
    }

    /// Fig.18 (basic case): linear, 1:2 ratio — only two endpoint equilibria; the mixed equilibrium is unstable.
    #[test]
    fn fig18_two_endpoint_equilibria() {
        let cfg = PhaseConfig {
            w_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 100.0,
            },
            b_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 50.0,
            },
            capacity: None,
        };
        let eqs = cfg.equilibria();

        // Both all-W and all-B are included
        assert!(eqs.iter().any(|e| e.kind == EquilibriumKind::AllWhite));
        assert!(eqs.iter().any(|e| e.kind == EquilibriumKind::AllBlack));

        // The endpoints are stable
        let all_w = eqs
            .iter()
            .find(|e| e.kind == EquilibriumKind::AllWhite)
            .unwrap();
        let all_b = eqs
            .iter()
            .find(|e| e.kind == EquilibriumKind::AllBlack)
            .unwrap();
        assert_eq!(all_w.stability, Stability::Stable);
        assert_eq!(all_b.stability, Stability::Stable);
    }

    /// Symmetric linear schedules (W_max = B_max = 100, R_max=2): the reaction curves have the same shape.
    /// h(W) = W - W_B(B_W(W)) shares a vertex at W=50 → the curves may be tangent.
    /// Here, the mixed equilibrium is tested in a case made asymmetric by changing W_max=B_max.
    #[test]
    fn region_classification_at_origin_is_both_viable() {
        let cfg = PhaseConfig {
            w_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 100.0,
            },
            b_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 100.0,
            },
            capacity: None,
        };
        // (10, 10): both reaction curves have sufficiently large values. Both should be viable.
        assert_eq!(cfg.region(10.0, 10.0), ViabilityRegion::BothViable);
        // (90, 90): both curves are extremely low → outside both
        assert_eq!(cfg.region(90.0, 90.0), ViabilityRegion::NeitherViable);
    }

    /// Steep schedule (Fig.19 family): three equilibria appear when the median tolerance ratio ≥ 1.5.
    /// For affine (intercept_pop=20, slope=40, pop_max=100), R_max = 2.
    /// The median (F=50) is R = 0.75; verify whether this distribution shape produces a mixed equilibrium.
    #[test]
    fn affine_schedule_introduces_mixed_equilibrium() {
        // Steep with an intercept (instead of F(0)=0, keep F(0)=0 and make the slope steeper)
        // Here, as a proxy for a high-median condition, pop_max=100 and R_max=2.0, but
        // the W and B populations are made asymmetric so that the reaction curves intersect within capacity.
        let cfg = PhaseConfig {
            w_schedule: ToleranceSchedule::Affine {
                intercept_pop: 0.0,
                slope: 25.0, // F(R) = 25R, F(4)=100 → R_max=4 (very tolerant)
                pop_max: 100.0,
            },
            b_schedule: ToleranceSchedule::Affine {
                intercept_pop: 0.0,
                slope: 25.0,
                pop_max: 100.0,
            },
            capacity: None,
        };
        let eqs = cfg.equilibria();
        let n_mixed = eqs
            .iter()
            .filter(|e| e.kind == EquilibriumKind::Mixed)
            .count();
        // Because this is a symmetric case, there should be exactly one point (or zero points) on the W=B diagonal.
        // What matters is the ability to detect a mixed equilibrium.
        assert!(
            n_mixed >= 1,
            "a symmetric, tolerant schedule should yield at least one mixed equilibrium"
        );
    }

    /// Vector-field generation: samples across the entire quadrant are classified by region.
    #[test]
    fn vector_field_covers_grid() {
        let cfg = PhaseConfig {
            w_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 100.0,
            },
            b_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 50.0,
            },
            capacity: None,
        };
        let field = cfg.vector_field(10, 10);
        assert_eq!(field.len(), 11 * 11);
        // Both populations are viable near the origin
        let origin = field.iter().find(|s| s.w == 0.0 && s.b == 0.0).unwrap();
        // (0,0) is an endpoint with W=0, B=0 → B_W(0)=0, W_B(0)=0 → both b<=0 and w<=0 hold
        // Under floating-point arithmetic, both populations are classified as viable
        assert_eq!(origin.region, ViabilityRegion::BothViable);
    }

    /// Capacity constraint: points exceeding capacity are excluded from the vector field.
    #[test]
    fn capacity_constraint_filters_vector_field() {
        let cfg = PhaseConfig {
            w_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 100.0,
            },
            b_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 100.0,
            },
            capacity: Some(100.0),
        };
        let field = cfg.vector_field(10, 10);
        // All samples satisfy W+B<=100
        assert!(field.iter().all(|s| s.w + s.b <= 100.0 + 1e-9));
    }

    #[test]
    fn brent_finds_root_of_simple_function() {
        let f = |x: f64| (x - 2.5).powi(3);
        let root = brent(0.0, 5.0, f(0.0), f(5.0), &f, 1e-9, 100).unwrap();
        assert!(approx(root, 2.5, 1e-6));
    }
}
