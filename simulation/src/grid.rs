use serde::Serialize;

/// Cell state (group A / group B / vacant).
///
/// The spatial structure itself is provided by `socsim_grid::{Grid, GridIndex}`; this enum
/// represents only group membership and the integer mapping for CSV output.
/// A vacant cell is represented by its absence from the `GridIndex` occupancy map and
/// does not appear in the color map (`SchellingWorld::colors`) either.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
pub enum Cell {
    /// Group A (asterisk `*` in the paper)
    GroupA,
    /// Group B (circle `O` in the paper)
    GroupB,
    /// Vacant cell
    Empty,
}

impl Cell {
    /// Converts to an integer value for CSV output (0=vacant, 1=A, 2=B)
    pub fn to_int(self) -> u8 {
        match self {
            Cell::Empty => 0,
            Cell::GroupA => 1,
            Cell::GroupB => 2,
        }
    }
}
