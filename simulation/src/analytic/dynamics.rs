//! 動学エンジン．位相平面 $(W, B)$ 上で時間発展を行い，軌跡と収束先を返す．
//!
//! 2つの実装を提供する:
//! - [`FlowModel::Continuous`]: 連続時間 ODE．$\dot W, \dot B$ の符号は領域分類で決まり，
//!   大きさは流速係数 $k_W, k_B$ と現在地から目標反応曲線への距離に比例．Euler 法で離散化．
//! - [`FlowModel::DiscreteBatch`]: 各ステップで超過分を一括退出 / 余裕分を一括流入させる
//!   論文の物語的記述に近い形式．

use serde::{Deserialize, Serialize};

use super::phase::{Equilibrium, EquilibriumKind, PhaseConfig};
use super::reaction::ReactionCurve;

/// 流速モデル．
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FlowModel {
    /// 連続時間 Euler．現在地から反応曲線までの「距離」に比例する流速．
    /// $\dot W = k_W \cdot \text{sign}(B_W(W) - B) \cdot |B_W(W) - B|^{0+}$ 等．
    /// 簡略化: 距離そのものを流速とし，$\dot W = k_W \cdot \text{sign}_W \cdot |\text{distance}|$．
    Continuous { k_w: f64, k_b: f64, dt: f64 },

    /// 離散バッチ．各ステップで:
    /// - $B > B_W(W)$ なら，許容できなくなった W を超過分一括退出．
    /// - $W \le W_B(B)$ かつ余裕があれば，外部の W を流入．
    /// - B についても対称．
    DiscreteBatch,
}

impl Default for FlowModel {
    fn default() -> Self {
        FlowModel::Continuous { k_w: 1.0, k_b: 1.0, dt: 0.1 }
    }
}

/// 動学設定．
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DynamicsConfig {
    pub flow: FlowModel,
    pub max_steps: usize,
    /// 収束判定: 連続2ステップの $\|(\Delta W, \Delta B)\|_\infty < \text{convergence\_tol}$．
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

/// 軌跡: 各時刻における $(W, B)$ の履歴．
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

/// 初期値 $(W_0, B_0)$ から動学積分を行う．
pub fn integrate(phase: &PhaseConfig, cfg: &DynamicsConfig, init: (f64, f64)) -> Trajectory {
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

        // 容量制約．超過した場合は比例配分でクリップ．
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
        w = w_next;
        b = b_next;
        t += dt;
        history.push(TrajectoryPoint { t, w, b });

        if delta < cfg.convergence_tol {
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

/// 1ステップの $(\dot W \cdot dt, \dot B \cdot dt, dt)$ を返す．
///
/// 動学の解釈 (Schelling 1971 §3):
/// 反応曲線 $B_W(W)$ の上で「満足区間」 $W \in [W_{lower}(B), W_{upper}(B)]$ を定める．
/// 区間内なら最も寛容な外部 W が流入し，区間上限 $W_{upper}$ へ漸近する．
/// 区間外（下側＝過少人口で異色比過多 / 上側＝過剰人口で許容限界違反）なら退出する：
/// - $W < W_{lower}$: 0 へ向かう (満足にできるほど W が増えない / 過少すぎ)．
/// - $W > W_{upper}$: $W_{upper}$ へ向かう (過剰分が退出)．
/// - 反応曲線頂点を $B$ が超える場合: $W$ は全退出 → 0．
///
/// この形式により: (i) 端点均衡には漸近接近で安定収束，(ii) 反応曲線交差点 (混合均衡)
/// には双方向から滑らかに収束 (chattering なし)，(iii) 鞍点は線形化で不安定．
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
            // 1ステップで目標値に直接ジャンプ．
            (w_target - w, b_target - b, 1.0)
        }
    }
}

/// 動学の到達目標．
/// `own_now` の位置と「満足区間」 $[W_{lower}, W_{upper}]$ の関係で行き先が決まる．
fn directional_target(rc: &ReactionCurve, other_now: f64, own_now: f64, own_pop_max: f64) -> f64 {
    // other_now <= 0 なら制約なし → 全人口へ向かう
    if other_now <= 0.0 {
        return own_pop_max;
    }
    let (w_peak, b_peak) = rc.peak();
    if other_now > b_peak {
        // 反応曲線頂点を超える対色数 → どの W でも満足できない → 全退出
        return 0.0;
    }
    let upper = upper_root(rc, other_now, own_pop_max, w_peak);
    let lower = lower_root(rc, other_now, w_peak);
    if own_now < lower {
        // 満足区間より下側 → 過少 / 過密ratio で全員不満足 → 0 へ
        0.0
    } else {
        // 区間内または上側 → 上限 W_upper へ向かう
        // 区間内: 流入で増加して W_upper に至る．
        // 区間上側: 過剰分退出で W_upper まで戻る．
        upper
    }
}

/// 反応曲線 $rc(W) = \text{target}$ を満たす上側根 (頂点より右側の解)．
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

