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
        Equilibrium {
            w,
            b,
            kind,
            stability,
        }
    }

    /// 混合均衡を Brent 法で探索する．
    ///
    /// 戦略: W 軸を細かいサンプル点で走査し，
    /// $h(W) = W - W_B(B_W(W))$ の符号変化区間を見つけて Brent 法で根を絞る．
    /// $h(W) = 0$ ⇔ $(W, B_W(W))$ が両反応曲線上にある．
    ///
    /// サンプルは半ステップずらした位置 $W_i = W_{\max} (i + 0.5) / (n + 1)$ に取る．
    /// ただしこのずらしだけでは不十分で，`n_samples = 400` のとき $i = 200$ は
    /// $W = W_{\max} / 2$ を厳密に踏む．対称ケース (例: $R_{\max} = 2$ の直線型で
    /// 交点が $(50, 50)$) ではそこが根そのものになり，$h = 0$ ちょうどのため
    /// `prev_h * cur_h < 0` が成立せず検出漏れになる．
    /// そこで符号変化に加えて **サンプル点上の厳密な零点** も root として拾う．
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
        // 既存の根と十分離れているもののみ採択する．
        let push_root = |roots: &mut Vec<f64>, root: f64| {
            if !roots.iter().any(|r: &f64| (r - root).abs() < 1e-3 * w_max) {
                roots.push(root);
            }
        };

        // 半ステップずらしたサンプル: i=0..=n に対し W = W_max*(i+0.5)/(n+1)
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
                    // サンプル点が根を厳密に踏んだケース．
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

    /// 混合均衡の安定性．反応曲線の交差方向 ($h$ が根を横切る向き) で判定する．
    /// $h(W) = W - W_B(B_W(W))$ が減少しながら根を横切れば安定，増加しながらなら不安定．
    ///
    /// 非縮退な場合これは $h'(W^*) = 1 - B_W' W_B' < 0$，すなわち流れ場のヤコビ行列の
    /// $\det J > 0$ と同値である (流れ場のトレースは常に $-(k_W + k_B) < 0$ なので
    /// 行列式の符号だけで安定性が決まる)．
    ///
    /// **縮退ケースの扱い**: $B_W'(W^*) W_B'(B^*) = 1$ ちょうどのとき $h'(W^*) = 0$ となり
    /// 線形化では判定できない (零固有値)．対称アフィン $F = c + sR$ で $R_{\max} = 3$ の
    /// とき，まさにこれが起きる (fig20 / fig25)．このとき $h$ は $W^*$ で3位の零点をもち
    /// $h(W^* + x) = 2x^3/s^2 + O(x^4)$ となる．中心多様体 $u = -v^2/(4s)$ 上へ縮約すると
    /// $v = W - B$ の従う方程式は $\dot v = \frac{k}{4s^2} v^3 + O(v^4)$ で，係数が正なので
    /// **不安定** (ただし発散は指数的でなく代数的で，$t^* = 1/(2Cv_0^2)$ で有限時間発散する)．
    ///
    /// 3位の零点は奇数位なので $h$ は根の前後で符号を変える．したがって
    /// **傾きの値ではなく符号パターンで判定すれば**，非縮退ケースと同じ規則のまま
    /// 縮退ケースも正しく解決できる．割線の傾き $(h(hi)-h(lo))/(hi-lo)$ を使うと
    /// 縮退時の値が $O(h\_eps^2)$ と極端に小さくなり，`h_eps` を詰めるほど丸め誤差に
    /// 埋もれてしまうため，ここでは商を取らず符号のみを見る．
    ///
    /// 根の両側で $h$ が同符号になるのは偶数位の零点で，これは片側安定 (半安定) なので
    /// [`Stability::Saddle`] を返す．
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
            // 減少しながら根を横切る → 安定．
            Stability::Stable
        } else if h_lo < 0.0 && h_hi > 0.0 {
            // 増加しながら根を横切る → 不安定 (3次縮退ケースもここに入る)．
            Stability::Unstable
        } else {
            // 同符号 = 偶数位の零点 (片側安定) / 数値的に判定不能．
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
/// `tol` は **区間幅** に対する許容誤差 (根の位置の精度) として解釈される．
///
/// 収束判定に関数値 `|f(b)| < tol` を併用してはならない．重根では $f$ が根の周りで
/// 極端に平坦になり，根から遠い点でも $|f|$ が小さくなるため精度が出ないためである．
/// 実際 $h$ が3位の零点をもつ縮退ケース ($R_{\max} = 3$ の対称アフィン) では
/// $h \approx 2x^3/s^2$ なので，`tol = 1e-9` に対し $|h| < $ `tol` は $|x| < 8.2\times10^{-3}$
/// を意味してしまい，根の位置が3桁も甘くなる．区間幅で判定すれば
/// 符号が信頼できる限り2分法が効くので，桁落ちで $h$ の符号が潰れる
/// $|x| \approx 2\times10^{-4}$ 付近まで詰められる．
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
            // 逆2次補間
            let s = a * fb * fc / ((fa - fb) * (fa - fc))
                + b * fa * fc / ((fb - fa) * (fb - fc))
                + c * fa * fb / ((fc - fa) * (fc - fb));
            // 受容条件．不適なら2分法へフォールバック．
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
            // 線形補間 (secant) → 2分法
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

    /// 縮退ケース (fig20 / fig25): 対称アフィン $F = c + sR$ で $R_{\max} = 3$ のとき
    /// $B_W'(W^*) W_B'(B^*) = 1$ ちょうどとなり $\det J = 0$，線形化では判定できない．
    /// $h$ は3位の零点をもち $h(W^* + x) = 2x^3/s^2$ となるため，中心多様体上の
    /// 縮約 $\dot v = \frac{k}{4s^2}v^3$ (係数 > 0) から **不安定** が正しい．
    ///
    /// `classify_mixed` は割線の値ではなく符号パターンで判定するのでこれを解決できる．
    /// 商を取る実装に戻すと縮退時の傾きが $O(h\_eps^2)$ となり丸め誤差に埋もれるため，
    /// この回帰テストで固定する．
    #[test]
    fn degenerate_r_max_3_mixed_equilibrium_is_unstable() {
        // fig20 相当: 直線型 R_max=3 (c=0, s=100/3, M=100) -> W* = M - s = 66.67
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
        assert_eq!(mixed.len(), 1, "混合均衡は1つ: {mixed:?}");
        // 3重根なので根の位置そのものの精度は落ちる (下記 fig25 のコメント参照)．
        assert!(approx(mixed[0].w, 200.0 / 3.0, 1e-2), "W*={}", mixed[0].w);
        assert_eq!(
            mixed[0].stability,
            Stability::Unstable,
            "det J = 0 の縮退点だが3次項により不安定"
        );

        // fig25 相当: アフィン F = 10 + 30R (M=90, s=30, R_max=3) -> W* = 60
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
        assert_eq!(mixed.len(), 1, "混合均衡は1つ: {mixed:?}");
        // 3重根でも [`brent`] が区間幅で収束判定する限りこの精度が出る
        // (関数値 |f| < tol で打ち切ると $10^{-3}$ 程度までしか詰まらない)．
        assert!(approx(mixed[0].w, 60.0, 1e-3), "W*={}", mixed[0].w);
        assert_eq!(mixed[0].stability, Stability::Unstable);
    }

    /// 重根での根の精度が [`brent`] の収束判定に依存することを固定する．
    /// 関数値ベースの打ち切り (`|f(b)| < tol`) に戻すと縮退ケースの精度が3桁落ちる．
    #[test]
    fn brent_resolves_triple_root_accurately() {
        // h(x) = 2(x - 2.5)^3 / s^2 型の平坦な3重根．
        let s: f64 = 100.0 / 3.0;
        let f = |x: f64| 2.0 * (x - 2.5).powi(3) / (s * s);
        let root = brent(0.0, 5.0, f(0.0), f(5.0), &f, 1e-9, 200).unwrap();
        assert!(
            approx(root, 2.5, 1e-4),
            "3重根でも区間幅判定なら高精度に解ける: root={root}"
        );
    }

    /// 縮退ケースの不安定性を動学側からも固定する．対称な初期値では $v = W - B = 0$ が
    /// 保たれて混合均衡に留まるが，非対称摂動を与えると単独均衡へティッピングする．
    ///
    /// 注意: 摂動 $v_0$ は小さすぎてはいけない．3次の発散は $t^* = 1/(2Cv_0^2)$ という
    /// 代数的な時間スケールをもち，$v_0$ が小さいと1ステップあたりの変位が
    /// `convergence_tol` を下回って [`integrate`] が誤って「収束」と判定してしまう
    /// (例: $v_0 = 2$ では中心多様体 $u = -v^2/(4s)$ 上に落ちた時点で停止する)．
    /// ここでは実測で端点到達を確認済みの $v_0 = 4$ を使う．
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

        // 対称: v = 0 は不変なので混合均衡に留まる．
        let sym = integrate(&phase, &cfg, (w_star, w_star));
        let last = sym.history.last().unwrap();
        assert!(
            approx(last.w, w_star, 1e-2) && approx(last.b, w_star, 1e-2),
            "対称な初期値では混合均衡に留まる: ({}, {})",
            last.w,
            last.b
        );

        // 非対称 (v = +4): 3次項に押されて全W 端点へ発散する．
        let asym = integrate(&phase, &cfg, (w_star + 2.0, w_star - 2.0));
        let last = asym.history.last().unwrap();
        assert!(
            last.w > 95.0 && last.b < 5.0,
            "非対称摂動で単独均衡へティッピングする: ({}, {})",
            last.w,
            last.b
        );
    }

    /// Fig.18 (基本ケース): 直線型，1:2 比 — 端点2均衡のみ，混合は不安定．
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

        // 全W / 全B が両方含まれる
        assert!(eqs.iter().any(|e| e.kind == EquilibriumKind::AllWhite));
        assert!(eqs.iter().any(|e| e.kind == EquilibriumKind::AllBlack));

        // 端点は安定であること
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

    /// 対称な直線型 (W_max = B_max = 100, R_max=2): 反応曲線は同形．
    /// h(W) = W - W_B(B_W(W)) は W=50 で頂点を共有 → 接する形になる場合がある．
    /// ここでは W_max=B_max を変えて非対称化したケースで混合均衡のテストを書く．
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
        let n_mixed = eqs
            .iter()
            .filter(|e| e.kind == EquilibriumKind::Mixed)
            .count();
        // 対称ケースなのでちょうど W=B の対角線上に1点 (または0点) のはず．
        // 重要なのは混合均衡が検出される能力があること．
        assert!(
            n_mixed >= 1,
            "対称・寛容スケジュールでは混合均衡が少なくとも1点出るべき"
        );
    }

    /// ベクトル場の生成: 全象限のサンプルが領域分類される．
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
