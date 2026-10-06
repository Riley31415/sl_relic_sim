//! Slayer Legend relic inheritance.
//!
//! - [`solver`]: one inheritance attempt, solved exactly (expectimax over
//!   every reachable state), plus the reports STRATEGY.md is written from
//! - [`heuristics`]: rules a player can hold in their head, scored exactly
//! - [`economy`]: the diamond cost of farming a crit relic (COST.md)
//! - [`levels`]: raising the inheritor level with pity, by Monte Carlo, and
//!   the searches for the cheapest plans (LEVELS.md)

pub mod economy;
pub mod format;
pub mod heuristics;
pub mod levels;
pub mod rng;
pub mod solve_report;
pub mod solver;
