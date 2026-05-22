use serde::Serialize;

/// セルの状態(集団 A / 集団 B / 空き)．
///
/// 空間構造そのものは `socsim_grid::{Grid, GridIndex}` が担い，本 enum は
/// 「どの集団に属するか」と CSV 出力用の整数マッピングのみを表現する．
/// 空きセルは `GridIndex` の占有マップに存在しないことで表され，色マップ
/// (`SchellingWorld::colors`)にも現れない．
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
