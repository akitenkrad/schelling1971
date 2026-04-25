mod analytic;
mod config;
mod grid;
mod metrics;
mod simulation;

use std::fs::{self, File};
use std::io::BufWriter;
use std::path::Path;

use chrono::Local;
use clap::{Parser, Subcommand};
use config::{Config, SatisfactionRule};
use csv::Writer;
use simulation::{run, save_metrics};

use analytic::dynamics::{DynamicsConfig, FlowModel};
use analytic::phase::PhaseConfig;
use analytic::runner::{
    cmd_bnm, cmd_bnm_basin, cmd_tipping, BnmBasinArgs, BnmRunArgs, TippingRunArgs,
};
use analytic::tipping::{FlowAsymmetry, Speculation, TippingConfig};
use analytic::tolerance::ToleranceSchedule;

// ---------------------------------------------------------------------------
// CLI 定義
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(
    name = "schelling",
    about = "Schelling (1971) Dynamic Models of Segregation — 再現実験"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// 単一シミュレーションを実行する（デフォルト）
    Run(RunArgs),
    /// パラメータ感度解析（グリッドサーチ）を実行する
    Sweep(SweepArgs),
    /// 境界近隣モデル（解析モデル）の単発実行
    Bnm(BnmArgs),
    /// 境界近隣モデルの吸引域解析（初期条件グリッド掃き）
    BnmBasin(BnmBasinCliArgs),
    /// ティッピングモデル（投機・非対称・チャネリングを含む拡張動学）
    Tipping(TippingArgs),
}

#[derive(Parser, Debug)]
struct RunArgs {
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

    /// 許容限界 τ: 同色近隣比率の最小要求値 (--rule 未指定時のみ使用)
    #[arg(long, default_value_t = 0.333)]
    threshold: f64,

    /// 満足判定ルール: "ratio:X" (分離型) / "min-same:N" (集会型, Fig.16) /
    /// "bounded:L:H" (統合型, Fig.17)．省略時は --threshold から ratio ルールを構築する．
    #[arg(long)]
    rule: Option<String>,

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

#[derive(Parser, Debug)]
struct SweepArgs {
    /// 許容限界 τ の範囲 ("start:stop:step" または単一値)
    #[arg(long, default_value = "0.333")]
    threshold: String,

    /// 空き地率の範囲 ("start:stop:step" または単一値)
    #[arg(long, default_value = "0.30")]
    vacant_rate: String,

    /// グリッドの行数
    #[arg(long, default_value_t = 13)]
    rows: usize,

    /// グリッドの列数
    #[arg(long, default_value_t = 16)]
    cols: usize,

    /// カンマ区切りの乱数シード (例: "42,123,456")
    #[arg(long, default_value = "42")]
    seeds: String,

    /// 最大反復回数
    #[arg(long, default_value_t = 500)]
    max_iterations: usize,

    /// スナップショット保存間隔 (0 = 保存しない，sweepではデフォルト0)
    #[arg(long, default_value_t = 0)]
    snapshot_interval: usize,

    /// 結果出力ベースディレクトリ
    #[arg(long, default_value = "results")]
    output_dir: String,
}

// ---------------------------------------------------------------------------
// BNM 関連の CLI 引数
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
struct BnmArgs {
    /// プリセット名 (fig18 / fig19 / fig22 / fig23)．省略時は --w-tolerance 等を要求．
    #[arg(long)]
    preset: Option<String>,

    /// W 集団の許容スケジュール．"linear:r_max=2.0:pop_max=100" 形式．
    /// preset 指定時は上書きとして機能する．
    #[arg(long)]
    w_tolerance: Option<String>,

    /// B 集団の許容スケジュール．
    #[arg(long)]
    b_tolerance: Option<String>,

    /// 容量制約 W+B<=C．省略時は無制約．
    #[arg(long)]
    capacity: Option<f64>,

    /// 初期値 "W,B"．preset 指定時は preset の default_init が使われる．
    #[arg(long)]
    init: Option<String>,

