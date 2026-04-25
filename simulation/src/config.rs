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
            rule: SatisfactionRule::Ratio { threshold: 1.0 / 3.0 },
            max_iterations: 500,
            seed: Some(42),
            snapshot_interval: 1,
            output_dir: "results".to_string(),
        }
    }
}
