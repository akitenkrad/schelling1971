//! Tolerance Schedule.
//!
//! Represents the cumulative distribution function (CDF) of each individual's
//! "upper limit τ on the proportion of the other color."
//! Under the sorting assumption (the least tolerant leave first), the marginal
//! tolerance R(n) can be recovered from the number n remaining.

use serde::{Deserialize, Serialize};

/// Tolerance schedule. Corresponds to the tolerance schedule in Schelling (1971) §3 (BNM).
///
/// The CDF $F(R)$ returns the number of individuals whose tolerance ratio is at most R.
/// Thus, $F(0) = 0$, $F(R_{\max}) = \text{pop\_max}$.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToleranceSchedule {
    /// Linear: $F(R) = (R / r_{\max}) \cdot \text{pop\_max}$, $R \in [0, r_{\max}]$.
    /// The basic case in Schelling Fig.18.
    Linear { r_max: f64, pop_max: f64 },

    /// Affine: $F(R) = \min(\text{intercept\_pop} + \text{slope} \cdot R, \text{pop\_max})$, $R \ge 0$.
    /// Represents a steep schedule with an intercept (Fig.19).
    /// The maximum tolerance ratio is the smallest R satisfying $F(R) = \text{pop\_max}$.
    Affine {
        intercept_pop: f64,
        slope: f64,
        pop_max: f64,
    },

    /// Piecewise linear: specified by an arbitrary sequence of $(R_i, F(R_i))$ points.
    /// The points must be monotonically increasing in R and nondecreasing in F(R).
    /// Clips to $F = 0$ for $R < R_0$ and to $F = \text{pop\_max}$ for $R > R_n$.
    PiecewiseLinear {
        points: Vec<(f64, f64)>,
        pop_max: f64,
    },
}

impl ToleranceSchedule {
    /// Returns the total population $\text{pop\_max}$.
    pub fn pop_max(&self) -> f64 {
        match *self {
            ToleranceSchedule::Linear { pop_max, .. } => pop_max,
            ToleranceSchedule::Affine { pop_max, .. } => pop_max,
            ToleranceSchedule::PiecewiseLinear { pop_max, .. } => pop_max,
        }
    }

    /// Cumulative distribution $F(R)$. Returns the number of individuals whose tolerance limit is at most $R$.
    /// Clips to 0 for $R < 0$. An Affine schedule with an intercept can represent the
    /// presence of "zero-tolerance individuals" with $F(0) = \text{intercept\_pop}$.
    pub fn cdf(&self, r: f64) -> f64 {
        match self {
            ToleranceSchedule::Linear { r_max, pop_max } => {
                if r <= 0.0 {
                    0.0
                } else if r >= *r_max {
                    *pop_max
                } else {
                    (r / r_max) * pop_max
                }
            }
            ToleranceSchedule::Affine {
                intercept_pop,
                slope,
                pop_max,
            } => {
                if r < 0.0 {
                    return 0.0;
                }
                let v = intercept_pop + slope * r;
                v.clamp(0.0, *pop_max)
            }
            ToleranceSchedule::PiecewiseLinear { points, pop_max } => {
                if points.is_empty() {
                    return 0.0;
                }
                let (first_r, first_f) = points[0];
                let (last_r, last_f) = points[points.len() - 1];
                if r <= first_r {
                    return first_f.min(*pop_max).max(0.0);
                }
                if r >= last_r {
                    return last_f.min(*pop_max);
                }
                for w in points.windows(2) {
                    let (r0, f0) = w[0];
                    let (r1, f1) = w[1];
                    if r >= r0 && r <= r1 {
                        if (r1 - r0).abs() < f64::EPSILON {
                            return f0.min(*pop_max);
                        }
                        let t = (r - r0) / (r1 - r0);
                        return (f0 + t * (f1 - f0)).min(*pop_max);
                    }
                }
                0.0
            }
        }
    }

    /// Marginal tolerance $R(n)$ when $n$ people remain under the sorting assumption.
    /// Because the most tolerant people remain, the tolerance limit of the least tolerant
    /// person among those remaining satisfies $F(R(n)) = \text{pop\_max} - n$.
    ///
    /// Returns $R = 0$ when $n = 0$ (no one is present).
    /// Returns $F^{-1}(0) = 0$ when $n \ge \text{pop\_max}$ (everyone is present).
    pub fn marginal_tolerance(&self, n: f64) -> f64 {
        let pop_max = self.pop_max();
        if n <= 0.0 {
            // If no one remains, the tolerance limit of the "least tolerant next entrant" is the maximum (R_max)
            // However, because complete exit has a different meaning, the BNM treats n=0 as an endpoint.
            // Returning the most tolerant person's tolerance limit is natural here for inflow.
            return self.r_max_finite();
        }
        if n >= pop_max {
            return 0.0;
        }
        let target = pop_max - n; // R satisfying F(R) = pop_max - n
        self.invert_cdf(target)
    }

    /// Returns the numerically meaningful maximum tolerance ratio.
    /// A concrete value for Linear/Affine, and the R at the maximum point for PiecewiseLinear.
    fn r_max_finite(&self) -> f64 {
        match self {
            ToleranceSchedule::Linear { r_max, .. } => *r_max,
            ToleranceSchedule::Affine {
                intercept_pop,
                slope,
                pop_max,
            } => {
                if *slope <= 0.0 {
                    return 0.0;
                }
                ((pop_max - intercept_pop) / slope).max(0.0)
            }
            ToleranceSchedule::PiecewiseLinear { points, .. } => {
                points.last().map(|(r, _)| *r).unwrap_or(0.0)
            }
        }
    }