    /// 流速モデル: "continuous:k_w=1.0:k_b=1.0:dt=0.1" / "discrete"
    #[arg(long, default_value = "continuous:k_w=1.0:k_b=1.0:dt=0.1")]
    flow: String,

    /// 最大ステップ数
    #[arg(long, default_value_t = 5000)]
    max_steps: usize,

    /// 収束許容誤差
    #[arg(long, default_value_t = 1e-4)]
    convergence_tol: f64,

    /// 結果出力ディレクトリ
    #[arg(long, default_value = "results")]
    output_dir: String,
}

#[derive(Parser, Debug)]
struct TippingArgs {
    /// プリセット名 (fig30a / fig30b / fig31 / fig32 など)．
    #[arg(long)]
    preset: Option<String>,

    #[arg(long)]
    w_tolerance: Option<String>,

    #[arg(long)]
    b_tolerance: Option<String>,

    #[arg(long)]
    capacity: Option<f64>,

    #[arg(long)]
    init: Option<String>,

    /// 投機モデル: "none" / "linear:alpha=0.3" / "trend:window=5:weight=0.5"
    #[arg(long, default_value = "none")]
    speculation: String,

    /// 流速非対称: "w_in=1.0:w_out=1.0:b_in=1.0:b_out=1.0" (省略時は対称)
    #[arg(long)]
    asymmetry: Option<String>,

    /// チャネリング (実効容量縮小係数 0..=1)．capacity と併用．
    #[arg(long)]
    channeling: Option<f64>,

    #[arg(long, default_value = "continuous:k_w=1.0:k_b=1.0:dt=0.1")]
    flow: String,

    #[arg(long, default_value_t = 5000)]
    max_steps: usize,

    #[arg(long, default_value_t = 1e-4)]
    convergence_tol: f64,

    #[arg(long, default_value = "results")]
    output_dir: String,
}

#[derive(Parser, Debug)]
struct BnmBasinCliArgs {
    #[arg(long)]
    preset: Option<String>,

    #[arg(long)]
    w_tolerance: Option<String>,

    #[arg(long)]
    b_tolerance: Option<String>,

    #[arg(long)]
    capacity: Option<f64>,

    /// 初期条件グリッドの分割数 "n_w x n_b"．
    #[arg(long, default_value = "20x20")]
    init_grid: String,

    #[arg(long, default_value = "continuous:k_w=1.0:k_b=1.0:dt=0.1")]
    flow: String,

    #[arg(long, default_value_t = 3000)]
    max_steps: usize,

    #[arg(long, default_value_t = 1e-3)]
    convergence_tol: f64,

