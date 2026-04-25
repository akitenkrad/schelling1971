//! 反応曲線 (Reaction Curve)．
//!
//! 許容限界スケジュールを「比率→絶対数」へ変換した曲線．
//! Schelling (1971) p.170 のパラボラ $B_W(W) = R_{\max} \cdot W \cdot (1 - W/W_{\max})$ に対応する．

use super::tolerance::ToleranceSchedule;

/// 反応曲線．
///
/// `own = n` 人が残留しているとき，最も不寛容な残留者が許容できる「他色の最大数」
/// $B_W(W) = W \cdot R(W)$ を表す．R(W) は [`ToleranceSchedule::marginal_tolerance`]．
pub struct ReactionCurve<'a> {
    pub schedule: &'a ToleranceSchedule,
}

impl<'a> ReactionCurve<'a> {
    pub fn new(schedule: &'a ToleranceSchedule) -> Self {
        Self { schedule }
    }

    /// $B_W(W) = W \cdot R(W)$．`own` 人残留時に許容できる他色の最大数．
    pub fn max_other(&self, own: f64) -> f64 {
        if own <= 0.0 {
            return 0.0;
        }
        own * self.schedule.marginal_tolerance(own)
    }

    /// 等間隔サンプリング．`(W, B_W(W))` の点列を返す (CSV 出力用)．
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

    /// 数値微分 $\frac{d B_W}{d W}$．安定性判定 (反応曲線が容量制約を横切る方向) で利用する．
    pub fn derivative(&self, own: f64) -> f64 {
        let h = (self.schedule.pop_max() * 1e-6).max(1e-9);
        let lo = (own - h).max(0.0);
        let hi = (own + h).min(self.schedule.pop_max());
        if hi <= lo {
            return 0.0;
        }
        (self.max_other(hi) - self.max_other(lo)) / (hi - lo)
    }

    /// 反応曲線の頂点 (放物線の極大点) の $W$ 座標を数値的に探索する．
    /// 連続スケジュールでは頂点は一意．
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
        // 端点
        assert!(approx(rc.max_other(0.0), 0.0, 1e-9));
        assert!(approx(rc.max_other(100.0), 0.0, 1e-9));
        // 頂点 W = W_max/2 = 50, B_W = R_max*W_max/4 = 50
        assert!(approx(rc.max_other(50.0), 50.0, 1e-9));
        // 中間点
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
        // 上昇相 (W < 50)
        assert!(rc.derivative(25.0) > 0.0);
        // 下降相 (W > 50)
        assert!(rc.derivative(75.0) < 0.0);
        // 頂点付近はほぼ0
        assert!(rc.derivative(50.0).abs() < 1e-3);
    }

    #[test]
    fn smaller_population_smaller_curve() {
        // B 集団総数 50 のケース: peak は 25 で値 25
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
