//! Outcome tables for one attempt: every (glory, despair) it can end on, with
//! cumulative probability, solved exactly once and shared by every run - and
//! kept on disk for the next process (see `disk`).

use std::sync::{Arc, LazyLock, OnceLock, RwLock};

use rustc_hash::FxHashMap;

use super::rules::{Bar, Relic};
use crate::solver::{Config, Objective, Solver, Strategy};

/// Which attempt: the inheritor level it is made at, and how it is played.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TableKey {
    /// the best chance of a relic's bar (`glory`+ / `despair`-), then
    /// closing the gap `then` = (glory, despair, steps off) - the goal's
    /// totals - then more glory and less despair (see `target`)
    Target { level: u8, glory: u8, despair: u8, then: (u8, u8, u8) },
    /// closing the gap to `glory` glory successes and at most `despair`
    /// despair successes, each within the board's slots, from a memory `gap`
    /// steps off (see `close`): every step closed counts; then more glory,
    /// less despair, weighted `tie` (glory : despair)
    Close { level: u8, glory: u8, despair: u8, gap: u8, tie: (u8, u8) },
    /// max amplification above `mark`%, a result kept from `least`% (one
    /// above the memory's; see `above`)
    Above { level: u8, mark: u8, least: u8 },
}

impl TableKey {
    /// Closing the gap from the memory `from` toward `glory` / `despair`,
    /// which may lie off the board (a total's gap can be more than one relic
    /// holds): both are brought within its slots, which changes no gap a
    /// result closes.  Memories as far off share a table.
    pub fn close(level: u8, glory: i64, despair: i64, from: Relic) -> Self {
        let slots = i64::from(slots_at(level));
        let clamp = |v: i64| v.clamp(0, slots) as u8;
        let (glory, despair) = (clamp(glory), clamp(despair));
        TableKey::Close { level, glory, despair, gap: gap_to((glory, despair), from) as u8, tie: (1, 1) }
    }

    /// The same, the rest settled `tie` (glory : despair) - a Close key only.
    pub fn tie(self, tie: (u8, u8)) -> Self {
        match self {
            TableKey::Close { level, glory, despair, gap, .. } => TableKey::Close { level, glory, despair, gap, tie },
            key => key,
        }
    }

    /// The best chance of `bar`, then closing the gap to `aim` (as for
    /// `close`) from the memory `from`.
    pub fn target(level: u8, bar: Bar, aim: (i64, i64), from: Relic) -> Self {
        let slots = slots_at(level);
        let TableKey::Close { glory, despair, gap, .. } = TableKey::close(level, aim.0, aim.1, from) else { unreachable!() };
        let bar_despair = bar.despair.map_or(slots, |d| d.min(slots));
        TableKey::Target { level, glory: bar.glory.min(slots), despair: bar_despair, then: (glory, despair, gap) }
    }

    /// Maximize amplification above a target, the minimum useful amplification `mark`%,
    /// over the memory `from`; memories at the same amplification share a
    /// table.
    pub fn above(level: u8, mark: u8, from: Relic) -> Self {
        let Objective::Above { mark, least } = Strategy::above(mark, Some(from)).objective else { unreachable!() };
        TableKey::Above { level, mark, least }
    }