    #[arg(long, default_value = "results")]
    output_dir: String,
}

// ---------------------------------------------------------------------------
// BNM 引数パーサ
// ---------------------------------------------------------------------------

/// "linear:r_max=2.0:pop_max=100" 等の文字列を ToleranceSchedule にパースする．
fn parse_tolerance_string(s: &str) -> ToleranceSchedule {
    let parts: Vec<&str> = s.split(':').collect();
    let kind = parts[0];
    let mut kwargs: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for kv in &parts[1..] {
        let mut it = kv.splitn(2, '=');
        let k = it.next().expect("key=value 形式が必要").to_string();
        let v: f64 = it
            .next()
            .expect("key=value 形式が必要")
            .parse()
            .expect("数値のパースに失敗");
        kwargs.insert(k, v);
    }
    let pop_max = *kwargs.get("pop_max").expect("pop_max が必要");
    match kind {
        "linear" => {
            let r_max = *kwargs.get("r_max").expect("r_max が必要");
            ToleranceSchedule::Linear { r_max, pop_max }
        }
        "affine" => {
            let intercept_pop = *kwargs.get("intercept_pop").unwrap_or(&0.0);
            let slope = *kwargs.get("slope").expect("slope が必要");
            ToleranceSchedule::Affine {
                intercept_pop,
                slope,
                pop_max,
            }
        }
        _ => panic!("未対応のスケジュール種別: \"{}\" (linear / affine)", kind),
    }
}

/// "continuous:k_w=1.0:k_b=1.0:dt=0.1" / "discrete" を FlowModel にパースする．
fn parse_flow_string(s: &str) -> FlowModel {
    let parts: Vec<&str> = s.split(':').collect();
    match parts[0] {
        "continuous" => {
            let mut kwargs: std::collections::HashMap<String, f64> =
                std::collections::HashMap::new();
            for kv in &parts[1..] {
                let mut it = kv.splitn(2, '=');
                let k = it.next().expect("key=value 形式が必要").to_string();
                let v: f64 = it
                    .next()
                    .expect("key=value 形式が必要")
                    .parse()
                    .expect("数値のパースに失敗");
                kwargs.insert(k, v);
            }
            FlowModel::Continuous {
                k_w: *kwargs.get("k_w").unwrap_or(&1.0),
                k_b: *kwargs.get("k_b").unwrap_or(&1.0),
                dt: *kwargs.get("dt").unwrap_or(&0.1),
            }
        }
        "discrete" => FlowModel::DiscreteBatch,
        _ => panic!("未対応の flow 種別: \"{}\" (continuous / discrete)", parts[0]),
    }
}

/// "W,B" を tuple にパースする．
fn parse_init_string(s: &str) -> (f64, f64) {
    let parts: Vec<&str> = s.split(',').collect();
    assert_eq!(parts.len(), 2, "init は \"W,B\" 形式");
    let w: f64 = parts[0].trim().parse().expect("W のパースに失敗");
    let b: f64 = parts[1].trim().parse().expect("B のパースに失敗");
    (w, b)
}

/// "20x20" を (20, 20) にパースする．
fn parse_grid_string(s: &str) -> (usize, usize) {
    let parts: Vec<&str> = s.split('x').collect();
    assert_eq!(parts.len(), 2, "init-grid は \"NxM\" 形式");
    let n: usize = parts[0].trim().parse().expect("N のパースに失敗");
    let m: usize = parts[1].trim().parse().expect("M のパースに失敗");
    (n, m)
}

/// BnmArgs から PhaseConfig + 初期値 + プリセット名 を組み立てる．
fn build_bnm_inputs(
    preset: Option<String>,
    w_tol: Option<String>,
    b_tol: Option<String>,
    capacity: Option<f64>,
    init: Option<String>,
) -> (Option<String>, PhaseConfig, (f64, f64)) {
    if let Some(name) = &preset {
        let p = analytic::preset::lookup(name).unwrap_or_else(|| {
            panic!(
                "未知のプリセット: \"{}\" (利用可能: {:?})",
                name,
                analytic::preset::all_names()
            )
        });
        let mut phase = p.phase;
        if let Some(s) = w_tol {
            phase.w_schedule = parse_tolerance_string(&s);
        }
        if let Some(s) = b_tol {
            phase.b_schedule = parse_tolerance_string(&s);
        }
        if let Some(c) = capacity {
            phase.capacity = Some(c);
        }
        let init = init
            .map(|s| parse_init_string(&s))
            .unwrap_or(p.default_init);
        (Some(name.clone()), phase, init)
    } else {
        let w_schedule =
            parse_tolerance_string(&w_tol.expect("preset 未指定時は --w-tolerance が必要"));
        let b_schedule =
            parse_tolerance_string(&b_tol.expect("preset 未指定時は --b-tolerance が必要"));
        let phase = PhaseConfig {
            w_schedule,
            b_schedule,
            capacity,
        };
        let init = init.map(|s| parse_init_string(&s)).unwrap_or((0.0, 0.0));
        (None, phase, init)
    }
}

// ---------------------------------------------------------------------------
// BNM サブコマンド本体
// ---------------------------------------------------------------------------

fn cmd_bnm_dispatch(args: BnmArgs) {
    let (preset_name, phase, init) =
        build_bnm_inputs(args.preset, args.w_tolerance, args.b_tolerance, args.capacity, args.init);
    let dynamics = DynamicsConfig {
        flow: parse_flow_string(&args.flow),
        max_steps: args.max_steps,
        convergence_tol: args.convergence_tol,
    };
    cmd_bnm(BnmRunArgs {
        preset_name,
        phase,
        dynamics,
        init,
        output_base: args.output_dir,
    });
}

/// 投機文字列をパース．
fn parse_speculation_string(s: &str) -> Speculation {
    let parts: Vec<&str> = s.split(':').collect();
    match parts[0] {
        "none" => Speculation::None,
        "linear" => {
            let mut alpha = 0.0_f64;
            for kv in &parts[1..] {
                let mut it = kv.splitn(2, '=');
                let k = it.next().unwrap_or("");
                let v: f64 = it.next().unwrap_or("0").parse().unwrap_or(0.0);
                if k == "alpha" {
                    alpha = v;
                }
            }
            Speculation::Linear { alpha }
        }
        "trend" => {
            let mut window = 5_usize;
            let mut weight = 0.5_f64;
            for kv in &parts[1..] {
                let mut it = kv.splitn(2, '=');
                let k = it.next().unwrap_or("");
                let v = it.next().unwrap_or("0");
                match k {
                    "window" => window = v.parse().unwrap_or(5),
                    "weight" => weight = v.parse().unwrap_or(0.5),
                    _ => {}
                }
            }
            Speculation::Trend { window, weight }
        }
        _ => panic!("未対応の投機モデル: \"{}\" (none / linear / trend)", parts[0]),
    }
}

/// 流速非対称文字列をパース．
fn parse_asymmetry_string(s: &str) -> FlowAsymmetry {
    let mut a = FlowAsymmetry {
        w_inflow: 1.0,
        w_outflow: 1.0,
        b_inflow: 1.0,
        b_outflow: 1.0,
    };
    for kv in s.split(':') {
        let mut it = kv.splitn(2, '=');
        let k = it.next().unwrap_or("");
        let v: f64 = it.next().unwrap_or("1").parse().unwrap_or(1.0);
        match k {
            "w_in" => a.w_inflow = v,
            "w_out" => a.w_outflow = v,
            "b_in" => a.b_inflow = v,
            "b_out" => a.b_outflow = v,
            _ => panic!("未対応のキー: \"{}\" (w_in/w_out/b_in/b_out)", k),
        }
    }
    a
}

fn cmd_tipping_dispatch(args: TippingArgs) {
    let (preset_name, phase, init) = build_bnm_inputs(
        args.preset,
        args.w_tolerance,
        args.b_tolerance,
        args.capacity,
        args.init,
    );
    let dynamics = DynamicsConfig {
        flow: parse_flow_string(&args.flow),
        max_steps: args.max_steps,
        convergence_tol: args.convergence_tol,
    };
    let speculation = parse_speculation_string(&args.speculation);
    let asymmetry = args.asymmetry.map(|s| parse_asymmetry_string(&s));
    let tipping = TippingConfig {
        phase,
        dynamics,
        speculation,
        asymmetry,
        channeling: args.channeling,
    };
    cmd_tipping(TippingRunArgs {
        preset_name,
        tipping,
        init,
        output_base: args.output_dir,
    });
}

fn cmd_bnm_basin_dispatch(args: BnmBasinCliArgs) {
    let (preset_name, phase, _) = build_bnm_inputs(
        args.preset,
        args.w_tolerance,
        args.b_tolerance,
        args.capacity,
        None,
    );
    let dynamics = DynamicsConfig {
        flow: parse_flow_string(&args.flow),
        max_steps: args.max_steps,
        convergence_tol: args.convergence_tol,
    };
    let (n_w, n_b) = parse_grid_string(&args.init_grid);
    cmd_bnm_basin(BnmBasinArgs {
        preset_name,
        phase,
        dynamics,
        n_w,
        n_b,
        output_base: args.output_dir,
    });
}

// ---------------------------------------------------------------------------
// レンジ文字列のパーサ
// ---------------------------------------------------------------------------

/// 小数点以下の桁数を文字列表現から推定する
fn step_decimals(v: f64) -> usize {
    let s = format!("{}", v);
    match s.find('.') {
        Some(pos) => s.len() - pos - 1,
        None => 0,
    }
}

/// "start:stop:step" → 等差数列，単一値 → 1要素のVecを返す．
/// 浮動小数点の誤差を許容するため，ステップ数を整数で算出する．
fn parse_range(s: &str) -> Vec<f64> {
    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        1 => {
            let v: f64 = parts[0].parse().expect("数値のパースに失敗");
            vec![v]
        }
        3 => {
            let start: f64 = parts[0].parse().expect("start のパースに失敗");
            let stop: f64 = parts[1].parse().expect("stop のパースに失敗");
            let step: f64 = parts[2].parse().expect("step のパースに失敗");
            assert!(step > 0.0, "step は正の値でなければなりません");
            // ステップ数を許容誤差付きで算出
            let n_steps = ((stop - start) / step + 0.5e-9).floor() as usize;
            // 小数点以下の桁数を step から推定し，丸めて浮動小数点誤差を除去
            let decimals = step_decimals(step);
            let factor = 10_f64.powi(decimals as i32);
            (0..=n_steps)
                .map(|i| ((start + step * i as f64) * factor).round() / factor)
                .collect()
        }
        _ => panic!("レンジ文字列の形式が不正です: \"{}\" (\"start:stop:step\" または単一値)", s),
    }
}

