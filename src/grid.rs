use serde::Serialize;

/// セルの状態
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
pub enum Cell {
    /// 集団A (論文中の星 `*`)
    GroupA,
    /// 集団B (論文中の丸 `O`)
    GroupB,
    /// 空きセル
    Empty,
}

impl Cell {
    /// CSV出力用の整数値に変換する (0=空, 1=A, 2=B)
    pub fn to_int(self) -> u8 {
        match self {
            Cell::Empty  => 0,
            Cell::GroupA => 1,
            Cell::GroupB => 2,
        }
    }
}

/// 2次元チェッカーボード・グリッド
pub struct Grid {
    pub rows: usize,
    pub cols: usize,
    pub cells: Vec<Vec<Cell>>,
}

impl Grid {
    /// 指定された配置でグリッドを生成する
    pub fn new(rows: usize, cols: usize, cells: Vec<Vec<Cell>>) -> Self {
        assert_eq!(cells.len(), rows);
        assert!(cells.iter().all(|row| row.len() == cols));
        Grid { rows, cols, cells }
    }

    /// ムーア近傍 (8近傍) のセル座標を返す
    pub fn moore_neighbors(&self, r: usize, c: usize) -> Vec<(usize, usize)> {
        let mut result = Vec::with_capacity(8);
        for dr in [-1i32, 0, 1] {
            for dc in [-1i32, 0, 1] {
                if dr == 0 && dc == 0 {
                    continue;
                }
                let nr = r as i32 + dr;
                let nc = c as i32 + dc;
                if nr >= 0 && nr < self.rows as i32 && nc >= 0 && nc < self.cols as i32 {
                    result.push((nr as usize, nc as usize));
                }
            }
        }
        result
    }

    /// 指定セルの同色近隣比率を計算する
    /// 占有近隣セルが 0 の場合は 1.0 (満足) を返す
    pub fn same_color_ratio(&self, r: usize, c: usize) -> f64 {
        let agent = self.cells[r][c];
        if agent == Cell::Empty {
            return 1.0;
        }
        let neighbors = self.moore_neighbors(r, c);
        let occupied: Vec<_> = neighbors
            .iter()
            .filter(|&&(nr, nc)| self.cells[nr][nc] != Cell::Empty)
            .collect();
        if occupied.is_empty() {
            return 1.0;
        }
        let same = occupied
            .iter()
            .filter(|&&(nr, nc)| self.cells[nr][nc] == agent)
            .count();
        same as f64 / occupied.len() as f64
    }

    /// エージェントが満足しているか判定する
    pub fn is_satisfied(&self, r: usize, c: usize, threshold: f64) -> bool {
        self.same_color_ratio(r, c) >= threshold
    }

    /// 仮に (from) から (to) へ移動した場合の同色比率をシミュレーションする
    pub fn simulated_ratio(&self, from: (usize, usize), to: (usize, usize)) -> f64 {
        let agent = self.cells[from.0][from.1];
        let neighbors = self.moore_neighbors(to.0, to.1);
        let mut same = 0usize;
        let mut total = 0usize;
        for &(nr, nc) in &neighbors {
            if (nr, nc) == from {
                continue; // 元の位置は空になる
            }
            if self.cells[nr][nc] != Cell::Empty {
                total += 1;
                if self.cells[nr][nc] == agent {
                    same += 1;
                }
            }
        }
        if total == 0 { 1.0 } else { same as f64 / total as f64 }
    }

    /// 全空きセルをリストアップする
    pub fn vacant_cells(&self) -> Vec<(usize, usize)> {
        let mut v = Vec::new();
        for r in 0..self.rows {
            for c in 0..self.cols {
                if self.cells[r][c] == Cell::Empty {
                    v.push((r, c));
                }
            }
        }
        v
    }

    /// チェビシェフ距離
    pub fn chebyshev(a: (usize, usize), b: (usize, usize)) -> usize {
        let dr = (a.0 as i32 - b.0 as i32).unsigned_abs() as usize;
        let dc = (a.1 as i32 - b.1 as i32).unsigned_abs() as usize;
        dr.max(dc)
    }

    /// グリッドの状態を行列 (Vec<Vec<u8>>) として返す (可視化用)
    pub fn to_matrix(&self) -> Vec<Vec<u8>> {
        self.cells
            .iter()
            .map(|row| row.iter().map(|c| c.to_int()).collect())
            .collect()
    }
}