    fn solve(self) -> Table {
        let (level, strategy) = match self {
            TableKey::Close { level, glory, despair, gap, tie: (wg, wd) } => (
                level,
                Strategy { objective: Objective::Close { glory, despair, gap }, ..Strategy::close(0, 0, (0, 0)) }.tie(wg, wd),
            ),
            TableKey::Target { level, glory, despair, then: (g, d, gap) } => {
                (level, Strategy::target(glory, despair).then_close(g, d, gap))
            }
            TableKey::Above { level, mark, least } => {
                (level, Strategy { objective: Objective::Above { mark, least }, ..Strategy::above(mark, None) })
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

/// Every table solved so far - in this process, or read back from disk.
type Cell = Arc<OnceLock<Table>>;
static SOLVED: LazyLock<RwLock<FxHashMap<TableKey, Cell>>> = LazyLock::new(|| RwLock::new(disk::load()));

/// The table for `key`, solved once: a run that asks while another solves it
/// waits for that solve rather than repeating it.
fn shared(key: TableKey) -> Cell {
    let found = SOLVED.read().expect("table cache").get(&key).cloned();
    let cell = found.unwrap_or_else(|| SOLVED.write().expect("table cache").entry(key).or_default().clone());
    cell.get_or_init(|| {
        let table = key.solve();
        disk::save(key, &table);
        table
    });
    cell
}

/// Solved tables kept between processes, in one file per version of the
/// solver (named for a hash of `solver.rs` and this file, so an edit to
/// either starts a fresh file): under `RELIC_TABLES` if set (a directory, or
/// "off" for none), else the system temp directory.  Each table is appended
/// as it is solved, with a checksum; a damaged tail (a process killed while
/// writing) is cut off on the next load.  Best effort: a file that cannot be
/// read or written only means solving again.
#[cfg(not(target_arch = "wasm32"))]
mod disk {
    use std::fs::{self, File, OpenOptions};
    use std::hash::{Hash, Hasher};
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::{Arc, LazyLock, Mutex, OnceLock};

    use rustc_hash::{FxHashMap, FxHasher};

    use super::{Cell, Table, TableKey};

    const SOURCES: [&str; 2] = [include_str!("../solver.rs"), include_str!("tables.rs")];

    fn path() -> Option<PathBuf> {
        let dir = match std::env::var_os("RELIC_TABLES") {
            Some(v) if v == "off" => return None,
            Some(v) => PathBuf::from(v),
            None => std::env::temp_dir().join("relic-tables"),
        };
        let mut h = FxHasher::default();
        SOURCES.hash(&mut h);
        Some(dir.join(format!("{:016x}.bin", h.finish())))
    }

    fn checksum(bytes: &[u8]) -> u64 {
        let mut h = FxHasher::default();
        bytes.hash(&mut h);
        h.finish()
    }

    fn key_bytes(key: TableKey) -> [u8; 8] {
        match key {
            TableKey::Target { level, glory, despair, then: (g, d, gap) } => [0, level, glory, despair, g, d, gap, 0],
            TableKey::Close { level, glory, despair, gap, tie: (wg, wd) } => [1, level, glory, despair, gap, wg, wd, 0],
            TableKey::Above { level, mark, least } => [2, level, mark, least, 0, 0, 0, 0],
        }
    }

    fn key_from(b: &[u8]) -> Option<TableKey> {
        match b[0] {
            0 => Some(TableKey::Target { level: b[1], glory: b[2], despair: b[3], then: (b[4], b[5], b[6]) }),
            1 => Some(TableKey::Close { level: b[1], glory: b[2], despair: b[3], gap: b[4], tie: (b[5], b[6]) }),
            2 => Some(TableKey::Above { level: b[1], mark: b[2], least: b[3] }),
            _ => None,
        }
    }

    /// One record: the key (8 bytes), the outcome count (2), the outcomes
    /// (2 each), their cumulative chances (8 each), then a checksum of all
    /// that (8).
    fn record(key: TableKey, table: &Table) -> Vec<u8> {
        let n = table.values.len();
        let mut out = Vec::with_capacity(18 + 10 * n);
        out.extend(key_bytes(key));
        out.extend((n as u16).to_le_bytes());
        for &(g, d) in &table.values {
            out.extend([g, d]);
        }
        for c in &table.cum {
            out.extend(c.to_le_bytes());
        }
        out.extend(checksum(&out).to_le_bytes());
        out
    }

    /// The record at the start of `b` and its length, if whole and sound.
    fn parse(b: &[u8]) -> Option<(TableKey, Table, usize)> {
        let n = usize::from(u16::from_le_bytes(b.get(8..10)?.try_into().ok()?));
        let len = 10 + 10 * n + 8;
        let body = b.get(..len - 8)?;
        if u64::from_le_bytes(b.get(len - 8..len)?.try_into().ok()?) != checksum(body) {
            return None;
        }
        let key = key_from(body)?;
        let values = body[10..10 + 2 * n].chunks(2).map(|p| (p[0], p[1])).collect();
        let cum = body[10 + 2 * n..].chunks(8).map(|c| f64::from_le_bytes(c.try_into().expect("8 bytes"))).collect();
        Some((key, Table { values, cum }, len))
    }

    pub(super) fn load() -> FxHashMap<TableKey, Cell> {
        let mut out = FxHashMap::default();
        let Some(path) = path() else { return out };
        let Ok(bytes) = fs::read(&path) else { return out };
        let mut at = 0;
        while let Some((key, table, len)) = parse(&bytes[at..]) {
            out.insert(key, Arc::new(OnceLock::from(table)));
            at += len;
        }
        if at < bytes.len() {
            // a damaged tail: cut it off, so what is appended next lines up
            let _ = OpenOptions::new().write(true).open(&path).and_then(|f| f.set_len(at as u64));
        }
        out
    }

    static FILE: LazyLock<Mutex<Option<File>>> = LazyLock::new(|| {
        let file = path().and_then(|path| {
            fs::create_dir_all(path.parent()?).ok()?;
            OpenOptions::new().create(true).append(true).open(path).ok()
        });
        Mutex::new(file)
    });

    pub(super) fn save(key: TableKey, table: &Table) {
        if let Some(file) = FILE.lock().expect("table file").as_mut() {
            let _ = file.write_all(&record(key, table));
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn a_record_reads_back_as_written_and_a_damaged_one_does_not() {
            let key = TableKey::Close { level: 18, glory: 9, despair: 0, gap: 4, tie: (2, 3) };
            let table = Table { values: vec![(7, 1), (8, 0), (9, 0)], cum: vec![0.25, 0.5, 1.0] };
            let bytes = record(key, &table);
            let (k, t, len) = parse(&bytes).unwrap();
            assert_eq!((k, t.values, t.cum, len), (key, table.values.clone(), table.cum.clone(), bytes.len()));
            let mut bad = bytes.clone();
            bad[12] ^= 1;
            assert!(parse(&bad).is_none());
            assert!(parse(&bytes[..bytes.len() - 1]).is_none());
        }
    }
}

/// No disk in the browser: every table is solved where it is needed.
#[cfg(target_arch = "wasm32")]
mod disk {
    use rustc_hash::FxHashMap;

    use super::{Cell, Table, TableKey};

    pub(super) fn load() -> FxHashMap<TableKey, Cell> {
        FxHashMap::default()
    }

    pub(super) fn save(_: TableKey, _: &Table) {}
}

/// One attempt at `level` played by the naive rule (`heuristics::naive`)
/// rather than solved: every result it ends on, for the naive climb.
pub(super) fn naive_table(level: u8, fuel: f64) -> Table {
    let cfg = Config::for_level(i64::from(level), Strategy::default()).expect("level >= 1");
    let rule = crate::heuristics::naive(cfg, fuel);
    let (values, cum) = Solver::new(cfg).analyse(Some(&rule)).outcome_table();
    Table { values, cum }
}

/// Memory slots per bar on inheritor `level`'s board.
fn slots_at(level: u8) -> u8 {
    Config::for_level(i64::from(level), Strategy::default()).expect("level >= 1").slots
}

/// The steps from (glory, despair) to a bar of `want` glory and at most
/// `allow` despair: a glory success short or a despair success over, one each.
pub fn gap_to((want, allow): (u8, u8), (glory, despair): Relic) -> u32 {
    u32::from(want.saturating_sub(glory)) + u32::from(despair.saturating_sub(allow))
}

thread_local! {
    static CLOSING: std::cell::RefCell<FxHashMap<(TableKey, (u8, u8), Relic), f64>> = Default::default();
}

/// The steps toward (`want`, `allow`) an attempt played per `key` is
/// expected to close from the memory `from`; a result that closes none counts
/// nothing (it is not kept).
pub fn expected_closing(key: TableKey, aim: (u8, u8), from: Relic) -> f64 {
    if let Some(e) = CLOSING.with(|c| c.borrow().get(&(key, aim, from)).copied()) {
        return e;
    }
    let before = gap_to(aim, from);
    let e = outcomes(key).into_iter().map(|(o, p)| p * f64::from(before.saturating_sub(gap_to(aim, o)))).sum();
    CLOSING.with(|c| c.borrow_mut().insert((key, aim, from), e));
    e
}

/// Every (glory, despair) an attempt played per `key` can end on, with its
/// probability.
pub fn outcomes(key: TableKey) -> Vec<(Relic, f64)> {
    let cell = shared(key);
    let table = cell.get().expect("solved");
    let mut before = 0.0;
    table
        .values
        .iter()
        .zip(&table.cum)
        .map(|(&value, &cum)| {
            let p = cum - before;
            before = cum;
            (value, p)
        })
        .collect()
}

/// A run's own view of the shared tables, so the hot loop does not touch the
/// shared lock once it has seen a table.
#[derive(Default)]
pub struct LocalTables {
    seen: FxHashMap<TableKey, Cell>,
}

impl LocalTables {
    pub fn get(&mut self, key: TableKey) -> &Table {
        self.seen.entry(key).or_insert_with(|| shared(key)).get().expect("solved")
    }
}