/// 反応曲線 $rc(W) = \text{target}$ を満たす下側根 (頂点より左側の解)．
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

/// 終点に最も近い平衡点を返す．
fn nearest_equilibrium(phase: &PhaseConfig, w: f64, b: f64) -> Option<Equilibrium> {
    let eqs = phase.equilibria();
    let scale = (phase.w_schedule.pop_max() + phase.b_schedule.pop_max()).max(1.0);
    eqs.into_iter()
        .map(|e| {
            let d2 = (e.w - w).powi(2) + (e.b - b).powi(2);
            (d2, e)
        })
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
        .filter(|(d2, _)| d2.sqrt() < 0.05 * scale) // 5%以内
        .map(|(_, e)| e)
}

/// 初期条件グリッドを掃いて吸引域マップを構築する．
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

pub fn basin_of_attraction(
    phase: &PhaseConfig,
    cfg: &DynamicsConfig,
    n_w: usize,
    n_b: usize,
) -> Vec<BasinSample> {
    let w_max = phase.w_schedule.pop_max();
    let b_max = phase.b_schedule.pop_max();
    let mut out = Vec::with_capacity((n_w + 1) * (n_b + 1));
    for i in 0..=n_w {
        for j in 0..=n_b {
            let w0 = w_max * (i as f64) / (n_w as f64);
            let b0 = b_max * (j as f64) / (n_b as f64);
            if !phase.within_capacity(w0, b0) {
                continue;
            }
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
    }
    out
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

    /// Fig.18: 初期 (90, 5) は全W 端点に収束するはず (B が圧倒的少数)．
    #[test]
    fn fig18_high_white_converges_to_all_white() {
        let phase = fig18_phase();
        let cfg = DynamicsConfig::default();
        let traj = integrate(&phase, &cfg, (90.0, 5.0));
        assert!(traj.converged, "収束すべき");
        let last = traj.history.last().unwrap();
        assert!(last.w > 80.0, "全W 端点付近に到達: w={}", last.w);
        assert!(last.b < 5.0, "B はほぼゼロ: b={}", last.b);
        assert_eq!(
            traj.final_equilibrium.map(|e| e.kind),
            Some(EquilibriumKind::AllWhite)
        );
    }

    /// Fig.18: 初期 (5, 40) は全B 端点に収束するはず (W が圧倒的少数)．
    #[test]
    fn fig18_high_black_converges_to_all_black() {
        let phase = fig18_phase();
        let cfg = DynamicsConfig::default();
        let traj = integrate(&phase, &cfg, (5.0, 40.0));
        assert!(traj.converged);
        let last = traj.history.last().unwrap();
        assert!(last.b > 40.0, "全B 端点付近に到達: b={}", last.b);
        assert!(last.w < 5.0, "W はほぼゼロ: w={}", last.w);
        assert_eq!(
            traj.final_equilibrium.map(|e| e.kind),
            Some(EquilibriumKind::AllBlack)
        );
    }

    /// 直線型の対称ケース: 初期条件を変えるとどちらかの端点に振れるが，
    /// 直線型では Schelling のいう「混合は静的には可能だが動的に不安定」が成立する．
    #[test]
    fn discrete_batch_converges_in_few_steps() {
        let phase = fig18_phase();
        let cfg = DynamicsConfig {
            flow: FlowModel::DiscreteBatch,
            max_steps: 50,
            convergence_tol: 1e-3,
        };
        let traj = integrate(&phase, &cfg, (50.0, 25.0));
        // バッチ型は1〜数ステップで端点に到達するはず
        assert!(traj.history.len() <= 10);
    }

    /// 容量制約を入れたケース: capacity を超えない．
    #[test]
    fn capacity_constraint_respected() {
        let phase = PhaseConfig {
            w_schedule: ToleranceSchedule::Linear { r_max: 2.0, pop_max: 100.0 },
            b_schedule: ToleranceSchedule::Linear { r_max: 2.0, pop_max: 100.0 },
            capacity: Some(120.0),
        };
        let cfg = DynamicsConfig::default();
        let traj = integrate(&phase, &cfg, (60.0, 50.0));
        for p in &traj.history {
            assert!(p.w + p.b <= 120.0 + 1e-6, "容量超過: w+b={}", p.w + p.b);
        }
    }

    /// 吸引域: 4 隅サンプルで適切に分類される．
    #[test]
    fn basin_sample_identifies_endpoints() {
        let phase = fig18_phase();
        let cfg = DynamicsConfig::default();
        let basin = basin_of_attraction(&phase, &cfg, 4, 4);
        assert!(!basin.is_empty());
        // 少なくとも1点が AllWhite に，1点が AllBlack に収束する
        let has_white = basin.iter().any(|s| s.converged_kind == Some(EquilibriumKind::AllWhite));
        let has_black = basin.iter().any(|s| s.converged_kind == Some(EquilibriumKind::AllBlack));
        assert!(has_white && has_black, "両端点への吸引域が観測されること");
    }
}
