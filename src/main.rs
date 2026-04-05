mod config;
mod grid;
mod metrics;
mod simulation;

use std::path::Path;

use chrono::Local;
use clap::Parser;
use config::Config;
use simulation::{run, save_metrics};

#[derive(Parser, Debug)]
#[command(
    name = "schelling",
    about = "Schelling (1971) Dynamic Models of Segregation — 再現実験"
)]
struct Args {
    /// グリッドの行数
    #[arg(long, default_value_t = 13)]
    rows: usize,

    /// グリッドの列数
    #[arg(long, default_value_t = 16)]
    cols: usize,

    /// 集団Aのエージェント数 (0 = 自動計算)
    #[arg(long, default_value_t = 0)]
    n_a: usize,

    /// 集団Bのエージェント数 (0 = 自動計算)
    #[arg(long, default_value_t = 0)]
    n_b: usize,

    /// 空き地率 [0, 1]
    #[arg(long, default_value_t = 0.30)]
    vacant_rate: f64,

    /// 許容限界 τ: 同色近隣比率の最小要求値
    #[arg(long, default_value_t = 0.333)]
    threshold: f64,

    /// 最大反復回数
    #[arg(long, default_value_t = 500)]
    max_iterations: usize,

    /// 乱数シード (省略時はランダム)
    #[arg(long)]
    seed: Option<u64>,

    /// スナップショット保存間隔 (0 = 保存しない)
    #[arg(long, default_value_t = 1)]
    snapshot_interval: usize,

    /// 結果出力ディレクトリ
    #[arg(long, default_value = "results")]
    output_dir: String,
}

fn main() {
    let args = Args::parse();

    // タイムスタンプ付きサブディレクトリを生成
    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let output_dir = format!("{}/{}", args.output_dir, timestamp);

    let total = args.rows * args.cols;
    let (n_a, n_b) = if args.n_a == 0 || args.n_b == 0 {
        let n_vacant = (total as f64 * args.vacant_rate).round() as usize;
        let n_agents = total - n_vacant;
        let a = n_agents / 2;
        (a, n_agents - a)
    } else {
        (args.n_a, args.n_b)
    };

    let cfg = Config {
        rows: args.rows,
        cols: args.cols,
        n_a,
        n_b,
        threshold: args.threshold,
        max_iterations: args.max_iterations,
        seed: args.seed,
        snapshot_interval: args.snapshot_interval,
        output_dir: output_dir.clone(),
    };

    println!("=== Schelling 分離モデル 再現実験 ===");
    println!(
        "グリッド: {}×{} | A: {} | B: {} | 空き: {} | τ: {:.3}",
        cfg.rows,
        cfg.cols,
        cfg.n_a,
        cfg.n_b,
        total - cfg.n_a - cfg.n_b,
        cfg.threshold
    );
    println!("シード: {:?}", cfg.seed);
    println!("出力先: {}", cfg.output_dir);
    println!("---------------------------------------");

    let result = run(&cfg);
    save_metrics(&result.metrics_history, &cfg.output_dir);

    // latest シンボリックリンクを作成・更新
    let symlink_path = Path::new(&args.output_dir).join("latest");
    if symlink_path.is_symlink() {
        let _ = std::fs::remove_file(&symlink_path);
    }
    #[cfg(unix)]
    {
        let _ = std::os::unix::fs::symlink(&timestamp, &symlink_path);
    }

    let last = result.metrics_history.last().unwrap();
    println!(
        "収束: {} | 反復回数: {}",
        if result.converged { "Yes" } else { "No" },
        result.final_iteration
    );
    println!("平均同色近隣比率: {:.1}%", last.avg_same_ratio * 100.0);
    println!("  集団A: {:.1}%  集団B: {:.1}%", last.avg_same_ratio_a * 100.0, last.avg_same_ratio_b * 100.0);
    println!("異色近隣なし割合: {:.1}%", last.pct_no_opposite);
    println!("メトリクス → {}/metrics.csv", cfg.output_dir);
    println!("スナップショット → {}/snapshots/", cfg.output_dir);
}
