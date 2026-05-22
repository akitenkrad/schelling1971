use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::BufWriter;

use csv::Writer;
use rand::seq::SliceRandom;

use socsim_core::{derive_seed, AgentId, SimRng};
use socsim_engine::{RandomActivationScheduler, SimulationBuilder};
use socsim_grid::{Boundary, Grid, GridIndex};

use crate::config::Config;
use crate::grid::Cell;
use crate::mechanisms::SchellingMoveMechanism;
use crate::metrics::Metrics;
use crate::world::SchellingWorld;

/// シミュレーション全体の実行結果
pub struct SimulationResult {
    pub metrics_history: Vec<Metrics>,
    pub converged: bool,
    pub final_iteration: usize,
}

// 単一の root シードから用途別の独立な決定論的 RNG ストリームを派生させるための
// ラベル．`derive_seed(root, &[label])` で互いに無相関なシードを得る．
/// 初期配置シャッフル用 RNG のラベル．
const RNG_WORLD_INIT: u64 = 0;
/// socsim エンジン(アクティベーション順序スケジューラ)用 RNG のラベル．
const RNG_ENGINE: u64 = 1;

/// グリッドを乱数で初期化し，占有インデックスと色マップを構築する．
///
/// 旧実装と同じく，シャッフルした位置に集団 A・B を配置したのち，占有セルを
/// **行優先**で走査して `AgentId(0..)` を順に割り当てる(配置順ではなく位置順)．
/// これにより決定論的な ID 割り当てが保たれる．
pub fn init_world(cfg: &Config, rng: &mut SimRng) -> (GridIndex, BTreeMap<AgentId, Cell>) {
    let total = cfg.rows * cfg.cols;
    assert!(
        cfg.n_a + cfg.n_b <= total,
        "エージェント数 ({}) がグリッドサイズ ({}) を超えています",
        cfg.n_a + cfg.n_b,
        total
    );

    let mut positions: Vec<(usize, usize)> = (0..cfg.rows)
        .flat_map(|r| (0..cfg.cols).map(move |c| (r, c)))
        .collect();
    positions.shuffle(rng);

    // セル配色をいったん 2次元配列に確定させる(旧実装と同じ手順)．
    let mut cells = vec![vec![Cell::Empty; cfg.cols]; cfg.rows];
    for i in 0..cfg.n_a {
        let (r, c) = positions[i];
        cells[r][c] = Cell::GroupA;
    }
    for i in 0..cfg.n_b {
        let (r, c) = positions[cfg.n_a + i];
        cells[r][c] = Cell::GroupB;
    }

    // 占有セルを行優先で走査し，AgentId(0..) を割り当ててインデックス・色マップへ．
    let mut index = GridIndex::new(Grid::new(cfg.rows, cfg.cols, Boundary::Fixed));
    let mut colors: BTreeMap<AgentId, Cell> = BTreeMap::new();
    let mut next_id = 0u64;
    for r in 0..cfg.rows {
        for c in 0..cfg.cols {
            if cells[r][c] != Cell::Empty {
                let id = AgentId(next_id);
                index.place(id, r, c).expect("初期配置に失敗");
                colors.insert(id, cells[r][c]);
                next_id += 1;
            }
        }
    }

    (index, colors)
}

/// シミュレーションを実行する．
///
/// socsim フレームワークの [`Simulation`](socsim_engine::Simulation) エンジンを内部で
/// 駆動する．移動規則は [`SchellingMoveMechanism`] が `Decision` フェーズで適用し，
/// アクティベーション順序は [`RandomActivationScheduler`] が毎ステップ決定する．
///
/// 早期停止は移動メカニズムが [`StepContext::request_stop`](socsim_core::StepContext::request_stop)
/// で要求し，ドライバは [`Simulation::stop_requested`](socsim_engine::Simulation::stop_requested)
/// を見てループを抜ける．ステップ結果(移動数・不満足数・収束)は
/// [`Simulation::scratch`](socsim_engine::Simulation::scratch) 経由で受け取る．
pub fn run(cfg: &Config) -> SimulationResult {
    // 出力ディレクトリの準備
    let snapshots_dir = format!("{}/snapshots", cfg.output_dir);
    fs::create_dir_all(&snapshots_dir).expect("スナップショットディレクトリの作成に失敗");

    // 乱数シード(未指定ならランダム)．単一 root から用途別シードを派生させる．
    let root = cfg.seed.unwrap_or_else(rand::random);

    // グリッド初期化(配置シャッフル用の RNG．root から派生)．
    let mut init_rng = SimRng::from_seed(derive_seed(root, &[RNG_WORLD_INIT]));
    let (index, colors) = init_world(cfg, &mut init_rng);

    // 世界状態とエンジンを構築(エンジン RNG も root から別ラベルで派生)．
    let world = SchellingWorld::new(index, colors, cfg.rule, cfg.max_iterations as u64);
    let mut sim = SimulationBuilder::new(world)
        .scheduler(Box::new(RandomActivationScheduler))
        .seed(derive_seed(root, &[RNG_ENGINE]))
        .add_mechanism(Box::new(SchellingMoveMechanism))
        .build();

    // メトリクス履歴
    let mut metrics_history: Vec<Metrics> = Vec::new();

    // 初期状態(step 0)を記録・保存
    metrics_history.push(Metrics::compute(sim.world(), 0, 0, 0));
    if cfg.snapshot_interval > 0 {
        save_snapshot(sim.world(), 0, &snapshots_dir);
    }

    for iteration in 1..=cfg.max_iterations {
        // 1ステップ実行(メカニズムが scratch に結果を書き，必要なら停止要求)．
        sim.step().expect("シミュレーションステップの実行に失敗");

        let n_dissatisfied = *sim
            .scratch()
            .get::<usize>("n_dissatisfied")
            .expect("n_dissatisfied が scratch に存在しません");
        let n_moved = *sim
            .scratch()
            .get::<usize>("n_moved")
            .expect("n_moved が scratch に存在しません");
        let converged = *sim
            .scratch()
            .get::<bool>("converged")
            .expect("converged が scratch に存在しません");

        // メトリクスを記録
        metrics_history.push(Metrics::compute(
            sim.world(),
            iteration,
            n_dissatisfied,
            n_moved,
        ));

        // スナップショットを保存
        if cfg.snapshot_interval > 0 && iteration % cfg.snapshot_interval == 0 {
            save_snapshot(sim.world(), iteration, &snapshots_dir);
        }

        // メカニズムが停止を要求していれば終了．
        if sim.stop_requested() {
            return SimulationResult {
                metrics_history,
                converged,
                final_iteration: iteration,
            };
        }
    }

    SimulationResult {
        metrics_history,
        converged: false,
        final_iteration: cfg.max_iterations,
    }
}