/// 文字列を SatisfactionRule にパースする．
///
/// - "ratio:0.333"      → Ratio { threshold: 0.333 }
/// - "min-same:3"       → MinSame { min_same: 3 }
/// - "bounded:3:6"      → Bounded { min_same: 3, max_same: 6 }
fn parse_rule_string(s: &str) -> SatisfactionRule {
    let parts: Vec<&str> = s.split(':').collect();
    match parts.as_slice() {
        ["ratio", t] => {
            let threshold: f64 = t.parse().expect("ratio の閾値パースに失敗");
            SatisfactionRule::Ratio { threshold }
        }
        ["min-same", n] => {
            let min_same: usize = n.parse().expect("min-same の値パースに失敗");
            SatisfactionRule::MinSame { min_same }
        }
        ["bounded", lo, hi] => {
            let min_same: usize = lo.parse().expect("bounded の下限パースに失敗");
            let max_same: usize = hi.parse().expect("bounded の上限パースに失敗");
            assert!(
                min_same <= max_same,
                "bounded ルールは下限 ({}) <= 上限 ({}) である必要があります",
                min_same,
                max_same
            );
            SatisfactionRule::Bounded { min_same, max_same }
        }
        _ => panic!(
            "不正なルール形式: \"{}\" (ratio:X, min-same:N, bounded:L:H のいずれか)",
            s
        ),
    }
}

