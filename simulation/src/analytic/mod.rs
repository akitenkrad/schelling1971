//! 解析モデル (Bounded-Neighborhood Model + Tipping Model) の実装．
//!
//! Schelling (1971) pp.167--186 の解析パートに対応する．
//! 空間配置を捨象し，集計人口 (W, B) のみを状態変数として位相平面動学を扱う．
//!
//! - [`tolerance`]   許容限界スケジュール (CDF) の型．
//! - [`reaction`]    比率→絶対数変換による反応曲線．
//! - [`phase`]       位相平面解析: 平衡点の探索と安定性判定．
//! - [`dynamics`]    時間発展エンジン (連続Euler / 離散バッチ)．
//! - [`tipping`]     ティッピング拡張 (投機的退出・非対称流速・類型分類)．
//! - [`preset`]      論文準拠のプリセット設定．
//! - [`runner`]      CLI から呼ばれる I/O オーケストレーション．

pub mod tolerance;
pub mod reaction;
pub mod phase;
pub mod dynamics;
pub mod tipping;
pub mod preset;
pub mod runner;