/// グリッドスナップショットをCSVに保存する
/// フォーマット: row,col,cell  (cell: 0=空, 1=A, 2=B)
pub fn save_snapshot(world: &SchellingWorld, step: usize, dir: &str) {
    let path = format!("{}/step_{:05}.csv", dir, step);
    let file = File::create(&path).expect("スナップショットファイルの作成に失敗");
    let mut wtr = Writer::from_writer(BufWriter::new(file));
    wtr.write_record(["row", "col", "cell"])
        .expect("ヘッダ書き込みに失敗");
    for r in 0..world.rows() {
        for c in 0..world.cols() {
            wtr.write_record(&[
                r.to_string(),
                c.to_string(),
                world.cell_color(r, c).to_int().to_string(),
            ])
            .expect("レコード書き込みに失敗");
        }
    }
    wtr.flush().expect("フラッシュに失敗");
}

/// メトリクス履歴をCSVに保存する
pub fn save_metrics(metrics: &[Metrics], output_dir: &str) {
    let path = format!("{}/metrics.csv", output_dir);
    let file = File::create(&path).expect("メトリクスファイルの作成に失敗");
    let mut wtr = Writer::from_writer(BufWriter::new(file));
    for m in metrics {
        wtr.serialize(m).expect("メトリクス書き込みに失敗");
    }
    wtr.flush().expect("フラッシュに失敗");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SatisfactionRule;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// テスト用に一意な出力ディレクトリを払い出す．
    fn temp_output_dir() -> String {
        static N: AtomicUsize = AtomicUsize::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "schelling_test_{}_{}_{}",
            std::process::id(),
            n,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        dir.to_string_lossy().into_owned()
    }

    fn test_config(output_dir: String) -> Config {
        Config {
            rows: 13,
            cols: 16,
            n_a: 73,
            n_b: 73,
            rule: SatisfactionRule::Ratio { threshold: 0.5 },
            max_iterations: 200,
            seed: Some(42),
            snapshot_interval: 0, // I/O を抑える
            output_dir,
        }
    }

    /// 同一シードなら socsim エンジン経由でも結果が完全に再現する．
    #[test]
    fn same_seed_is_deterministic() {
        let a = run(&test_config(temp_output_dir()));
        let b = run(&test_config(temp_output_dir()));
        assert_eq!(a.converged, b.converged);
        assert_eq!(a.final_iteration, b.final_iteration);
        assert_eq!(a.metrics_history.len(), b.metrics_history.len());
        for (ma, mb) in a.metrics_history.iter().zip(&b.metrics_history) {
            assert_eq!(ma.n_dissatisfied, mb.n_dissatisfied);
            assert_eq!(ma.n_moved, mb.n_moved);
            assert!((ma.avg_same_ratio - mb.avg_same_ratio).abs() < 1e-12);
        }
    }

    /// 旧実装のセマンティクス: 各ステップの移動数は開始時不満足数以下．
    #[test]
    fn moved_never_exceeds_dissatisfied() {
        let result = run(&test_config(temp_output_dir()));
        for m in &result.metrics_history {
            assert!(
                m.n_moved <= m.n_dissatisfied,
                "step {}: n_moved={} > n_dissatisfied={}",
                m.step,
                m.n_moved,
                m.n_dissatisfied
            );
        }
    }
}
