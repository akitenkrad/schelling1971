use std::fs::{self, File};
use std::io::BufWriter;

use csv::Writer;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::config::Config;
use crate::grid::{Cell, Grid};
use crate::metrics::Metrics;

/// シミュレーション全体の実行結果
pub struct SimulationResult {
    pub metrics_history: Vec<Metrics>,
    pub converged: bool,
    pub final_iteration: usize,
}

/// グリッドを乱数で初期化する
pub fn init_grid(cfg: &Config, rng: &mut ChaCha8Rng) -> Grid {
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

    let mut cells = vec![vec![Cell::Empty; cfg.cols]; cfg.rows];
    for i in 0..cfg.n_a {
        let (r, c) = positions[i];
        cells[r][c] = Cell::GroupA;
    }
    for i in 0..cfg.n_b {
        let (r, c) = positions[cfg.n_a + i];
        cells[r][c] = Cell::GroupB;
    }
    Grid::new(cfg.rows, cfg.cols, cells)
}

/// シミュレーションを実行する
pub fn run(cfg: &Config) -> SimulationResult {
    // 出力ディレクトリの準備
    let snapshots_dir = format!("{}/snapshots", cfg.output_dir);
    fs::create_dir_all(&snapshots_dir).expect("スナップショットディレクトリの作成に失敗");

    // 乱数生成器
    let seed = cfg.seed.unwrap_or_else(rand::random);
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    // グリッド初期化
    let mut grid = init_grid(cfg, &mut rng);

    // メトリクス履歴
    let mut metrics_history: Vec<Metrics> = Vec::new();

    // 初期状態を保存
    let init_metrics = Metrics::compute(&grid, 0, 0, 0);
    metrics_history.push(init_metrics);
    if cfg.snapshot_interval > 0 {
        save_snapshot(&grid, 0, &snapshots_dir);
    }

    let mut converged = false;

    for iteration in 1..=cfg.max_iterations {
        // 不満足エージェントを収集
        let mut dissatisfied: Vec<(usize, usize)> = Vec::new();
        for r in 0..grid.rows {
            for c in 0..grid.cols {
                if grid.cells[r][c] != Cell::Empty
                    && !grid.is_satisfied(r, c, cfg.rule)
                {
                    dissatisfied.push((r, c));
                }
            }
        }

        let n_dissatisfied = dissatisfied.len();

        if n_dissatisfied == 0 {
            converged = true;
            // 収束時のメトリクスを記録
            let m = Metrics::compute(&grid, iteration, 0, 0);
            metrics_history.push(m);
            if cfg.snapshot_interval > 0 {
                save_snapshot(&grid, iteration, &snapshots_dir);
            }
            return SimulationResult {
                metrics_history,
                converged,
                final_iteration: iteration,
            };
        }

        // ランダム順序で処理
        dissatisfied.shuffle(&mut rng);

        let mut n_moved = 0usize;
        for (r, c) in &dissatisfied {
            // 他の移動で満足になっていたらスキップ
            if grid.is_satisfied(*r, *c, cfg.rule) {
                continue;
            }

            // 空きセルをチェビシェフ距離の昇順で探索
            let mut vacants = grid.vacant_cells();
            vacants.sort_by_key(|&v| Grid::chebyshev((*r, *c), v));

            for v in &vacants {
                if grid.will_be_satisfied_after_move((*r, *c), *v, cfg.rule) {
                    // 移動を実行
                    grid.cells[v.0][v.1] = grid.cells[*r][*c];
                    grid.cells[*r][*c] = Cell::Empty;
                    n_moved += 1;
                    break;
                }
            }
        }

        // メトリクスを記録
        let m = Metrics::compute(&grid, iteration, n_dissatisfied, n_moved);
        metrics_history.push(m);

        // スナップショットを保存
        if cfg.snapshot_interval > 0 && iteration % cfg.snapshot_interval == 0 {
            save_snapshot(&grid, iteration, &snapshots_dir);
        }

        // 移動がなければ（全員が移動先を見つけられない）終了
        if n_moved == 0 {
            return SimulationResult {
                metrics_history,
                converged: false,
                final_iteration: iteration,
            };
        }
    }

    SimulationResult {
        metrics_history,
        converged,
        final_iteration: cfg.max_iterations,
    }
}

/// グリッドスナップショットをCSVに保存する
/// フォーマット: row,col,cell  (cell: 0=空, 1=A, 2=B)
pub fn save_snapshot(grid: &Grid, step: usize, dir: &str) {
    let path = format!("{}/step_{:05}.csv", dir, step);
    let file = File::create(&path).expect("スナップショットファイルの作成に失敗");
    let mut wtr = Writer::from_writer(BufWriter::new(file));
    wtr.write_record(["row", "col", "cell"])
        .expect("ヘッダ書き込みに失敗");
    for r in 0..grid.rows {
        for c in 0..grid.cols {
            wtr.write_record(&[
                r.to_string(),
                c.to_string(),
                grid.cells[r][c].to_int().to_string(),
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
