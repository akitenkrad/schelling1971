//! Implementation of the analytical models (Bounded-Neighborhood Model + Tipping Model).
//!
//! Corresponds to the analytical portion of Schelling (1971) pp.167--186.
//! Abstracts away spatial arrangements and treats phase-plane dynamics using only aggregate populations (W, B) as state variables.
//!
//! - [`tolerance`]   Tolerance schedule (CDF) types.
//! - [`reaction`]    Reaction curves converting ratios to absolute counts.
//! - [`phase`]       Phase-plane analysis: equilibrium search and stability assessment.
//! - [`dynamics`]    Time-evolution engine (continuous Euler / discrete batch).
//! - [`tipping`]     Tipping extensions (speculative exit, asymmetric flow rates, and type classification).
//! - [`preset`]      Preset configurations based on the paper.
//! - [`runner`]      I/O orchestration invoked from the CLI.

pub mod dynamics;
pub mod phase;
pub mod preset;
pub mod reaction;
pub mod runner;
pub mod tipping;
pub mod tolerance;
