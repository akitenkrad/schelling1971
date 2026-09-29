//! Preset configurations from the paper (Schelling 1971).
//!
//! Each preset returns a [`PhaseConfig`], default initial values, and a label.

use super::phase::PhaseConfig;
use super::tolerance::ToleranceSchedule;

/// Result of a preset.
pub struct Preset {
    #[allow(dead_code)]
    pub name: &'static str,
    #[allow(dead_code)]
    pub description: &'static str,
    pub phase: PhaseConfig,
    pub default_init: (f64, f64),
}

/// Constructs a PhaseConfig from a preset name. Returns None for an unknown name.
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

/// List of known preset names.
pub fn all_names() -> Vec<&'static str> {
    vec![
        "fig18", "fig19", "fig20", "fig21", "fig22", "fig23", "fig24", "fig25", "fig26", "fig27",
        "fig28", "fig29", "fig30a", "fig30b", "fig31", "fig32",
    ]
}

/// Fig.18: Linear form, 1:2 ratio—only two endpoint equilibria.
fn fig18() -> Preset {
    Preset {
        name: "fig18",
        description: "Fig.18: Linear schedules (R_max=2.0, W_max=100, B_max=50). Only two endpoint equilibria.",
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

/// Fig.19: Tolerant affine form with an intercept (equivalent to R_max=4)—three types of intersections (mixed + two endpoints).
///
/// To match the example on p.171 of the paper, which "raises the median tolerance ratio to 1.5,"
/// use intercept_pop=20, slope=20, pop_max=100 (R_max=4).
/// At the median (F=50), R = 30/20 = 1.5, consistent with the paper's description.
/// The reaction curve $B_W(W) = W \cdot (80 - W) / 20$ has its peak at (40, 80),
/// and the symmetric intersection is (60, 60), a point on both reaction curves.
fn fig19() -> Preset {
    Preset {
        name: "fig19",
        description: "Fig.19: Tolerant schedules (intercept=20, slope=20, R_max=4). A mixed equilibrium appears at the midpoint 1.5.",
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

/// Fig.20: Gently sloped linear form (tolerant schedule). Expands R_max from 2 → 3
/// and plots the reaction curve when everyone becomes more tolerant. The peak rises and the mixed region expands.
///
/// Corresponds to the example on pp.171-172 of the paper where "uniformly increasing tolerance raises
/// the peak of the curve and creates room for stable mixing." Symmetric (W_max=B_max=100).
fn fig20() -> Preset {
    Preset {
        name: "fig20",
        description: "Fig.20: Shallow linear schedules (R_max=3, symmetric). Greater tolerance raises the peaks of the response curves.",
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

/// Fig.21: Steeply sloped linear form (intolerant schedule). Narrows R_max from 2 → 1.
/// The reaction-curve peak falls, stable mixing disappears, and only endpoint segregation remains.
///
/// Corresponds to the example on pp.171-172 of the paper where "uniformly decreasing tolerance strengthens segregation." Symmetric.
fn fig21() -> Preset {
    Preset {
        name: "fig21",
        description: "Fig.21: Steep linear schedules (R_max=1, symmetric). Greater intolerance lowers the peaks of the response curves.",
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

/// Fig.22: Unequal numbers (W:B = 2:1). With the linear form, the curves do not intersect and stable mixing disappears.
fn fig22() -> Preset {
    Preset {
        name: "fig22",
        description: "Fig.22: Unequal numbers (W=100, B=50) with nonintersecting curves. No mixed equilibrium.",
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

/// Fig.23: Entry-limit quota—the excess acts as "effectively zero-tolerance agents" and creates a mixed equilibrium.
///
/// Represents a large W:B population while restricting the B-side entry limit to 30.
/// Simplified here by truncating the B schedule at pop_max=30.
fn fig23() -> Preset {
    Preset {
        name: "fig23",
        description: "Fig.23: B-side entry limit of 30 (limiting numbers). The quota creates a mixed equilibrium.",
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

/// Fig.24: Asymmetric tolerance—W is tolerant (R_max=2), while B is intolerant (R_max=1).
/// The reaction curves become asymmetric, producing a mixed equilibrium biased toward the B side.
///
/// Corresponds to the generalization on pp.174-176 of the paper where "the two groups have different tolerance schedules."
fn fig24() -> Preset {
    Preset {
        name: "fig24",
        description:
            "Fig.24: Asymmetric tolerance (W:R_max=2, B:R_max=1). The mixed equilibrium is skewed.",
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

/// Fig.25: Affine form with an intercept (including zero-tolerance agents). With intercept_pop=10,
/// the reaction curves represent the case where both groups contain "10 people who always want only their own group."
///
/// The presence of zero-tolerance agents increases outflow near the endpoints and narrows the mixed region (pp.176-178). Symmetric.
fn fig25() -> Preset {
    Preset {
        name: "fig25",
        description: "Fig.25: Some agents have zero tolerance (intercept=10, slope=30). Outflow from the endpoints increases.",
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

/// Fig.26: With a capacity constraint (total neighborhood capacity C=120 < W_max+B_max=200).
/// Entry competition occurs, with both groups competing for a full neighborhood. The mixed equilibrium lies on the capacity line.
///
/// Corresponds to the case on pp.178-180 of the paper where "the neighborhood has a physical capacity limit."
fn fig26() -> Preset {
    Preset {
        name: "fig26",
        description: "Fig.26: Capacity constraint C=120. Competition for entry places the mixed equilibrium on the capacity line.",
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

/// Fig.27: Piecewise-linear schedule—a tolerance distribution that bends in the middle.
/// Represents an "S-shaped" CDF in which most agents are concentrated near the moderate value (R≈1) and both tails are thin.
///
/// Corresponds to the generalization on pp.180-182 of the paper where "the tolerance distribution is nonuniform." Symmetric.
fn fig27() -> Preset {
    Preset {
        name: "fig27",
        description: "Fig.27: Piecewise-linear schedules (S-shaped CDF, concentrated near the middle). Nonuniform tolerance distributions.",
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

/// Fig.28: Unequal numbers + tolerant schedule (W=100, B=50, R_max=4).
/// Because the minority (B) is tolerant, the mixed equilibrium survives despite unequal numbers (in contrast to Fig.22).
///
/// Corresponds to the example on pp.182-184 of the paper where "minority tolerance mitigates segregation."
fn fig28() -> Preset {
    Preset {
        name: "fig28",
        description: "Fig.28: Unequal numbers (W=100, B=50) with tolerant B agents (R_max=4). Mixing persists.",
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

/// Fig.29: A stricter entry-limit quota (B-side pop_max=20).
/// A quota even stricter than Fig.23 fixes the mixed equilibrium in the low-B region.
///
/// Corresponds to the example on pp.184-186 of the paper where "tightening the quota lowers the mixed point."
fn fig29() -> Preset {
    Preset {
        name: "fig29",
        description: "Fig.29: Strong quota (B pop_max=20). The mixed equilibrium is confined to the low-B region.",
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

/// Fig.30a: In-tipping only. The all-W endpoint is unstable → B agents begin entering spontaneously.
/// The B tolerance schedule is permissive, so B agents want to enter even in the all-W state.
fn fig30a() -> Preset {
    Preset {
        name: "fig30a",
        description: "Fig.30a: In-tipping only. The B side is extremely tolerant, producing spontaneous inflow from the all-W state.",
        phase: PhaseConfig {
            // W follows the ordinary linear form.
            w_schedule: ToleranceSchedule::Linear {
                r_max: 2.0,
                pop_max: 100.0,
            },
            // B is very tolerant (R_max=8) → B agents want to enter even with W at 100.
            b_schedule: ToleranceSchedule::Linear {
                r_max: 8.0,
                pop_max: 50.0,
            },
            capacity: None,
        },
        default_init: (100.0, 0.0),
    }
}

/// Fig.30b: Out-tipping only. Same as Fig.18 (unstable mixing, two stable endpoints).
fn fig30b() -> Preset {
    Preset {
        name: "fig30b",
        description:
            "Fig.30b: Out-tipping only. Linear schedules as in Fig.18, with two stable endpoints.",
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

/// Fig.31: Both types of tipping. The all-W state is unstable + no stable mixing.
///
/// Geometric condition: when the peak of B's reaction curve $W_B(B_{\max}/2) = R_{\max}^B B_{\max}/4$
/// exceeds $W_{\max}$, a path along which B "extends upward" from $W = W_{\max}$ emerges.
/// With $R_{\max}^B = 12, B_{\max} = 50, W_{\max} = 100$, $12 \cdot 50/4 = 150 > 100$.
fn fig31() -> Preset {
    Preset {
        name: "fig31",
        description: "Fig.31: In-tipping and out-tipping. A tolerant schedule whose B response curve covers the all-W point.",
        phase: PhaseConfig {
            // W is intolerant (R_max=1).
            w_schedule: ToleranceSchedule::Linear {
                r_max: 1.0,
                pop_max: 100.0,
            },
            // B is extremely tolerant (R_max=12).
            b_schedule: ToleranceSchedule::Linear {
                r_max: 12.0,
                pop_max: 50.0,
            },
            capacity: None,
        },
        default_init: (100.0, 5.0),
    }
}

/// Fig.32: Neither type. Same as Fig.19 (stable mixing + stable endpoints).
fn fig32() -> Preset {
    Preset {
        name: "fig32",
        description: "Fig.32: No tipping. A stable mixed equilibrium exists, and the endpoints are also stable.",
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

    /// Every preset in `all_names` can be resolved by `lookup`, and its name matches.
    #[test]
    fn all_names_resolve() {
        for name in all_names() {
            let p = lookup(name).unwrap_or_else(|| panic!("cannot resolve preset {}", name));
            assert_eq!(p.name, name);
            assert!(!p.description.is_empty());
        }
    }

    /// An unknown name returns None.
    #[test]
    fn unknown_name_is_none() {
        assert!(lookup("fig99").is_none());
    }

    /// The newly added tolerance-schedule variation presets (Fig.20-21/24-29)
    /// have the expected schedule types and parameters.
    #[test]
    fn schedule_variant_presets_have_expected_shape() {
        // Fig.20: Gentle slope (R_max=3).
        match lookup("fig20").unwrap().phase.w_schedule {
            ToleranceSchedule::Linear { r_max, .. } => assert_eq!(r_max, 3.0),
            _ => panic!("fig20 should use Linear schedules"),
        }
        // Fig.21: Steep slope (R_max=1).
        match lookup("fig21").unwrap().phase.w_schedule {
            ToleranceSchedule::Linear { r_max, .. } => assert_eq!(r_max, 1.0),
            _ => panic!("fig21 should use Linear schedules"),
        }
        // Fig.24: Asymmetric (W R_max=2, B R_max=1).
        let f24 = lookup("fig24").unwrap();
        assert!(matches!(
            (f24.phase.w_schedule, f24.phase.b_schedule),
            (
                ToleranceSchedule::Linear { r_max: 2.0, .. },
                ToleranceSchedule::Linear { r_max: 1.0, .. }
            )
        ));
        // Fig.26: With a capacity constraint.
        assert_eq!(lookup("fig26").unwrap().phase.capacity, Some(120.0));
        // Fig.27: Piecewise linear.
        assert!(matches!(
            lookup("fig27").unwrap().phase.w_schedule,
            ToleranceSchedule::PiecewiseLinear { .. }
        ));
        // Fig.29: Strict quota (B pop_max=20).
        assert_eq!(lookup("fig29").unwrap().phase.b_schedule.pop_max(), 20.0);
    }
}
