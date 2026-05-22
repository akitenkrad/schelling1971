//! socsim フレームワーク上の Schelling 分居モデルの世界状態．
//!
//! `SchellingWorld` は socsim の [`WorldState`] を実装し，空間占有を
//! [`socsim_grid::GridIndex`] で，各エージェントの集団(色)を `colors` マップで
//! 保持する．空きセルは占有マップに現れず，`colors` にも現れない．
//!
//! 旧実装の手書き近傍計算(moore_neighbors / vacant_cells / chebyshev など)は
//! socsim-grid の [`Grid`]・[`GridIndex`] に置き換え，本モジュールでは Schelling
//! 固有の判定(満足・同色比率・移動先探索)のみをドメインヘルパとして実装する．

use std::collections::BTreeMap;

use socsim_core::{AgentId, SimClock, WorldState};
use socsim_grid::{GridIndex, Metric, Neighborhood};

use crate::config::SatisfactionRule;
use crate::grid::Cell;

/// Schelling 分居モデルの世界状態
pub struct SchellingWorld {
    /// シミュレーションクロック
    pub clock: SimClock,
    /// 空間占有インデックス．近隣計算・空きセル探索の正本．
    pub index: GridIndex,
    /// 各エージェントの集団(色)．空きセルは存在しない(キーは占有エージェントのみ)．
    pub colors: BTreeMap<AgentId, Cell>,
    /// 満足判定ルール
    pub rule: SatisfactionRule,
}

impl SchellingWorld {
    /// グリッドインデックスと色マップから世界状態を構築する．
    pub fn new(index: GridIndex, colors: BTreeMap<AgentId, Cell>, rule: SatisfactionRule, t_max: u64) -> Self {
        SchellingWorld {
            clock: SimClock::new(t_max),
            index,
            colors,
            rule,
        }
    }

    /// グリッドの行数．
    pub fn rows(&self) -> usize {
        self.index.grid().rows()
    }

    /// グリッドの列数．
    pub fn cols(&self) -> usize {
        self.index.grid().cols()
    }

    /// `(r, c)` に居るエージェントの色を返す(空きセルなら `Cell::Empty`)．
    pub fn cell_color(&self, r: usize, c: usize) -> Cell {
        match self.index.occupant(r, c) {
            Some(id) => self.colors[&id],
            None => Cell::Empty,
        }
    }

    /// `(r, c)` の (同色占有近隣数, 占有近隣数) を返す．
    /// 空きセルを指定した場合は (0, 0) を返す．
    pub fn neighbor_counts(&self, r: usize, c: usize) -> (usize, usize) {
        let agent = self.cell_color(r, c);
        if agent == Cell::Empty {
            return (0, 0);
        }
        let mut same = 0usize;
        let mut total = 0usize;
        for (nr, nc) in self.index.grid().neighbors(r, c, Neighborhood::Moore) {
            if let Some(id) = self.index.occupant(nr, nc) {
                total += 1;
                if self.colors[&id] == agent {
                    same += 1;
                }
            }
        }
        (same, total)
    }

    /// 指定セルの同色近隣比率を計算する．
    /// 占有近隣セルが 0 の場合は 1.0 (満足) を返す(旧実装の規約を維持)．
    pub fn same_color_ratio(&self, r: usize, c: usize) -> f64 {
        let (same, total) = self.neighbor_counts(r, c);
        if total == 0 {
            return 1.0;
        }
        same as f64 / total as f64
    }

    /// エージェントがルールに照らして満足しているか判定する．
    pub fn is_satisfied(&self, r: usize, c: usize) -> bool {
        if self.cell_color(r, c) == Cell::Empty {
            return true;
        }
        let (same, total) = self.neighbor_counts(r, c);
        self.rule.evaluate(same, total)
    }

    /// `(r, c)` に異色の占有近隣が存在するか判定する(metrics 用)．
    pub fn has_opposite_neighbor(&self, r: usize, c: usize) -> bool {
        let agent = self.cell_color(r, c);
        if agent == Cell::Empty {
            return false;
        }
        self.index
            .grid()
            .neighbors(r, c, Neighborhood::Moore)
            .into_iter()
            .any(|(nr, nc)| match self.index.occupant(nr, nc) {
                Some(id) => self.colors[&id] != agent,
                None => false,
            })
    }

    /// `from` から `to` へ移動したと仮定した場合に満足となるか判定する．
    ///
    /// `to` の近傍を数える際，移動元 `from` のセルは(エージェントが抜けるため)
    /// 占有とみなさない．移動するエージェントの色は `from` の現在の色を用いる．
    pub fn will_be_satisfied_after_move(&self, from: (usize, usize), to: (usize, usize)) -> bool {
        let agent = self.cell_color(from.0, from.1);
        let mut same = 0usize;
        let mut total = 0usize;
        for (nr, nc) in self.index.grid().neighbors(to.0, to.1, Neighborhood::Moore) {
            if (nr, nc) == from {
                continue; // 元の位置は空になる
            }
            if let Some(id) = self.index.occupant(nr, nc) {
                total += 1;
                if self.colors[&id] == agent {
                    same += 1;
                }
            }
        }
        self.rule.evaluate(same, total)
    }

    /// `from` から最近傍(チェビシェフ距離)順に空きセルを走査し，移動後に満足
    /// できる最初のセルを返す．
    ///
    /// 空きセルは [`GridIndex::vacant_cells`] が返す行優先順を起点に，チェビシェフ
    /// 距離で**安定ソート**する(距離は整数値なので同距離内は行優先順を保つ)．
    /// これは旧実装の `sort_by_key(chebyshev)` の挙動と一致する．
    pub fn nearest_satisfying_vacant(&self, from: (usize, usize)) -> Option<(usize, usize)> {
        let mut vacants = self.index.vacant_cells();
        vacants.sort_by(|&a, &b| {
            let da = self.index.grid().distance(Metric::Chebyshev, from, a);
            let db = self.index.grid().distance(Metric::Chebyshev, from, b);
            da.partial_cmp(&db).unwrap()
        });
        vacants
            .into_iter()
            .find(|&v| self.will_be_satisfied_after_move(from, v))
    }
}

impl WorldState for SchellingWorld {
    fn agent_ids(&self) -> Vec<AgentId> {
        // BTreeMap のキーは昇順 → 決定論のために必要なソート順を満たす．
        self.colors.keys().copied().collect()
    }

    fn clock(&self) -> &SimClock {
        &self.clock
    }

    fn clock_mut(&mut self) -> &mut SimClock {
        &mut self.clock
    }
}
