//! 許容限界スケジュール (Tolerance Schedule)．
//!
//! 各個人の「異色比率の上限 τ」の累積分布関数 (CDF) を表す．
//! ソート仮定 (最も不寛容な者から退出) の下で，残留人数 n から
//! 周辺許容限界 R(n) を逆引きできる．

use serde::{Deserialize, Serialize};

/// 許容限界スケジュール．Schelling (1971) §3 (BNM) の許容スケジュールに対応する．
///
/// CDF $F(R)$ は「許容比率が R 以下の個人の人数」を返す．
/// すなわち $F(0) = 0$, $F(R_{\max}) = \text{pop\_max}$．
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToleranceSchedule {
    /// 直線型: $F(R) = (R / r_{\max}) \cdot \text{pop\_max}$, $R \in [0, r_{\max}]$．
    /// Schelling Fig.18 の基本ケース．
    Linear { r_max: f64, pop_max: f64 },

    /// アフィン型: $F(R) = \min(\text{intercept\_pop} + \text{slope} \cdot R, \text{pop\_max})$, $R \ge 0$．
    /// 切片付き急勾配スケジュール (Fig.19) を表す．
    /// 最大許容比率は $F(R) = \text{pop\_max}$ を満たす最小の R で決まる．
    Affine {
        intercept_pop: f64,
        slope: f64,
        pop_max: f64,
    },

    /// 区分線形: 任意の $(R_i, F(R_i))$ 点列で指定する．
    /// 点列は R で単調増加，F(R) でも単調非減少でなければならない．
    /// $R < R_0$ では $F = 0$，$R > R_n$ では $F = \text{pop\_max}$ にクリップする．
    PiecewiseLinear {
        points: Vec<(f64, f64)>,
        pop_max: f64,
    },
}

impl ToleranceSchedule {
    /// 集団総数 $\text{pop\_max}$ を返す．
    pub fn pop_max(&self) -> f64 {
        match *self {
            ToleranceSchedule::Linear { pop_max, .. } => pop_max,
            ToleranceSchedule::Affine { pop_max, .. } => pop_max,
            ToleranceSchedule::PiecewiseLinear { pop_max, .. } => pop_max,
        }
    }

    /// 累積分布 $F(R)$．許容限界が $R$ 以下の個人の人数を返す．
    /// $R < 0$ では 0 にクリップ．切片付き Affine では $F(0) = \text{intercept\_pop}$ で
    /// 「ゼロ許容者」の存在を表現できる．
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

    /// ソート仮定の下で，$n$ 人が残留しているときの周辺許容限界 $R(n)$．
    /// 最も寛容な者から残るので，残留者のうち最も不寛容な者の許容限界は
    /// $F(R(n)) = \text{pop\_max} - n$ を満たす．
    ///
    /// $n = 0$ のとき (誰も居ない) は $R = 0$ を返す．
    /// $n \ge \text{pop\_max}$ のとき (全員居る) は $F^{-1}(0) = 0$ を返す．
    pub fn marginal_tolerance(&self, n: f64) -> f64 {
        let pop_max = self.pop_max();
        if n <= 0.0 {
            // 誰も残っていなければ，「次に入る最も不寛容な者」の許容限界は最大 (R_max)
            // ただし全員退出済みは別の意味なので，BNM では n=0 の点は端点として扱う．
            // ここでは流入向けに最寛容者の許容限界を返すのが自然．
            return self.r_max_finite();
        }
        if n >= pop_max {
            return 0.0;
        }
        let target = pop_max - n; // F(R) = pop_max - n を満たす R
        self.invert_cdf(target)
    }

    /// 数値的に意味のある最大許容比率を返す．
    /// Linear/Affine では具体値，PiecewiseLinear では最大点の R．
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

    /// $F(R) = \text{target}$ を満たす最小の $R$ を返す (CDF の逆関数)．
    /// $\text{target} \le 0$ なら 0，$\text{target} \ge \text{pop\_max}$ なら $r\_max\_finite$．
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

    /// CDF をサンプリングして $(R, F(R))$ の点列を返す (CSV 出力用)．
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

    /// CLI/ログ出力用のラベル．
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
                format!("piecewise(n_points={}, pop_max={:.1})", points.len(), pop_max)
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
        assert!(approx(s.cdf(3.0), 100.0, 1e-9)); // クリップ
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
        // 中央で折れ曲がる例: (0,0)-(1,30)-(2,100)
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
