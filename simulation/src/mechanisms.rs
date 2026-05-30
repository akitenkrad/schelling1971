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

use crate::config::MoveMode;
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

        // 厳格運用 (Fig.8): 満足者も「同色比率を厳密に改善できる」空きセルへ投機的に
        // 移動する．対象はステップ開始時に満足していたエージェントのみ(当該ステップで
        // 不満足者の移動により満足化した者は次ステップまで動かさない)．処理時点で
        // 不満足化していればスキップする(緩運用の対象になりうるが当該ステップでは不可)．
        let mut n_speculative = 0usize;
        if ctx.world.move_mode == MoveMode::Strict {
            for id in ctx.agent_order {
                // 開始時に不満足だったエージェントは投機対象外(既に上で処理済み)．
                if dissatisfied.contains(id) {
                    continue;
                }
                let (r, c) = match ctx.world.index.position(*id) {
                    Some(pos) => pos,
                    None => continue,
                };
                // 処理時点で不満足なら投機ではなく通常移動の対象(当該ステップでは動かさない)．
                if !ctx.world.is_satisfied_buf(r, c, &mut buf) {
                    continue;
                }
                if let Some(v) = ctx.world.best_speculative_vacant((r, c)) {
                    ctx.world
                        .index
                        .move_to(*id, v.0, v.1)
                        .expect("空きセルへの投機的移動に失敗");
                    n_speculative += 1;
                }
            }
        }

        let total_moved = n_moved + n_speculative;
        let converged = dissatisfied.is_empty();

        // ステップ結果を scratch に書き出す(ドライバが読む)．
        // `n_moved` は通常移動＋投機移動の合計(緩運用では投機移動は常に 0)．
        ctx.scratch.insert("n_moved", total_moved);
        ctx.scratch.insert("n_dissatisfied", dissatisfied.len());
        ctx.scratch.insert("converged", converged);

        // 停止判定:
        // - 緩運用: 収束(開始時不満足が空)または行き詰まり(移動 0)．
        //   投機移動は常に 0 なので `total_moved == 0` は旧来の `n_moved == 0` と一致し，
        //   挙動はビット単位で不変．
        // - 厳格運用: 不満足移動も投機移動も発生しなくなった時点で停止(全員が満足し，
        //   かつ誰も同色比率を改善できない安定状態)．
        let should_stop = match ctx.world.move_mode {
            MoveMode::Standard => converged || total_moved == 0,
            MoveMode::Strict => converged && total_moved == 0,
        };
        if should_stop {
            ctx.request_stop();
        }

        Ok(())
    }
}
