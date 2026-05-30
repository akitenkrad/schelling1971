/// 満足判定ルール．Schelling (1971) の3種類の選好形式に対応する．
///
/// - `Ratio`         : 同色近隣比率が閾値以上なら満足（デフォルト・分離型, Fig. 7-14）
/// - `MinSame`       : 同色近隣の絶対数が下限以上なら満足（集会型, Fig. 16）
/// - `Bounded`       : 同色近隣の絶対数が [min_same, max_same] の範囲内なら満足（統合型, Fig. 17）
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SatisfactionRule {
    Ratio { threshold: f64 },
    MinSame { min_same: usize },
    Bounded { min_same: usize, max_same: usize },
}

impl SatisfactionRule {
    /// 同色近隣数と占有近隣数から，満足かどうかを評価する．
    pub fn evaluate(&self, same: usize, total_occupied: usize) -> bool {
        match *self {
            SatisfactionRule::Ratio { threshold } => {
                // 占有近隣が0なら満足（既存挙動を維持）
                if total_occupied == 0 {
                    return true;
                }
                (same as f64) / (total_occupied as f64) >= threshold
            }
            SatisfactionRule::MinSame { min_same } => same >= min_same,
            SatisfactionRule::Bounded { min_same, max_same } => {
                same >= min_same && same <= max_same
            }
        }
    }

    /// CLI/ログ出力用のラベル
    pub fn label(&self) -> String {
        match *self {
            SatisfactionRule::Ratio { threshold } => format!("ratio:{:.3}", threshold),
            SatisfactionRule::MinSame { min_same } => format!("min-same:{}", min_same),
            SatisfactionRule::Bounded { min_same, max_same } => {
                format!("bounded:{}:{}", min_same, max_same)
            }
        }
    }
}

/// 移動運用モード．Schelling (1971) p.155 が区別する 2 つの運用形式に対応する．
///
/// - `Standard` : 緩運用．不満足エージェントのみが移動する (Fig.9–14 のデフォルト)．
/// - `Strict`   : 厳格運用 (Fig.8)．不満足エージェントに加えて，満足している
///   エージェントも「同色比率を厳密に改善できる空きセル」へ投機的に移動する．
///   満足者が常により同質な近隣を探すため，分離度が緩運用より高くなる．
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MoveMode {
    /// 緩運用: 不満足者のみ移動 (既存挙動)．
    #[default]
    Standard,
    /// 厳格運用 (Fig.8): 満足者も同色比率を厳密に改善できれば移動する．
    Strict,
}

impl MoveMode {
    /// CLI 文字列からパースする．
    pub fn parse(s: &str) -> Option<MoveMode> {
        match s {
            "standard" => Some(MoveMode::Standard),
            "strict" => Some(MoveMode::Strict),
            _ => None,
        }
    }

    /// CLI/ログ出力用のラベル．
    pub fn label(self) -> &'static str {
        match self {
            MoveMode::Standard => "standard",
            MoveMode::Strict => "strict",
        }
    }
}

/// 移動先選択戦略．不満足エージェントが「どの満足できる空きセルへ動くか」を決める．
///
/// - `Nearest`   : 最近傍 (チェビシェフ距離) で最初に見つかった満足できる空きセルへ
///   動く (既存挙動)．Schelling の格子図 Fig.7–14 のデフォルト．
/// - `BestLocal` : 満足できる全空きセルのうち，移動後の同色比率が最大のセルへ動く．
///   少数派が「最も同質な区画」へ寄り集まるため，不等数 (Fig.12) の少数派クラスタ
///   比率が論文値 (>80%) に近づく．同比率の候補は距離昇順→行優先順で先勝ち
///   (より近く・より上左のセルを選ぶ；決定論)．探索する空きセル集合は Nearest と
///   同一で，選択基準だけが異なる．
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MoveStrategy {
    /// 最近傍で最初に満足できる空きセル (既存挙動)．
    #[default]
    Nearest,
    /// 最近傍距離帯の中で移動後同色比率が最大のセル (Fig.12 クラスタ改善)．
    BestLocal,
}

impl MoveStrategy {
    /// CLI 文字列からパースする．
    pub fn parse(s: &str) -> Option<MoveStrategy> {
        match s {
            "nearest" => Some(MoveStrategy::Nearest),
            "best-local" => Some(MoveStrategy::BestLocal),
            _ => None,
        }
    }

    /// CLI/ログ出力用のラベル．
    pub fn label(self) -> &'static str {
        match self {
            MoveStrategy::Nearest => "nearest",
            MoveStrategy::BestLocal => "best-local",
        }
    }
}

/// シミュレーション設定
#[derive(Debug, Clone)]
pub struct Config {
    /// グリッドの行数
    pub rows: usize,
    /// グリッドの列数
    pub cols: usize,
    /// 集団Aのエージェント数
    pub n_a: usize,
    /// 集団Bのエージェント数
    pub n_b: usize,
    /// 満足判定ルール
    pub rule: SatisfactionRule,
    /// 移動運用モード (緩運用 = Standard / 厳格運用 = Strict, Fig.8)
    pub move_mode: MoveMode,
    /// 移動先選択戦略 (Nearest = 既存 / BestLocal = Fig.12 クラスタ改善)
    pub move_strategy: MoveStrategy,
    /// 最大反復回数
    pub max_iterations: usize,
    /// 乱数シード (None の場合はランダム)
    pub seed: Option<u64>,
    /// スナップショットを保存するステップ間隔 (0 = 保存しない)
    pub snapshot_interval: usize,
    /// 結果出力ディレクトリ
    pub output_dir: String,
}

impl Default for Config {
    /// Schellingの論文 (Figure 7--10) に近い標準設定
    fn default() -> Self {
        // 13行16列 = 208セル，約30%空き → エージェント計146
        let rows = 13;
        let cols = 16;
        let total = rows * cols;
        let n_vacant = (total as f64 * 0.30).round() as usize;
        let n_agents = total - n_vacant;
        let n_a = n_agents / 2;
        let n_b = n_agents - n_a;

        Config {
            rows,
            cols,
            n_a,
            n_b,
            rule: SatisfactionRule::Ratio {
                threshold: 1.0 / 3.0,
            },
            move_mode: MoveMode::Standard,
            move_strategy: MoveStrategy::Nearest,
            max_iterations: 500,
            seed: Some(42),
            snapshot_interval: 1,
            output_dir: "results".to_string(),
        }
    }
}
