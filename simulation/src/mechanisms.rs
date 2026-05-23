//! socsim フレームワーク上の Schelling 移動メカニズム．
//!
//! Schelling (1971) の移動規則を socsim の [`Mechanism`] として実装する．
//! `Decision` フェーズで発火し，[`StepContext::agent_order`](socsim_core::StepContext)
//! が与えるアクティベーション順(= スケジューラがシャッフルした順序)に従って
//! 各エージェントを処理する:
//!
//! 1. 移動対象は**ステップ開始時に不満足だったエージェントのみ**(旧実装の
//!    「不満足エージェントを収集してから処理する」セマンティクスを保つ)．
//!    途中で他者の移動により不満足化したエージェントは当該ステップでは動かさない．
//! 2. 対象エージェントが処理時点で既に満足していればスキップ(他者の移動で
//!    満足化した場合)．
//! 3. 不満足なら空きセルをチェビシェフ距離の昇順で探索し，移動後に満足できる
//!    最初の空きセルへ移動する．
//!
//! これにより `n_moved <= n_dissatisfied(ステップ開始時)` が保たれる．
//!
//! 移動先選択に乱数は使わない(最近傍貪欲)．ランダム性はアクティベーション順序の
//! シャッフルのみに由来する(`RandomActivationScheduler`)．`agent_order` は全
//! エージェントのシャッフルだが，開始時不満足集合でフィルタするため，旧実装が
//! 不満足ベクタを直接シャッフルしたのと統計的に等価な処理順となる．
//!
//! ステップ結果(移動数・開始時不満足数・収束フラグ)は [`StepContext::scratch`] に
//! 書き込み，ドライバが [`Simulation::scratch`](socsim_engine::Simulation::scratch)
//! 経由で読み取る．収束(開始時不満足が空)または行き詰まり(`n_moved == 0`)を検知
//! したら [`StepContext::request_stop`] でエンジンに停止を要求する．

use std::collections::HashSet;

use socsim_core::{AgentId, Mechanism, Phase, Result, StepContext};

use crate::world::SchellingWorld;

/// 不満足エージェントを最近傍の満足できる空きセルへ移動させるメカニズム．
pub struct SchellingMoveMechanism;

impl Mechanism<SchellingWorld> for SchellingMoveMechanism {
    fn name(&self) -> &str {
        "schelling_move"
    }

    fn phases(&self) -> &'static [Phase] {
        &[Phase::Decision]
    }

    fn apply(&mut self, _phase: Phase, ctx: &mut StepContext<'_, SchellingWorld>) -> Result<()> {
        // 近隣走査用の再利用バッファ(満足判定のホットパスからヒープ確保を排除する)．
        // `neighbors_into` は `neighbors` と同一順序の近隣を埋めるため，満足判定・
        // 移動先選択(ひいては結果)はいずれも不変．
        let mut buf: Vec<(usize, usize)> = Vec::new();

        // ステップ開始時に不満足なエージェント集合をスナップショットする．
        // 当該ステップで移動できるのはこの集合のメンバーのみ．
        let dissatisfied: HashSet<AgentId> = ctx
            .world
            .colors
            .keys()
            .copied()
            .filter(|id| {
                let (r, c) = ctx.world.index.position(*id).unwrap();
                !ctx.world.is_satisfied_buf(r, c, &mut buf)
            })
            .collect();

        let mut n_moved = 0usize;

        for id in ctx.agent_order {
            // 開始時に満足していたエージェントは当該ステップでは動かさない．
            if !dissatisfied.contains(id) {
                continue;
            }

            // 現在位置を取得．
            let (r, c) = match ctx.world.index.position(*id) {
                Some(pos) => pos,
                None => continue, // 念のため(常に存在するはず)
            };

            // 他者の移動で既に満足していればスキップ．
            if ctx.world.is_satisfied_buf(r, c, &mut buf) {
                continue;
            }

            // 空きセルを最近傍順に探索し，満足できる最初のセルへ移動．
            if let Some(v) = ctx.world.nearest_satisfying_vacant((r, c)) {
                ctx.world
                    .index
                    .move_to(*id, v.0, v.1)
                    .expect("空きセルへの移動に失敗");
                n_moved += 1;
            }
        }

        let converged = dissatisfied.is_empty();

        // ステップ結果を scratch に書き出す(ドライバが読む)．
        ctx.scratch.insert("n_moved", n_moved);
        ctx.scratch.insert("n_dissatisfied", dissatisfied.len());
        ctx.scratch.insert("converged", converged);

        // 収束(開始時不満足が空)または行き詰まり(移動0)なら停止を要求．
        if converged || n_moved == 0 {
            ctx.request_stop();
        }

        Ok(())
    }
}
