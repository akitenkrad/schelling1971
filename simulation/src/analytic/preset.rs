//! 論文 (Schelling 1971) のプリセット設定．
//!
//! 各プリセットは [`PhaseConfig`] と既定初期値・ラベルを返す．

use super::phase::PhaseConfig;
use super::tolerance::ToleranceSchedule;

/// プリセットの結果．
pub struct Preset {
    #[allow(dead_code)]
    pub name: &'static str,
    #[allow(dead_code)]
    pub description: &'static str,
    pub phase: PhaseConfig,
    pub default_init: (f64, f64),
}

/// プリセット名から PhaseConfig を構築する．未知の名前なら None．
pub fn lookup(name: &str) -> Option<Preset> {
    match name {
        "fig18" => Some(fig18()),
        "fig19" => Some(fig19()),
        "fig20" => Some(fig20()),
        "fig21" => Some(fig21()),
        "fig22" => Some(fig22()),
        "fig23" => Some(fig23()),
        "fig24" => Some(fig24()),
        "fig25" => Some(fig25()),
        "fig26" => Some(fig26()),
        "fig27" => Some(fig27()),
        "fig28" => Some(fig28()),
        "fig29" => Some(fig29()),
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
        "fig18", "fig19", "fig20", "fig21", "fig22", "fig23", "fig24", "fig25", "fig26", "fig27",
        "fig28", "fig29", "fig30a", "fig30b", "fig31", "fig32",
    ]
}

/// Fig.18: 直線型，1:2 比 — 端点2均衡のみ．
fn fig18() -> Preset {
    Preset {
        name: "fig18",
        description: "Fig.18: 直線型スケジュール (R_max=2.0, W_max=100, B_max=50). 端点2均衡のみ．",
        phase: PhaseConfig {
            w_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 100.0,
            },
            b_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 50.0,
            },
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

/// Fig.20: 緩勾配の直線型 (寛容スケジュール)．R_max を 2 → 3 へ広げ，
/// 全員がより寛容になった場合の反応曲線を描く．頂点が高くなり混合域が拡がる．
///
/// 論文 pp.171-172 の「許容度を一律に引き上げると曲線の頂点が上がり，
/// 安定混合の余地が生じる」例に対応する．対称 (W_max=B_max=100)．
fn fig20() -> Preset {
    Preset {
        name: "fig20",
        description: "Fig.20: 緩勾配の直線型 (R_max=3, 対称). 寛容化で反応曲線の頂点が上がる．",
        phase: PhaseConfig {
            w_schedule: ToleranceSchedule::Linear {
                r_max: 3.0,
                pop_max: 100.0,
            },
            b_schedule: ToleranceSchedule::Linear {
                r_max: 3.0,
                pop_max: 100.0,
            },
            capacity: None,
        },
        default_init: (50.0, 50.0),
    }
}

/// Fig.21: 急勾配の直線型 (不寛容スケジュール)．R_max を 2 → 1 へ狭める．
/// 反応曲線の頂点が下がり，安定混合が消えて端点分離だけが残る．
///
/// 論文 pp.171-172 の「許容度を一律に下げると分離が強まる」例に対応する．対称．
fn fig21() -> Preset {
    Preset {
        name: "fig21",
        description: "Fig.21: 急勾配の直線型 (R_max=1, 対称). 不寛容化で反応曲線の頂点が下がる．",
        phase: PhaseConfig {
            w_schedule: ToleranceSchedule::Linear {
                r_max: 1.0,
                pop_max: 100.0,
            },
            b_schedule: ToleranceSchedule::Linear {
                r_max: 1.0,
                pop_max: 100.0,
            },
            capacity: None,
        },
        default_init: (50.0, 50.0),
    }
}

/// Fig.22: 不等数 (W:B = 2:1)．直線型では曲線が交差せず安定混合は消滅．
fn fig22() -> Preset {
    Preset {
        name: "fig22",
        description: "Fig.22: 不等数 (W=100, B=50) で曲線非交差．混合均衡なし．",
        phase: PhaseConfig {
            w_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 100.0,
            },
            b_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 50.0,
            },
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
            w_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 100.0,
            },
            b_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 30.0,
            },
            capacity: None,
        },
        default_init: (50.0, 15.0),
    }
}

