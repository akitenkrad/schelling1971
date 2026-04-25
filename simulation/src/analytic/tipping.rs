//! ティッピングモデル (Schelling 1971 §4, pp.181--186)．
//!
//! 境界近隣モデル (BNM) を住宅市場に適用したもの．BNM の基本動学に以下を追加する:
//! - **投機的退出 (Speculation)**: 期待値ベースの早期退出により，現在比率が許容内でも将来予測で退出．
//! - **流速の非対称性 (FlowAsymmetry)**: 流入・退出の速度が色・方向で異なる．
//! - **チャネリング (channeling)**: 境界明確な小規模近隣ではティッピングが集中する効果を，
//!   実効容量の縮小として表現．
//! - **ティッピング類型分類 (TippingType)**: in-tipping / out-tipping の有無で 4 類型に分類．

use serde::{Deserialize, Serialize};

use super::dynamics::{integrate, DynamicsConfig, FlowModel, Trajectory, TrajectoryPoint};
use super::phase::{EquilibriumKind, PhaseConfig};
use super::reaction::ReactionCurve;

// ---------------------------------------------------------------------------
// 投機的退出
// ---------------------------------------------------------------------------

/// 期待形成モデル．
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Speculation {
    /// 投機なし．許容判定は現在比率のみ．
    None,
    /// 線形外挿: 期待 $B_t^e = B_t + \alpha \cdot \dot B_{t-1}$．
    /// 許容判定で $B_t$ の代わりに $B_t^e$ を使い，将来悪化を予測した早期退出を許す．
    Linear { alpha: f64 },
    /// 過去 window ステップから線形回帰で外挿．weight は 0..=1 で，
    /// $B_t^e = B_t + \text{weight} \cdot \text{trend}$．
    Trend { window: usize, weight: f64 },
}

impl Default for Speculation {
    fn default() -> Self {
        Speculation::None
    }
}

// ---------------------------------------------------------------------------
// 流速非対称性
// ---------------------------------------------------------------------------

/// 流入・退出の速度を色・方向別に指定する．
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
// ティッピング設定
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TippingConfig {
    pub phase: PhaseConfig,
    pub dynamics: DynamicsConfig,
    pub speculation: Speculation,
    pub asymmetry: Option<FlowAsymmetry>,
    /// 実効容量を `phase.capacity * channeling` に縮小する．
    /// None または 1.0 で無効．小さな値ほど近隣境界が明確で，ティッピングが起きやすい．
    pub channeling: Option<f64>,
}

impl TippingConfig {
    /// 軌跡を積分する．投機項と非対称性は dynamics::integrate ではなく専用ルーチンで処理．
    pub fn integrate(&self, init: (f64, f64)) -> Trajectory {
        // 現状: 投機・非対称が未指定なら BNM の積分にフォールバック．
        // 容量制約は channeling 適用後の値を使う．
        let mut phase = self.phase.clone();
        if let Some(c) = self.channeling {
            if (0.0..=1.0).contains(&c) {
                phase.capacity = phase.capacity.map(|cap| cap * c);
            }
        }

        match (self.speculation, self.asymmetry) {
            (Speculation::None, None) => integrate(&phase, &self.dynamics, init),
            _ => self.integrate_with_extensions(&phase, init),
        }
    }

