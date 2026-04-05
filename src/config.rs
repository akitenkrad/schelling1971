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
    /// 許容限界: 同色近隣比率がこの値未満なら不満足  τ ∈ (0, 1]
    pub threshold: f64,
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
            threshold: 1.0 / 3.0,
            max_iterations: 500,
            seed: Some(42),
            snapshot_interval: 1,
            output_dir: "results".to_string(),
        }
    }
}
