//! Reaction Curve.
//!
//! A curve that converts a tolerance schedule from ratios to absolute counts.
//! Corresponds to the parabola $B_W(W) = R_{\max} \cdot W \cdot (1 - W/W_{\max})$ in Schelling (1971) p.170.

use super::tolerance::ToleranceSchedule;

/// Reaction curve.
///
/// Represents the maximum number of the other color that the least tolerant remaining person
/// can tolerate when `own = n` people remain: $B_W(W) = W \cdot R(W)$, where R(W) is
/// [`ToleranceSchedule::marginal_tolerance`].
pub struct ReactionCurve<'a> {
    pub schedule: &'a ToleranceSchedule,
}

impl<'a> ReactionCurve<'a> {
    pub fn new(schedule: &'a ToleranceSchedule) -> Self {
        Self { schedule }
    }

    /// $B_W(W) = W \cdot R(W)$. The maximum number of the other color that can be tolerated when `own` people remain.
    pub fn max_other(&self, own: f64) -> f64 {
        if own <= 0.0 {
            return 0.0;
        }
        own * self.schedule.marginal_tolerance(own)
    }

    /// Samples at equal intervals. Returns a sequence of `(W, B_W(W))` points (for CSV output).
    pub fn sample(&self, n_points: usize) -> Vec<(f64, f64)> {
        let pop_max = self.schedule.pop_max();
        if pop_max <= 0.0 || n_points == 0 {
            return Vec::new();
        }
        (0..=n_points)
            .map(|i| {
                let w = pop_max * (i as f64) / (n_points as f64);
                (w, self.max_other(w))
            })
            .collect()
    }

    /// Numerical derivative $\frac{d B_W}{d W}$. Used to determine stability (the direction in which the reaction curve crosses the capacity constraint).
    #[allow(dead_code)]
    pub fn derivative(&self, own: f64) -> f64 {
        let h = (self.schedule.pop_max() * 1e-6).max(1e-9);
        let lo = (own - h).max(0.0);
        let hi = (own + h).min(self.schedule.pop_max());
        if hi <= lo {
            return 0.0;
        }
        (self.max_other(hi) - self.max_other(lo)) / (hi - lo)
    }

    /// Numerically searches for the $W$ coordinate of the reaction curve's vertex (the parabola's maximum).
    /// The vertex is unique for continuous schedules.
    pub fn peak(&self) -> (f64, f64) {
        let pop_max = self.schedule.pop_max();
        let n = 1000;
        let mut best = (0.0, 0.0);
        for i in 0..=n {
            let w = pop_max * (i as f64) / (n as f64);
            let b = self.max_other(w);
            if b > best.1 {
                best = (w, b);
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn linear_schedule_yields_paper_parabola() {
        // Schelling Fig.18: B_W(W) = 2.0 * W * (1 - W/100)
        let s = ToleranceSchedule::Linear {
            r_max: 2.0,
            pop_max: 100.0,
        };
        let rc = ReactionCurve::new(&s);
        // Endpoints
        assert!(approx(rc.max_other(0.0), 0.0, 1e-9));
        assert!(approx(rc.max_other(100.0), 0.0, 1e-9));
        // Vertex W = W_max/2 = 50, B_W = R_max*W_max/4 = 50
        assert!(approx(rc.max_other(50.0), 50.0, 1e-9));
        // Intermediate points
        assert!(approx(rc.max_other(25.0), 37.5, 1e-9));
        assert!(approx(rc.max_other(75.0), 37.5, 1e-9));
    }

    #[test]
    fn peak_is_at_half() {
        let s = ToleranceSchedule::Linear {
            r_max: 2.0,
            pop_max: 100.0,
        };
        let rc = ReactionCurve::new(&s);
        let (w_peak, b_peak) = rc.peak();
        assert!((w_peak - 50.0).abs() < 0.5);
        assert!((b_peak - 50.0).abs() < 1e-3);
    }

    #[test]
    fn derivative_signs_around_peak() {
        let s = ToleranceSchedule::Linear {
            r_max: 2.0,
            pop_max: 100.0,
        };
        let rc = ReactionCurve::new(&s);
        // Ascending phase (W < 50)
        assert!(rc.derivative(25.0) > 0.0);
        // Descending phase (W > 50)
        assert!(rc.derivative(75.0) < 0.0);
        // Approximately 0 near the vertex
        assert!(rc.derivative(50.0).abs() < 1e-3);
    }

    #[test]
    fn smaller_population_smaller_curve() {
        // Case with a total B population of 50: the peak is at 25 with value 25
        let s = ToleranceSchedule::Linear {
            r_max: 2.0,
            pop_max: 50.0,
        };
        let rc = ReactionCurve::new(&s);
        let (w_peak, b_peak) = rc.peak();
        assert!((w_peak - 25.0).abs() < 0.5);
        assert!((b_peak - 25.0).abs() < 1e-3);
    }
}
