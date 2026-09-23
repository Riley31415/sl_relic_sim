//! Outcome tables for one attempt: every (glory, despair) it can end on, with
//! cumulative probability, solved exactly once and shared by every run.

use std::sync::{Arc, LazyLock, RwLock};

use rustc_hash::FxHashMap;

use super::rules::Relic;
use crate::solver::{Config, Solver, Strategy};

/// Which attempt: the inheritor level it is made at, and how it is played.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TableKey {
    /// all-or-nothing for (glory, despair); None = any despair
    Target { level: u8, glory: u8, despair: Option<u8> },
    /// for a weighted score; weights stored as f64 bits so the key hashes
    Score { level: u8, w_glory: u64, w_despair: u64 },
}

impl TableKey {
    pub fn score(level: u8, (w_glory, w_despair): (f64, f64)) -> Self {
        TableKey::Score { level, w_glory: w_glory.to_bits(), w_despair: w_despair.to_bits() }
    }

    fn solve(self) -> Table {
        let (level, strategy) = match self {
            TableKey::Target { level, glory, despair } => {
                let any = Config::for_level(i64::from(level), Strategy::default()).expect("level >= 1").slots;
                (level, Strategy::target(glory, despair.unwrap_or(any)))
            }
            TableKey::Score { level, w_glory, w_despair } => {
                (level, Strategy::score(f64::from_bits(w_glory), f64::from_bits(w_despair)))
            }
        };
        let cfg = Config::for_level(i64::from(level), strategy).expect("level >= 1");
        let (values, cum) = Solver::new(cfg).analyse(None).outcome_table();
        Table { values, cum }
    }
}

pub struct Table {
    values: Vec<Relic>,
    cum: Vec<f64>,
}

impl Table {
    #[cfg(test)]
    pub fn new(values: Vec<Relic>, cum: Vec<f64>) -> Self {
        Table { values, cum }
    }

    /// The outcome for a uniform draw `r` in [0, 1).
    pub fn sample(&self, r: f64) -> Relic {
        let i = self.cum.partition_point(|&c| c <= r);
        self.values[i.min(self.values.len() - 1)]
    }
}

/// Every table solved so far in this process.
static SOLVED: LazyLock<RwLock<FxHashMap<TableKey, Arc<Table>>>> = LazyLock::new(Default::default);

fn shared(key: TableKey) -> Arc<Table> {
    if let Some(table) = SOLVED.read().expect("table cache").get(&key) {
        return table.clone();
    }
    // solved outside the lock: two runs asking at once both solve, harmlessly
    let table = Arc::new(key.solve());
    SOLVED.write().expect("table cache").entry(key).or_insert(table).clone()
}

/// A run's own view of the shared tables, so the hot loop does not touch the
/// shared lock once it has seen a table.
#[derive(Default)]
pub struct LocalTables {
    seen: FxHashMap<TableKey, Arc<Table>>,
}

impl LocalTables {
    pub fn get(&mut self, key: TableKey) -> &Table {
        self.seen.entry(key).or_insert_with(|| shared(key))
    }
}