    fn integrate_with_extensions(&self, phase: &PhaseConfig, init: (f64, f64)) -> Trajectory {
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

        // 期待形成のための過去履歴 (window 用)
        let mut prev_w = w;
        let mut prev_b = b;

        for step in 0..self.dynamics.max_steps {
            // 期待値 (投機モデル適用)
            let (b_eff, w_eff) = self.apply_speculation(&history, prev_w, prev_b, w, b);

            // 反応曲線目標．不在色は実効値で評価
            let w_target = directional_target(&phase.w_reaction(), b_eff, w, w_max);
            let b_target = directional_target(&phase.b_reaction(), w_eff, b, b_max);

            // 流速 (非対称性)
            let asym = self.asymmetry.unwrap_or_default();
            let w_rate = if w_target >= w {
                k_w * asym.w_inflow * (w_target - w)
            } else {
                k_w * asym.w_outflow * (w_target - w) // 負の値
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
            prev_w = w;
            prev_b = b;
            w = w_next;
            b = b_next;
            t += dt;
            history.push(TrajectoryPoint { t, w, b });

            if delta < self.dynamics.convergence_tol {
                converged = true;
                converged_step = Some(step + 1);
                break;
            }
        }

        // 終点に最も近い平衡点 (ヘルパは dynamics 側 private なので簡易再実装)
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

    /// 投機項により「許容判定に使う相手色の実効値」を計算する．
    /// 戻り値は (B 効値，W 効値)．通常の BNM では (B, W) と一致．
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

// dynamics::directional_target は private なのでここで再利用するため公開ラッパを用意
fn directional_target(rc: &ReactionCurve, other_now: f64, own_now: f64, own_pop_max: f64) -> f64 {
    if other_now <= 0.0 {
        return own_pop_max;
    }
    let (w_peak, b_peak) = rc.peak();
    if other_now > b_peak {
        return 0.0;
    }
    // 上側根
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
    // 下側根
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
// ティッピング類型分類
// ---------------------------------------------------------------------------

/// ティッピング類型 (Schelling Fig.30--32 の 4 類型)．
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TippingType {
    /// in-tipping のみ: 全W 端点が不安定で，少数派 (B) が自然流入を始める．
    InTippingOnly,
    /// out-tipping のみ: 全W 端点は安定だが，B が閾値超過で W が連鎖退出する．
    OutTippingOnly,
    /// 両方: 自然流入と退出連鎖の両方が起きる (典型的ホワイトフライト)．
    Both,
    /// 両方なし: 安定混合均衡が存在し，端点も安定．
    Neither,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TippingClassification {
    pub tipping_type: TippingType,
    pub all_white_stable: bool,
    pub mixed_stable_exists: bool,
}

/// 反応曲線の幾何から in/out tipping の有無を判定する．
///
/// **In-tipping (幾何条件)**: 黒人反応曲線 $W_B(B)$ の頂点が $W_{\max}$ 以上．
/// すなわち $\max_B W_B(B) \ge W_{\max}$．これが成り立てば，ある B 値で
/// 「W = W_max でも B が許容できる」状態が存在し，その水準を超える B 流入が
/// 自己持続的になる ($B$ がもう少し増えれば $W$ が減らせる)．
///
/// **Out-tipping**: 安定な混合均衡が**存在しない**こと．
/// 安定混合があれば B 流入は混合均衡で止まり連鎖退出は起きない．
/// 安定混合がなければ，B が閾値を越えると W が連鎖退出する．
pub fn classify_tipping(phase: &PhaseConfig) -> TippingClassification {
    let eqs = phase.equilibria();

    // 全W 端点の安定性 (情報用: 線形化に基づく)
    let all_white = eqs.iter().find(|e| e.kind == EquilibriumKind::AllWhite);
    let all_white_stable = all_white
        .map(|e| e.stability == super::phase::Stability::Stable)
        .unwrap_or(true);

    // 安定混合均衡の存在
    let mixed_stable_exists = eqs.iter().any(|e| {
        e.kind == EquilibriumKind::Mixed && e.stability == super::phase::Stability::Stable
    });

    // 幾何 in-tipping: B 反応曲線の頂点が W_max 以上 (B が W_max を覆える経路あり)
    let w_max = phase.w_schedule.pop_max();
    let (_, b_curve_peak_w) = phase.b_reaction().peak();
    let in_tipping = b_curve_peak_w >= w_max - 1e-9;

    // 幾何 out-tipping: 安定混合がなければ B が閾値超過で W は連鎖退出
    let out_tipping = !mixed_stable_exists;

    let tipping_type = match (in_tipping, out_tipping) {
        (true, true) => TippingType::Both,
        (true, false) => TippingType::InTippingOnly,
        (false, true) => TippingType::OutTippingOnly,
        (false, false) => TippingType::Neither,
    };

    TippingClassification {
        tipping_type,
        all_white_stable,
        mixed_stable_exists,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytic::tolerance::ToleranceSchedule;

    fn fig18_phase() -> PhaseConfig {
        PhaseConfig {
            w_schedule: ToleranceSchedule::Linear { r_max: 2.0, pop_max: 100.0 },
            b_schedule: ToleranceSchedule::Linear { r_max: 2.0, pop_max: 50.0 },
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

    /// Fig.18: 全W 安定 + 混合不安定 → out-tipping のみ (B が閾値超過で W が連鎖退出)．
    #[test]
    fn fig18_classifies_as_out_tipping() {
        let phase = fig18_phase();
        let cls = classify_tipping(&phase);
        assert_eq!(cls.tipping_type, TippingType::OutTippingOnly);
        assert!(cls.all_white_stable);
        assert!(!cls.mixed_stable_exists);
    }

    /// Fig.19: 全W 安定 + 安定混合あり → どちらの ティッピングもなし．
    #[test]
    fn fig19_classifies_as_neither() {
        let phase = fig19_phase();
        let cls = classify_tipping(&phase);
        assert_eq!(cls.tipping_type, TippingType::Neither);
        assert!(cls.all_white_stable);
        assert!(cls.mixed_stable_exists);
    }

    /// 投機なし・非対称なし → BNM と同じ軌跡．
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

    /// 投機あり: alpha=0.5 程度なら依然として正しい収束先に到達する (発散しない)．
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
        // 終点は端点近くにあること
        assert!(last.w > 80.0 || last.b > 40.0);
    }

    /// 流速非対称: B の inflow を遅くすると，B 流入が遅延しすぎて
    /// W に押し負けて all_white に向かう傾向が強まる (極端なケース)．
    #[test]
    fn asymmetric_flow_changes_outcome_for_borderline_init() {
        // 投機なし・流入 W=2.0, B=0.1 → W 圧倒的優勢
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
        // 中央付近の初期条件でも，W優勢の流入で all_white に向かう
        let last = traj.history.last().unwrap();
        assert!(last.w > last.b);
    }
}
