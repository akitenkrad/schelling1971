use crate::grid::{Cell, Grid};
use serde::Serialize;

/// 1ステップ分の分離度指標
#[derive(Debug, Clone, Serialize)]
pub struct Metrics {
    pub step: usize,
    /// 全エージェントの平均同色近隣比率
    pub avg_same_ratio: f64,
    /// 異色近隣を持たないエージェントの割合 (%)
    pub pct_no_opposite: f64,
    /// 非類似性指数 D = 0.5 * Σ |a_i/A - b_i/B|  (格子全体を1ゾーンとした簡易版)
    pub dissimilarity_index: f64,
    /// 不満足エージェント数
    pub n_dissatisfied: usize,
    /// このステップで実際に移動したエージェント数
    pub n_moved: usize,
    /// 集団Aの平均同色近隣比率
    pub avg_same_ratio_a: f64,
    /// 集団Bの平均同色近隣比率
    pub avg_same_ratio_b: f64,
}

impl Metrics {
    /// グリッドの現在状態からメトリクスを計算する
    pub fn compute(grid: &Grid, step: usize, n_dissatisfied: usize, n_moved: usize) -> Self {
        let mut sum_a = 0.0;
        let mut sum_b = 0.0;
        let mut count_a = 0usize;
        let mut count_b = 0usize;
        let mut no_opp = 0usize;
        let mut total_agents = 0usize;

        for r in 0..grid.rows {
            for c in 0..grid.cols {
                let cell = grid.cells[r][c];
                if cell == Cell::Empty {
                    continue;
                }
                total_agents += 1;
                let ratio = grid.same_color_ratio(r, c);

                match cell {
                    Cell::GroupA => { sum_a += ratio; count_a += 1; }
                    Cell::GroupB => { sum_b += ratio; count_b += 1; }
                    Cell::Empty  => {}
                }

                // 異色近隣がいないか確認
                let has_opposite = grid.moore_neighbors(r, c).iter().any(|&(nr, nc)| {
                    grid.cells[nr][nc] != Cell::Empty && grid.cells[nr][nc] != cell
                });
                if !has_opposite {
                    no_opp += 1;
                }
            }
        }

        let avg_a = if count_a > 0 { sum_a / count_a as f64 } else { 0.0 };
        let avg_b = if count_b > 0 { sum_b / count_b as f64 } else { 0.0 };
        let avg_all = if total_agents > 0 {
            (sum_a + sum_b) / total_agents as f64
        } else {
            0.0
        };
        let pct_no_opp = if total_agents > 0 {
            no_opp as f64 / total_agents as f64 * 100.0
        } else {
            0.0
        };

        // 簡易非類似性指数: 格子全体を1ゾーンとして計算
        // D = 0.5 * |a/A - b/B|  (完全分離=0, 完全混合=1 に近づく)
        // ここでは補完: 分離が強いほど D が大きくなるよう符号を調整
        let dissimilarity = if count_a > 0 && count_b > 0 {
            0.5 * ((count_a as f64 / total_agents as f64)
                 - (count_b as f64 / total_agents as f64))
                .abs()
        } else {
            0.5
        };

        Metrics {
            step,
            avg_same_ratio: avg_all,
            pct_no_opposite: pct_no_opp,
            dissimilarity_index: dissimilarity,
            n_dissatisfied,
            n_moved,
            avg_same_ratio_a: avg_a,
            avg_same_ratio_b: avg_b,
        }
    }
}
