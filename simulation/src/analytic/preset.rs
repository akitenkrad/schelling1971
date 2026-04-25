//! 論文 (Schelling 1971) のプリセット設定．
//!
//! 各プリセットは [`PhaseConfig`] と既定初期値・ラベルを返す．

use super::phase::PhaseConfig;
use super::tolerance::ToleranceSchedule;

/// プリセットの結果．
pub struct Preset {
    pub name: &'static str,
    pub description: &'static str,
    pub phase: PhaseConfig,
    pub default_init: (f64, f64),
}

/// プリセット名から PhaseConfig を構築する．未知の名前なら None．
pub fn lookup(name: &str) -> Option<Preset> {
    match name {
        "fig18" => Some(fig18()),
        "fig19" => Some(fig19()),
        "fig22" => Some(fig22()),
        "fig23" => Some(fig23()),
        "fig30a" => Some(fig30a()),
        "fig30b" => Some(fig30b()),
        "fig31" => Some(fig31()),
        "fig32" => Some(fig32()),
        _ => None,
    }
}

/// 既知のプリセット名一覧．
pub fn all_names() -> Vec<&'static str> {
    vec![
        "fig18", "fig19", "fig22", "fig23", "fig30a", "fig30b", "fig31", "fig32",
    ]
}

/// Fig.18: 直線型，1:2 比 — 端点2均衡のみ．
fn fig18() -> Preset {
    Preset {
        name: "fig18",
        description: "Fig.18: 直線型スケジュール (R_max=2.0, W_max=100, B_max=50). 端点2均衡のみ．",
        phase: PhaseConfig {
            w_schedule: ToleranceSchedule::Linear { r_max: 2.0, pop_max: 100.0 },
            b_schedule: ToleranceSchedule::Linear { r_max: 2.0, pop_max: 50.0 },
            capacity: None,
        },
        default_init: (50.0, 25.0),
    }
}

/// Fig.19: 寛容な切片付きアフィン (R_max=4 相当) — 3 種類の交点 (混合 + 2 端点)．
///
/// 論文 p.171 の「中央値許容比率を 1.5 に上げる」例に整合するよう
/// intercept_pop=20, slope=20, pop_max=100 (R_max=4) を採用する．
/// 中央値 (F=50) → R = 30/20 = 1.5 となり論文記述と一致．
/// 反応曲線 $B_W(W) = W \cdot (80 - W) / 20$ の頂点は (40, 80) で，
/// 対称交点は (60, 60) (両反応曲線上の点)．
fn fig19() -> Preset {
    Preset {
        name: "fig19",
        description: "Fig.19: 寛容スケジュール (intercept=20, slope=20, R_max=4). 中央値=1.5 で混合均衡が出現．",
        phase: PhaseConfig {
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
        },
        default_init: (60.0, 60.0),
    }
}

/// Fig.22: 不等数 (W:B = 2:1)．直線型では曲線が交差せず安定混合は消滅．
fn fig22() -> Preset {
    Preset {
        name: "fig22",
        description: "Fig.22: 不等数 (W=100, B=50) で曲線非交差．混合均衡なし．",
        phase: PhaseConfig {
            w_schedule: ToleranceSchedule::Linear { r_max: 2.0, pop_max: 100.0 },
            b_schedule: ToleranceSchedule::Linear { r_max: 2.0, pop_max: 50.0 },
            capacity: None,
        },
        default_init: (60.0, 30.0),
    }
}

/// Fig.23: 入域上限クオータ — 超過分が「実効的零許容者」として混合均衡を生む．
///
/// W:B が大規模だが，B 側の入域上限を 30 に制限する形で表現する．
/// ここでは B のスケジュールを pop_max=30 で打ち切ることで簡略化．
fn fig23() -> Preset {
    Preset {
        name: "fig23",
        description: "Fig.23: B 側入域上限 30 (limiting numbers). クオータが混合均衡を生む．",
        phase: PhaseConfig {
            w_schedule: ToleranceSchedule::Linear { r_max: 2.0, pop_max: 100.0 },
            b_schedule: ToleranceSchedule::Linear { r_max: 2.0, pop_max: 30.0 },
            capacity: None,
        },
        default_init: (50.0, 15.0),
    }
}

/// Fig.30a: in-tipping のみ．全W端点が不安定 → 黒人が自然流入を始める．
/// B の許容スケジュールが寛容で，全W状態でも B が入りたがる構造．
fn fig30a() -> Preset {
    Preset {
        name: "fig30a",
        description: "Fig.30a: in-tipping のみ．B 側が極めて寛容で，全W から自然流入．",
        phase: PhaseConfig {
            // W は通常の直線型
            w_schedule: ToleranceSchedule::Linear { r_max: 2.0, pop_max: 100.0 },
            // B は非常に寛容 (R_max=8) → W 100 でも B は流入意欲あり
            b_schedule: ToleranceSchedule::Linear { r_max: 8.0, pop_max: 50.0 },
            capacity: None,
        },
        default_init: (100.0, 0.0),
    }
}

/// Fig.30b: out-tipping のみ．Fig.18 と同じ (混合不安定，端点 2 安定)．
fn fig30b() -> Preset {
    Preset {
        name: "fig30b",
        description: "Fig.30b: out-tipping のみ．Fig.18 同様の直線型，端点2安定．",
        phase: PhaseConfig {
            w_schedule: ToleranceSchedule::Linear { r_max: 2.0, pop_max: 100.0 },
            b_schedule: ToleranceSchedule::Linear { r_max: 2.0, pop_max: 50.0 },
            capacity: None,
        },
        default_init: (90.0, 5.0),
    }
}

/// Fig.31: 両方のティッピング．全W が不安定 + 安定混合なし．
///
/// 幾何条件: B の反応曲線の頂点 $W_B(B_{\max}/2) = R_{\max}^B B_{\max}/4$ が
/// $W_{\max}$ を超えるとき，$W = W_{\max}$ から B が「上へ伸びる経路」が生じる．
/// $R_{\max}^B = 12, B_{\max} = 50, W_{\max} = 100$ で $12 \cdot 50/4 = 150 > 100$．
fn fig31() -> Preset {
    Preset {
        name: "fig31",
        description: "Fig.31: in-tipping + out-tipping．B 反応曲線が全W点を覆う寛容スケジュール．",
        phase: PhaseConfig {
            // W は不寛容 (R_max=1)
            w_schedule: ToleranceSchedule::Linear { r_max: 1.0, pop_max: 100.0 },
            // B は極めて寛容 (R_max=12)
            b_schedule: ToleranceSchedule::Linear { r_max: 12.0, pop_max: 50.0 },
            capacity: None,
        },
        default_init: (100.0, 5.0),
    }
}

/// Fig.32: 両方ともなし．Fig.19 と同様 (安定混合あり + 端点も安定)．
fn fig32() -> Preset {
    Preset {
        name: "fig32",
        description: "Fig.32: ティッピングなし．安定混合均衡が存在し，端点も安定．",
        phase: PhaseConfig {
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
        },
        default_init: (60.0, 60.0),
    }
}