/// Fig.24: 非対称許容 — W が寛容 (R_max=2)，B が不寛容 (R_max=1)．
/// 反応曲線が非対称になり，混合均衡が B 側に偏った位置に生じる．
///
/// 論文 pp.174-176 の「2 集団の許容スケジュールが異なる」一般化に対応する．
fn fig24() -> Preset {
    Preset {
        name: "fig24",
        description: "Fig.24: 非対称許容 (W:R_max=2, B:R_max=1). 混合均衡が偏在する．",
        phase: PhaseConfig {
            w_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 100.0,
            },
            b_schedule: ToleranceSchedule::Linear {
                r_max: 1.0,
                pop_max: 100.0,
            },
            capacity: None,
        },
        default_init: (50.0, 50.0),
    }
}

/// Fig.25: 切片付きアフィン (ゼロ許容者あり)．intercept_pop=10 で
/// 「常に同色のみを望む 10 人」が両集団に居る場合の反応曲線．
///
/// ゼロ許容者の存在が端点近傍の流出を強め，混合域を狭める (pp.176-178)．対称．
fn fig25() -> Preset {
    Preset {
        name: "fig25",
        description: "Fig.25: ゼロ許容者あり (intercept=10, slope=30). 端点流出が強まる．",
        phase: PhaseConfig {
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
        },
        default_init: (60.0, 60.0),
    }
}

/// Fig.26: 容量制約あり (近隣の総収容数 C=120 < W_max+B_max=200)．
/// 入域競合が生じ，両集団が満員近隣を奪い合う．混合均衡が容量線上に乗る．
///
/// 論文 pp.178-180 の「近隣に物理的な収容上限がある」場合に対応する．
fn fig26() -> Preset {
    Preset {
        name: "fig26",
        description: "Fig.26: 容量制約 C=120. 入域競合で混合均衡が容量線上に乗る．",
        phase: PhaseConfig {
            w_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 100.0,
            },
            b_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 100.0,
            },
            capacity: Some(120.0),
        },
        default_init: (60.0, 50.0),
    }
}

/// Fig.27: 区分線形スケジュール — 中央で折れ曲がる許容分布．
/// 多数が中庸 (R≈1) に集中し，両裾が薄い「S 字」CDF を表す．
///
/// 論文 pp.180-182 の「許容度の分布が一様でない」一般化に対応する．対称．
fn fig27() -> Preset {
    Preset {
        name: "fig27",
        description: "Fig.27: 区分線形 (S字CDF, 中庸集中). 非一様な許容分布．",
        phase: PhaseConfig {
            w_schedule: ToleranceSchedule::PiecewiseLinear {
                points: vec![
                    (0.0, 0.0),
                    (0.5, 10.0),
                    (1.0, 50.0),
                    (1.5, 90.0),
                    (2.0, 100.0),
                ],
                pop_max: 100.0,
            },
            b_schedule: ToleranceSchedule::PiecewiseLinear {
                points: vec![
                    (0.0, 0.0),
                    (0.5, 10.0),
                    (1.0, 50.0),
                    (1.5, 90.0),
                    (2.0, 100.0),
                ],
                pop_max: 100.0,
            },
            capacity: None,
        },
        default_init: (55.0, 55.0),
    }
}