// ---------------------------------------------------------------------------
// sweep_summary.csv の1行
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
struct SweepRow {
    threshold: f64,
    vacant_rate: f64,
    rows: usize,
    cols: usize,
    seed: u64,
    converged: bool,
    final_iteration: usize,
    avg_same_ratio: f64,
    avg_same_ratio_a: f64,
    avg_same_ratio_b: f64,
    pct_no_opposite: f64,
    dissimilarity_index: f64,
    n_dissatisfied_final: usize,
    n_moved_final: usize,
}

// ---------------------------------------------------------------------------
// sweep_config.json 用の構造体
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
struct SweepConfigJson {
    threshold: serde_json::Value,
    vacant_rate: serde_json::Value,
    rows: usize,
    cols: usize,
    seeds: Vec<u64>,
    max_iterations: usize,
    snapshot_interval: usize,
}

// ---------------------------------------------------------------------------
// config.json (run 用) の構造体
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
struct RunConfigJson {
    command: &'static str,
    rule: String,
    rule_kind: &'static str,
    threshold: Option<f64>,
    min_same: Option<usize>,
    max_same: Option<usize>,
    rows: usize,
    cols: usize,
    n_a: usize,
    n_b: usize,
    n_vacant: usize,
    vacant_rate: f64,
    seed: Option<u64>,
    max_iterations: usize,
    snapshot_interval: usize,
    output_dir: String,
}