    /// Returns the smallest $R$ satisfying $F(R) = \text{target}$ (the inverse CDF).
    /// Returns 0 if $\text{target} \le 0$, and $r\_max\_finite$ if $\text{target} \ge \text{pop\_max}$.
    fn invert_cdf(&self, target: f64) -> f64 {
        let pop_max = self.pop_max();
        if target <= 0.0 {
            return 0.0;
        }
        if target >= pop_max {
            return self.r_max_finite();
        }
        match self {
            ToleranceSchedule::Linear { r_max, pop_max } => (target / pop_max) * r_max,
            ToleranceSchedule::Affine {
                intercept_pop,
                slope,
                ..
            } => {
                if *slope <= 0.0 {
                    return 0.0;
                }
                ((target - intercept_pop) / slope).max(0.0)
            }
            ToleranceSchedule::PiecewiseLinear { points, .. } => {
                if points.is_empty() {
                    return 0.0;
                }
                for w in points.windows(2) {
                    let (r0, f0) = w[0];
                    let (r1, f1) = w[1];
                    if target >= f0 && target <= f1 {
                        if (f1 - f0).abs() < f64::EPSILON {
                            return r0;
                        }
                        let t = (target - f0) / (f1 - f0);
                        return r0 + t * (r1 - r0);
                    }
                }
                points.last().map(|(r, _)| *r).unwrap_or(0.0)
            }
        }
    }

    /// Samples the CDF and returns a sequence of $(R, F(R))$ points (for CSV output).
    pub fn sample(&self, n_points: usize) -> Vec<(f64, f64)> {
        let r_max = self.r_max_finite();
        if r_max <= 0.0 || n_points == 0 {
            return Vec::new();
        }
        (0..=n_points)
            .map(|i| {
                let r = r_max * (i as f64) / (n_points as f64);
                (r, self.cdf(r))
            })
            .collect()
    }

    /// Label for CLI/log output.
    pub fn label(&self) -> String {
        match self {
            ToleranceSchedule::Linear { r_max, pop_max } => {
                format!("linear(r_max={:.3}, pop_max={:.1})", r_max, pop_max)
            }
            ToleranceSchedule::Affine {
                intercept_pop,
                slope,
                pop_max,
            } => format!(
                "affine(intercept={:.3}, slope={:.3}, pop_max={:.1})",
                intercept_pop, slope, pop_max
            ),
            ToleranceSchedule::PiecewiseLinear { points, pop_max } => {
                format!(
                    "piecewise(n_points={}, pop_max={:.1})",
                    points.len(),
                    pop_max
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn linear_cdf_endpoints() {
        // Schelling Fig.18: R_max=2.0, pop_max=100
        let s = ToleranceSchedule::Linear {
            r_max: 2.0,
            pop_max: 100.0,
        };
        assert!(approx(s.cdf(0.0), 0.0, 1e-9));
        assert!(approx(s.cdf(1.0), 50.0, 1e-9));
        assert!(approx(s.cdf(2.0), 100.0, 1e-9));
        assert!(approx(s.cdf(3.0), 100.0, 1e-9)); // Clipped
    }

    #[test]
    fn linear_marginal_tolerance_matches_paper_formula() {
        // R(W) = R_max * (1 - W / W_max)
        let s = ToleranceSchedule::Linear {
            r_max: 2.0,
            pop_max: 100.0,
        };
        assert!(approx(s.marginal_tolerance(100.0), 0.0, 1e-9));
        assert!(approx(s.marginal_tolerance(50.0), 1.0, 1e-9));
        assert!(approx(s.marginal_tolerance(0.0), 2.0, 1e-9));
        assert!(approx(s.marginal_tolerance(25.0), 1.5, 1e-9));
    }

    #[test]
    fn affine_cdf_clipping() {
        // intercept_pop=20, slope=40, pop_max=100
        // F(R) = min(20 + 40*R, 100)
        let s = ToleranceSchedule::Affine {
            intercept_pop: 20.0,
            slope: 40.0,
            pop_max: 100.0,
        };
        assert!(approx(s.cdf(0.0), 20.0, 1e-9));
        assert!(approx(s.cdf(1.0), 60.0, 1e-9));
        assert!(approx(s.cdf(2.0), 100.0, 1e-9));
        assert!(approx(s.cdf(3.0), 100.0, 1e-9));
    }

    #[test]
    fn piecewise_linear_interpolation() {
        // Example with a bend in the middle: (0,0)-(1,30)-(2,100)
        let s = ToleranceSchedule::PiecewiseLinear {
            points: vec![(0.0, 0.0), (1.0, 30.0), (2.0, 100.0)],
            pop_max: 100.0,
        };
        assert!(approx(s.cdf(0.5), 15.0, 1e-9));
        assert!(approx(s.cdf(1.0), 30.0, 1e-9));
        assert!(approx(s.cdf(1.5), 65.0, 1e-9));
    }

    #[test]
    fn invert_round_trip() {
        let s = ToleranceSchedule::Linear {
            r_max: 2.0,
            pop_max: 100.0,
        };
        for &r in &[0.1, 0.5, 1.0, 1.5, 1.9] {
            let f = s.cdf(r);
            let r_back = s.invert_cdf(f);
            assert!(approx(r, r_back, 1e-9), "r={}, r_back={}", r, r_back);
        }
    }
}
