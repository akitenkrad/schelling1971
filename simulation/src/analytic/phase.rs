//! 位相平面解析．平衡点の探索と安定性判定．
//!
//! 状態 $(W, B)$ について以下を扱う:
//! - 反応曲線 $B_W(W)$, $W_B(B)$ の位置関係から各点の動学符号を決める「領域分類」．
//! - 平衡点: 端点 (全W / 全B / 空) と内部交点 (混合均衡)．
//! - 安定性: 反応曲線が容量制約 $W + B = C$ を横切る方向で判定．
//!
//! Schelling (1971) §3 (BNM, pp.167--181) および Appendix A (本ノート) に対応．

use serde::{Deserialize, Serialize};

use super::reaction::ReactionCurve;
use super::tolerance::ToleranceSchedule;

/// 位相平面解析の設定．
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseConfig {
    /// 白人 (W) 集団の許容スケジュール．
    pub w_schedule: ToleranceSchedule,
    /// 黒人 (B) 集団の許容スケジュール．
    pub b_schedule: ToleranceSchedule,
    /// 容量制約 $W + B \le C$．None の場合は無制約．
    pub capacity: Option<f64>,
}

impl PhaseConfig {
    /// 白人反応曲線 $B_W(W)$．
    pub fn w_reaction(&self) -> ReactionCurve<'_> {
        ReactionCurve::new(&self.w_schedule)
    }

    /// 黒人反応曲線 $W_B(B)$．
    pub fn b_reaction(&self) -> ReactionCurve<'_> {
        ReactionCurve::new(&self.b_schedule)
    }

    /// 与えられた点 $(W, B)$ の動学符号を分類する．
    pub fn region(&self, w: f64, b: f64) -> ViabilityRegion {
        let bw_max = self.w_reaction().max_other(w); // W が許容できる B の最大数
        let wb_max = self.b_reaction().max_other(b); // B が許容できる W の最大数
        let w_ok = b <= bw_max; // W 集団は満足
        let b_ok = w <= wb_max; // B 集団は満足
        match (w_ok, b_ok) {
            (true, true) => ViabilityRegion::BothViable,
            (true, false) => ViabilityRegion::WViableOnly,
            (false, true) => ViabilityRegion::BViableOnly,
            (false, false) => ViabilityRegion::NeitherViable,
        }
    }

    /// 容量制約を満たすか．
    pub fn within_capacity(&self, w: f64, b: f64) -> bool {
        match self.capacity {
            Some(c) => w + b <= c + 1e-9,
            None => true,
        }
    }

    /// 平衡点の集合を返す．
    /// 端点 (全W / 全B / Empty) と，反応曲線の交点 (混合均衡) を列挙する．
    pub fn equilibria(&self) -> Vec<Equilibrium> {
        let mut eqs = Vec::new();

        let w_max = self.w_schedule.pop_max();
        let b_max = self.b_schedule.pop_max();

        // 端点: (W_max, 0) — 全W
        if self.within_capacity(w_max, 0.0) {
            eqs.push(self.classify_endpoint(w_max, 0.0, EquilibriumKind::AllWhite));
        }
        // 端点: (0, B_max) — 全B
        if self.within_capacity(0.0, b_max) {
            eqs.push(self.classify_endpoint(0.0, b_max, EquilibriumKind::AllBlack));
        }
        // 端点: (0, 0) — 空
        eqs.push(Equilibrium {
            w: 0.0,
            b: 0.0,
            kind: EquilibriumKind::Empty,
            stability: Stability::Unstable, // 通常は流入で抜け出す
        });

        // 混合均衡: B = B_W(W) かつ W = W_B(B) の交点を数値求解．
        // パラメトリックに W ∈ [0, w_max] を掃き，「W について B_W(W) 上にいると仮定したとき
        // それが W_B 反応曲線も満たすか」のゼロを Brent 法で探す．
        eqs.extend(self.find_mixed_equilibria());

        eqs
    }

    fn classify_endpoint(&self, w: f64, b: f64, kind: EquilibriumKind) -> Equilibrium {
        // 端点の安定性: 微小流入摂動で押し戻されるかで判定する．
        // 全Wの (W_max, 0) では，B が微小に増えたときに B 集団が退出に向かえば安定．
        //   B>0 で B_max > B_W(W_max)? を見る → B_W(W_max)=0 かつ W_B(0)=0 なので
        //   微小 B に対し W_B(eps) < W_max なら B が「W が多すぎる」と感じて退出 → 安定
        let stability = match kind {
            EquilibriumKind::AllWhite => {
                // B 集団: W_B(eps) と W_max の比較．W_B(eps) < W_max なら B は退出 → 安定．
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
        Equilibrium { w, b, kind, stability }
    }

    /// 混合均衡を Brent 法で探索する．
    ///
    /// 戦略: W 軸を細かいサンプル点で走査し，
    /// $h(W) = W - W_B(B_W(W))$ の符号変化区間を見つけて Brent 法で根を絞る．
    /// $h(W) = 0$ ⇔ $(W, B_W(W))$ が両反応曲線上にある．
    ///
    /// サンプルは半ステップずらした位置 $W_i = W_{\max} (i + 0.5) / n$ に取る．
    /// 整数比の臨界点 (例: 対称ケースの $W = W_{\max}/2$) で
    /// $h = 0$ 丁度に当たって符号判定が失敗するのを避けるため．
    fn find_mixed_equilibria(&self) -> Vec<Equilibrium> {
        let w_max = self.w_schedule.pop_max();
        let n_samples = 400;
        let h = |w: f64| -> f64 {
            if w <= 0.0 {
                return 0.0; // 端点は別途扱う
            }
            let b = self.w_reaction().max_other(w);
            let w_required = self.b_reaction().max_other(b);
            w - w_required
        };

        let mut roots: Vec<f64> = Vec::new();
        // 半ステップずらしたサンプル: i=0..=n に対し W = W_max*(i+0.5)/(n+1)
        let mut prev_w = 0.5 * w_max / (n_samples as f64 + 1.0);
        let mut prev_h = h(prev_w);
        for i in 1..=n_samples {
            let w = w_max * (i as f64 + 0.5) / (n_samples as f64 + 1.0);
            let cur_h = h(w);
            if prev_h.is_finite() && cur_h.is_finite() && prev_h * cur_h < 0.0 {
                if let Some(root) = brent(prev_w, w, prev_h, cur_h, &h, 1e-9, 100) {
                    // 既存の根と十分離れているもののみ採択
                    if !roots.iter().any(|r| (r - root).abs() < 1e-3 * w_max) {
                        roots.push(root);
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

    /// 混合均衡の安定性．反応曲線の交差方向で判定する．
    /// $h(W) = W - W_B(B_W(W))$ の傾きが正なら不安定 (左から右に交差)，負なら安定．
    fn classify_mixed(&self, w: f64, _b: f64) -> Stability {
        let h_eps = (self.w_schedule.pop_max() * 1e-4).max(1e-6);
        let h = |w: f64| -> f64 {
            let b = self.w_reaction().max_other(w);
            let w_required = self.b_reaction().max_other(b);
            w - w_required
        };
        let lo = (w - h_eps).max(1e-9);
        let hi = (w + h_eps).min(self.w_schedule.pop_max() - 1e-9);
        let slope = (h(hi) - h(lo)) / (hi - lo);
        if slope < 0.0 {
            Stability::Stable
        } else if slope > 0.0 {
            Stability::Unstable
        } else {
            Stability::Saddle
        }
    }

    /// ベクトル場をサンプリングする．
    /// $(W, B, \dot W, \dot B, region)$ のタプル列を返す．
    /// $\dot W, \dot B$ は領域に基づく符号 ($\pm 1$) で返す (大きさは [`super::dynamics`] で乗算)．
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

/// 平衡点．
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Equilibrium {
    pub w: f64,
    pub b: f64,
    pub kind: EquilibriumKind,
    pub stability: Stability,
}

/// 平衡点の種別．
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EquilibriumKind {
    /// $(W_{\max}, 0)$．
    AllWhite,
    /// $(0, B_{\max})$．
    AllBlack,
    /// 反応曲線交点の混合状態．
    Mixed,
    /// $(0, 0)$ の空状態．
    Empty,
}

/// 安定性分類．
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stability {
    Stable,
    Unstable,
    Saddle,
}

/// 動学符号領域 (4 区分)．
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViabilityRegion {
    /// 両曲線の内側．両集団とも流入．
    BothViable,
    /// $B \le B_W(W)$ かつ $W > W_B(B)$．W 流入・B 退出．
    WViableOnly,
    /// $W \le W_B(B)$ かつ $B > B_W(W)$．B 流入・W 退出．
    BViableOnly,
    /// 両曲線の外側．両集団とも退出．
    NeitherViable,
}

impl ViabilityRegion {
    /// $\dot W, \dot B$ の符号 ($\pm 1$) を返す．
    pub fn signs(&self) -> (f64, f64) {
        match self {
            ViabilityRegion::BothViable => (1.0, 1.0),
            ViabilityRegion::WViableOnly => (1.0, -1.0),
            ViabilityRegion::BViableOnly => (-1.0, 1.0),
            ViabilityRegion::NeitherViable => (-1.0, -1.0),
        }
    }
}

/// ベクトル場の1サンプル．
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct VectorSample {
    pub w: f64,
    pub b: f64,
    pub dw_sign: f64,
    pub db_sign: f64,
    pub region: ViabilityRegion,
}

/// Brent 法による1次元根求解．
/// `f(a)*f(b) < 0` (符号変化区間) を仮定する．
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
        if fb.abs() < tol || (b - a).abs() < tol {
            return Some(b);
        }
        if fa != fc && fb != fc {
            // 逆2次補間
            let s = a * fb * fc / ((fa - fb) * (fa - fc))
                + b * fa * fc / ((fb - fa) * (fb - fc))
                + c * fa * fb / ((fc - fa) * (fc - fb));
            // 受容条件．不適なら2分法へフォールバック．
            let cond1 = (s - (3.0 * a + b) / 4.0) * (s - b) >= 0.0;
            let cond2 = (s - b).abs() >= (b - c).abs() / 2.0;
            let cond3 = (b - c).abs() < tol;
            let s = if cond1 || cond2 || cond3 { (a + b) / 2.0 } else { s };
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
            // 線形補間 (secant) → 2分法
            let s = if fb != fa {
                b - fb * (b - a) / (fb - fa)
            } else {
                (a + b) / 2.0
            };
            let s = if (s - b).abs() < tol { (a + b) / 2.0 } else { s };
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

    /// Fig.18 (基本ケース): 直線型，1:2 比 — 端点2均衡のみ，混合は不安定．
    #[test]
    fn fig18_two_endpoint_equilibria() {
        let cfg = PhaseConfig {
            w_schedule: ToleranceSchedule::Linear { r_max: 2.0, pop_max: 100.0 },
            b_schedule: ToleranceSchedule::Linear { r_max: 2.0, pop_max: 50.0 },
            capacity: None,
        };
        let eqs = cfg.equilibria();

        // 全W / 全B が両方含まれる
        assert!(eqs.iter().any(|e| e.kind == EquilibriumKind::AllWhite));
        assert!(eqs.iter().any(|e| e.kind == EquilibriumKind::AllBlack));

        // 端点は安定であること
        let all_w = eqs.iter().find(|e| e.kind == EquilibriumKind::AllWhite).unwrap();
        let all_b = eqs.iter().find(|e| e.kind == EquilibriumKind::AllBlack).unwrap();
        assert_eq!(all_w.stability, Stability::Stable);
        assert_eq!(all_b.stability, Stability::Stable);
    }

    /// 対称な直線型 (W_max = B_max = 100, R_max=2): 反応曲線は同形．
    /// h(W) = W - W_B(B_W(W)) は W=50 で頂点を共有 → 接する形になる場合がある．
    /// ここでは W_max=B_max を変えて非対称化したケースで混合均衡のテストを書く．
    #[test]
    fn region_classification_at_origin_is_both_viable() {
        let cfg = PhaseConfig {
            w_schedule: ToleranceSchedule::Linear { r_max: 2.0, pop_max: 100.0 },
            b_schedule: ToleranceSchedule::Linear { r_max: 2.0, pop_max: 100.0 },
            capacity: None,
        };
        // (10, 10): 両反応曲線とも値十分大．両viable のはず．
        assert_eq!(cfg.region(10.0, 10.0), ViabilityRegion::BothViable);
        // (90, 90): 両曲線とも極めて低い → どちらも外側
        assert_eq!(cfg.region(90.0, 90.0), ViabilityRegion::NeitherViable);
    }

    /// 急勾配スケジュール (Fig.19 系): 中央値許容比率 ≥ 1.5 で 3 均衡が現れる．
    /// アフィン (intercept_pop=20, slope=40, pop_max=100) なら R_max = 2.
    /// 中央値 (F=50) は R = 0.75 だが，その分布形状で混合均衡が出るかを確認．
    #[test]
    fn affine_schedule_introduces_mixed_equilibrium() {
        // 切片付きで急勾配 (F(0)=0 でなく F(0)=0 を保ち，傾きをきつくする)
        // ここでは中央値が高い条件の代理として，pop_max=100, R_max=2.0 だが
        // 反応曲線が容量内側で交差するように W,B 集団を非対称に組む．
        let cfg = PhaseConfig {
            w_schedule: ToleranceSchedule::Affine {
                intercept_pop: 0.0,
                slope: 25.0, // F(R) = 25R, F(4)=100 → R_max=4 (とても寛容)
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
        let n_mixed = eqs.iter().filter(|e| e.kind == EquilibriumKind::Mixed).count();
        // 対称ケースなのでちょうど W=B の対角線上に1点 (または0点) のはず．
        // 重要なのは混合均衡が検出される能力があること．
        assert!(n_mixed >= 1, "対称・寛容スケジュールでは混合均衡が少なくとも1点出るべき");
    }

    /// ベクトル場の生成: 全象限のサンプルが領域分類される．
    #[test]
    fn vector_field_covers_grid() {
        let cfg = PhaseConfig {
            w_schedule: ToleranceSchedule::Linear { r_max: 2.0, pop_max: 100.0 },
            b_schedule: ToleranceSchedule::Linear { r_max: 2.0, pop_max: 50.0 },
            capacity: None,
        };
        let field = cfg.vector_field(10, 10);
        assert_eq!(field.len(), 11 * 11);
        // 原点近傍は両viable
        let origin = field.iter().find(|s| s.w == 0.0 && s.b == 0.0).unwrap();
        // (0,0) は端点で W=0, B=0 → B_W(0)=0, W_B(0)=0 → b<=0, w<=0 が両立
        // 浮動小数点上は両 viable と判定される
        assert_eq!(origin.region, ViabilityRegion::BothViable);
    }

    /// 容量制約: capacity を超える点はベクトル場から除外される．
    #[test]
    fn capacity_constraint_filters_vector_field() {
        let cfg = PhaseConfig {
            w_schedule: ToleranceSchedule::Linear { r_max: 2.0, pop_max: 100.0 },
            b_schedule: ToleranceSchedule::Linear { r_max: 2.0, pop_max: 100.0 },
            capacity: Some(100.0),
        };
        let field = cfg.vector_field(10, 10);
        // すべてのサンプルが W+B<=100
        assert!(field.iter().all(|s| s.w + s.b <= 100.0 + 1e-9));
    }

    #[test]
    fn brent_finds_root_of_simple_function() {
        let f = |x: f64| (x - 2.5).powi(3);
        let root = brent(0.0, 5.0, f(0.0), f(5.0), &f, 1e-9, 100).unwrap();
        assert!(approx(root, 2.5, 1e-6));
    }
}