fn run_config_json(cfg: &Config, vacant_rate: f64) -> RunConfigJson {
    let total = cfg.rows * cfg.cols;
    let n_vacant = total.saturating_sub(cfg.n_a + cfg.n_b);
    let (rule_kind, threshold, min_same, max_same) = match cfg.rule {
        SatisfactionRule::Ratio { threshold } => ("ratio", Some(threshold), None, None),
        SatisfactionRule::MinSame { min_same } => ("min-same", None, Some(min_same), None),
        SatisfactionRule::Bounded { min_same, max_same } => {
            ("bounded", None, Some(min_same), Some(max_same))
        }
    };
    RunConfigJson {
        command: "run",
        rule: cfg.rule.label(),
        rule_kind,
        threshold,
        min_same,
        max_same,
        rows: cfg.rows,
        cols: cfg.cols,
        n_a: cfg.n_a,
        n_b: cfg.n_b,
        n_vacant,
        vacant_rate,
        seed: cfg.seed,
        max_iterations: cfg.max_iterations,
        snapshot_interval: cfg.snapshot_interval,
        output_dir: cfg.output_dir.clone(),
    }
}

/// レンジ文字列をJSONに変換する（range → {start, stop, step}，単一値 → 数値）
fn range_to_json(s: &str) -> serde_json::Value {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() == 3 {
        serde_json::json!({
            "start": parts[0].parse::<f64>().unwrap(),
            "stop":  parts[1].parse::<f64>().unwrap(),
            "step":  parts[2].parse::<f64>().unwrap(),
        })
    } else {
        serde_json::json!(parts[0].parse::<f64>().unwrap())
    }
}

// ---------------------------------------------------------------------------
// run サブコマンド（既存の単一実行ロジック）
// ---------------------------------------------------------------------------