/// Fig.28: 不等数 + 寛容スケジュール (W=100, B=50, R_max=4)．
/// 少数派 (B) が寛容なため，不等数でも混合均衡が生き残る (Fig.22 との対比)．
///
/// 論文 pp.182-184 の「少数派の寛容さが分離を緩和する」例に対応する．
fn fig28() -> Preset {
    Preset {
        name: "fig28",
        description: "Fig.28: 不等数 (W=100, B=50) + B 寛容 (R_max=4). 混合が生き残る．",
        phase: PhaseConfig {
            w_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 100.0,
            },
            b_schedule: ToleranceSchedule::Linear {
                r_max: 4.0,
                pop_max: 50.0,
            },
            capacity: None,
        },
        default_init: (60.0, 25.0),
    }
}

/// Fig.29: 入域上限クオータの強化版 (B 側 pop_max=20)．
/// Fig.23 よりさらに厳しいクオータで，混合均衡が低 B 域に固定される．
///
/// 論文 pp.184-186 の「クオータを強めるほど混合点が下がる」例に対応する．
fn fig29() -> Preset {
    Preset {
        name: "fig29",
        description: "Fig.29: 強いクオータ (B pop_max=20). 混合均衡が低 B 域に固定される．",
        phase: PhaseConfig {
            w_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 100.0,
            },
            b_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 20.0,
            },
            capacity: None,
        },
        default_init: (60.0, 10.0),
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
            w_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 100.0,
            },
            // B は非常に寛容 (R_max=8) → W 100 でも B は流入意欲あり
            b_schedule: ToleranceSchedule::Linear {
                r_max: 8.0,
                pop_max: 50.0,
            },
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
            w_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 100.0,
            },
            b_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 50.0,
            },
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
            w_schedule: ToleranceSchedule::Linear {
                r_max: 1.0,
                pop_max: 100.0,
            },
            // B は極めて寛容 (R_max=12)
            b_schedule: ToleranceSchedule::Linear {
                r_max: 12.0,
                pop_max: 50.0,
            },
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `all_names` の全プリセットが `lookup` で解決でき，名前が一致する．
    #[test]
    fn all_names_resolve() {
        for name in all_names() {
            let p = lookup(name).unwrap_or_else(|| panic!("preset {} が解決できない", name));
            assert_eq!(p.name, name);
            assert!(!p.description.is_empty());
        }
    }

    /// 未知の名前は None．
    #[test]
    fn unknown_name_is_none() {
        assert!(lookup("fig99").is_none());
    }

    /// 新規追加した許容スケジュール変形プリセット (Fig.20-21/24-29) が
    /// 期待するスケジュール種別・パラメータを持つ．
    #[test]
    fn schedule_variant_presets_have_expected_shape() {
        // Fig.20: 緩勾配 (R_max=3)
        match lookup("fig20").unwrap().phase.w_schedule {
            ToleranceSchedule::Linear { r_max, .. } => assert_eq!(r_max, 3.0),
            _ => panic!("fig20 は Linear のはず"),
        }
        // Fig.21: 急勾配 (R_max=1)
        match lookup("fig21").unwrap().phase.w_schedule {
            ToleranceSchedule::Linear { r_max, .. } => assert_eq!(r_max, 1.0),
            _ => panic!("fig21 は Linear のはず"),
        }
        // Fig.24: 非対称 (W R_max=2, B R_max=1)
        let f24 = lookup("fig24").unwrap();
        assert!(matches!(
            (f24.phase.w_schedule, f24.phase.b_schedule),
            (
                ToleranceSchedule::Linear { r_max: 2.0, .. },
                ToleranceSchedule::Linear { r_max: 1.0, .. }
            )
        ));
        // Fig.26: 容量制約あり
        assert_eq!(lookup("fig26").unwrap().phase.capacity, Some(120.0));
        // Fig.27: 区分線形
        assert!(matches!(
            lookup("fig27").unwrap().phase.w_schedule,
            ToleranceSchedule::PiecewiseLinear { .. }
        ));
        // Fig.29: 強いクオータ (B pop_max=20)
        assert_eq!(lookup("fig29").unwrap().phase.b_schedule.pop_max(), 20.0);
    }
}