fn cmd_run(args: RunArgs) {
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

    let rule = match &args.rule {
        Some(s) => parse_rule_string(s),
        None => SatisfactionRule::Ratio { threshold: args.threshold },
    };

    let cfg = Config {
        rows: args.rows,
        cols: args.cols,
        n_a,
        n_b,
        rule,
        max_iterations: args.max_iterations,
        seed: args.seed,
        snapshot_interval: args.snapshot_interval,
        output_dir: output_dir.clone(),
    };

    println!("=== Schelling 分離モデル 再現実験 ===");
    println!(
        "グリッド: {}×{} | A: {} | B: {} | 空き: {} | ルール: {}",
        cfg.rows,
        cfg.cols,
        cfg.n_a,
        cfg.n_b,
        total - cfg.n_a - cfg.n_b,
        cfg.rule.label(),
    );
    println!("シード: {:?}", cfg.seed);
    println!("出力先: {}", cfg.output_dir);
    println!("---------------------------------------");

    let result = run(&cfg);
    save_metrics(&result.metrics_history, &cfg.output_dir);

    // config.json を保存
    {
        let path = format!("{}/config.json", cfg.output_dir);
        let file = File::create(&path).expect("config.json の作成に失敗");
        serde_json::to_writer_pretty(BufWriter::new(file), &run_config_json(&cfg, args.vacant_rate))
            .expect("config.json の書き込みに失敗");
    }

    // latest シンボリックリンクを作成・更新
    let symlink_path = Path::new(&args.output_dir).join("latest");
    if symlink_path.is_symlink() {
        let _ = fs::remove_file(&symlink_path);
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
    println!(
        "  集団A: {:.1}%  集団B: {:.1}%",
        last.avg_same_ratio_a * 100.0,
        last.avg_same_ratio_b * 100.0
    );
    println!("異色近隣なし割合: {:.1}%", last.pct_no_opposite);
    println!("メトリクス → {}/metrics.csv", cfg.output_dir);
    println!("設定       → {}/config.json", cfg.output_dir);
    println!("スナップショット → {}/snapshots/", cfg.output_dir);
}

// ---------------------------------------------------------------------------
// sweep サブコマンド
// ---------------------------------------------------------------------------

fn cmd_sweep(args: SweepArgs) {
    let thresholds = parse_range(&args.threshold);
    let vacant_rates = parse_range(&args.vacant_rate);
    let seeds: Vec<u64> = args
        .seeds
        .split(',')
        .map(|s| s.trim().parse::<u64>().expect("シードのパースに失敗"))
        .collect();

    // 全組み合わせのカルテシアン積を構築
    struct Combo {
        threshold: f64,
        vacant_rate: f64,
        seed: u64,
    }
    let mut combos: Vec<Combo> = Vec::new();
    for &tau in &thresholds {
        for &vac in &vacant_rates {
            for &seed in &seeds {
                combos.push(Combo {
                    threshold: tau,
                    vacant_rate: vac,
                    seed,
                });
            }
        }
    }
    let n_total = combos.len();

    // タイムスタンプ付きsweepディレクトリを生成
    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let sweep_dir = format!("{}/{}_sweep", args.output_dir, timestamp);
    fs::create_dir_all(&sweep_dir).expect("sweepディレクトリの作成に失敗");

    println!("=== Schelling 分離モデル パラメータスイープ ===");
    println!(
        "グリッド: {}×{} | τ: {} 値 | vacant_rate: {} 値 | シード: {} 個 | 合計: {} 実行",
        args.rows,
        args.cols,
        thresholds.len(),
        vacant_rates.len(),
        seeds.len(),
        n_total
    );
    println!("出力先: {}", sweep_dir);
    println!("-----------------------------------------------");

    let mut summary_rows: Vec<SweepRow> = Vec::with_capacity(n_total);

    for (i, combo) in combos.iter().enumerate() {
        let total = args.rows * args.cols;
        let n_vacant = (total as f64 * combo.vacant_rate).round() as usize;
        let n_agents = total - n_vacant;
        let n_a = n_agents / 2;
        let n_b = n_agents - n_a;

        let run_dir = format!(
            "{}/tau_{:.3}_vac_{:.3}_seed_{}",
            sweep_dir, combo.threshold, combo.vacant_rate, combo.seed
        );

        let cfg = Config {
            rows: args.rows,
            cols: args.cols,
            n_a,
            n_b,
            rule: SatisfactionRule::Ratio { threshold: combo.threshold },
            max_iterations: args.max_iterations,
            seed: Some(combo.seed),
            snapshot_interval: args.snapshot_interval,
            output_dir: run_dir.clone(),
        };

        let result = run(&cfg);
        save_metrics(&result.metrics_history, &cfg.output_dir);

        let last = result.metrics_history.last().unwrap();

        println!(
            "[{}/{}] τ={:.3} vacant={:.3} seed={} → converged={} iter={} avg_same={:.3}",
            i + 1,
            n_total,
            combo.threshold,
            combo.vacant_rate,
            combo.seed,
            if result.converged { "Yes" } else { "No" },
            result.final_iteration,
            last.avg_same_ratio,
        );

        summary_rows.push(SweepRow {
            threshold: combo.threshold,
            vacant_rate: combo.vacant_rate,
            rows: args.rows,
            cols: args.cols,
            seed: combo.seed,
            converged: result.converged,
            final_iteration: result.final_iteration,
            avg_same_ratio: last.avg_same_ratio,
            avg_same_ratio_a: last.avg_same_ratio_a,
            avg_same_ratio_b: last.avg_same_ratio_b,
            pct_no_opposite: last.pct_no_opposite,
            dissimilarity_index: last.dissimilarity_index,
            n_dissatisfied_final: last.n_dissatisfied,
            n_moved_final: last.n_moved,
        });
    }

    // sweep_summary.csv を保存
    {
        let path = format!("{}/sweep_summary.csv", sweep_dir);
        let file = File::create(&path).expect("sweep_summary.csv の作成に失敗");
        let mut wtr = Writer::from_writer(BufWriter::new(file));
        for row in &summary_rows {
            wtr.serialize(row).expect("サマリ行の書き込みに失敗");
        }
        wtr.flush().expect("フラッシュに失敗");
    }

    // sweep_config.json を保存
    {
        let config_json = SweepConfigJson {
            threshold: range_to_json(&args.threshold),
            vacant_rate: range_to_json(&args.vacant_rate),
            rows: args.rows,
            cols: args.cols,
            seeds: seeds.clone(),
            max_iterations: args.max_iterations,
            snapshot_interval: args.snapshot_interval,
        };
        let path = format!("{}/sweep_config.json", sweep_dir);
        let file = File::create(&path).expect("sweep_config.json の作成に失敗");
        serde_json::to_writer_pretty(BufWriter::new(file), &config_json)
            .expect("sweep_config.json の書き込みに失敗");
    }

    // latest シンボリックリンクを作成・更新
    let symlink_path = Path::new(&args.output_dir).join("latest");
    if symlink_path.is_symlink() {
        let _ = fs::remove_file(&symlink_path);
    }
    #[cfg(unix)]
    {
        let link_target = format!("{}_sweep", timestamp);
        let _ = std::os::unix::fs::symlink(&link_target, &symlink_path);
    }

    // サマリテーブルを表示
    println!("===============================================");
    println!("スイープ完了: {} 実行", n_total);
    println!("-----------------------------------------------");
    println!(
        "{:<10} {:<12} {:<6} {:<10} {:<6} {:<10}",
        "threshold", "vacant_rate", "seed", "converged", "iter", "avg_same"
    );
    println!("{}", "-".repeat(60));
    for row in &summary_rows {
        println!(
            "{:<10.3} {:<12.3} {:<6} {:<10} {:<6} {:.3}",
            row.threshold,
            row.vacant_rate,
            row.seed,
            if row.converged { "Yes" } else { "No" },
            row.final_iteration,
            row.avg_same_ratio,
        );
    }
    println!("-----------------------------------------------");
    println!("サマリ → {}/sweep_summary.csv", sweep_dir);
    println!("設定   → {}/sweep_config.json", sweep_dir);
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

/// サブコマンドなしで実行された場合に `run` として解釈するためのラッパ構造体．
/// clap の `try_parse_from` で先にフラット引数としてパースを試み，
/// 失敗した場合のみサブコマンド付きでパースする．
#[derive(Parser, Debug)]
#[command(
    name = "schelling",
    about = "Schelling (1971) Dynamic Models of Segregation — 再現実験"
)]
struct FlatRunCli {
    #[command(flatten)]
    args: RunArgs,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // 第1引数がサブコマンド名かどうかで分岐
    let has_subcommand = args
        .get(1)
        .map(|a| {
            a == "run"
                || a == "sweep"
                || a == "bnm"
                || a == "bnm-basin"
                || a == "tipping"
                || a == "help"
                || a == "--help"
                || a == "-h"
        })
        .unwrap_or(false);

    if has_subcommand {
        let cli = Cli::parse_from(&args);
        match cli.command {
            Some(Commands::Run(run_args)) => cmd_run(run_args),
            Some(Commands::Sweep(sweep_args)) => cmd_sweep(sweep_args),
            Some(Commands::Bnm(bnm_args)) => cmd_bnm_dispatch(bnm_args),
            Some(Commands::BnmBasin(basin_args)) => cmd_bnm_basin_dispatch(basin_args),
            Some(Commands::Tipping(tipping_args)) => cmd_tipping_dispatch(tipping_args),
            None => cmd_run(RunArgs::parse_from(args.iter().take(1))),
        }
    } else {
        // サブコマンドなしのフラット引数として解釈（後方互換性）
        let flat = FlatRunCli::parse_from(&args);
        cmd_run(flat.args);
    }
}
