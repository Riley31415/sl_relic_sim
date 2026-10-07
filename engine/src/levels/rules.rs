//! What a level asks for, and what a plan does about it at one level.

/// A relic's (glory successes, despair successes).
pub type Relic = (u8, u8);

/// A set of relics, one bit per relic index.
pub type RelicSet = u16;

pub fn contains(set: RelicSet, relic: usize) -> bool {
    set & (1 << relic) != 0
}

pub fn set_of(relics: &[usize]) -> RelicSet {
    relics.iter().fold(0, |set, &i| set | (1 << i))
}

pub fn members(set: RelicSet) -> impl Iterator<Item = usize> {
    (0..16).filter(move |&i| contains(set, i))
}

/// A per-relic bar: at least `glory`, at most `despair` (None = any despair).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Bar {
    pub glory: u8,
    pub despair: Option<u8>,
}

impl Bar {
    pub const fn new(glory: u8, despair: Option<u8>) -> Self {
        Bar { glory, despair }
    }

    pub fn met_by(self, (g, d): Relic) -> bool {
        g >= self.glory && self.despair.is_none_or(|limit| d <= limit)
    }
}

/// The condition for being at a level.
#[derive(Clone, Debug, PartialEq)]
pub enum Quest {
    /// each listed relic must clear the bar
    Relics { which: Vec<usize>, bar: Bar },
    /// every relic must clear the bar
    Each(Bar),
    /// summed over every relic: at least `glory`, at most `despair`
    Total { glory: u32, despair: u32 },
}

impl Quest {
    pub fn satisfied(&self, states: &[Relic]) -> bool {
        match self {
            Quest::Relics { which, bar } => which.iter().all(|&i| bar.met_by(states[i])),
            Quest::Each(bar) => states.iter().all(|&s| bar.met_by(s)),
            Quest::Total { .. } => self.gap(states) == 0,
        }
    }

    /// For a total: slots still missing - glory short plus despair over.
    pub fn gap(&self, states: &[Relic]) -> u32 {
        let Quest::Total { glory, despair } = *self else { return 0 };
        let (g, d) = totals(states);
        glory.saturating_sub(g) + d.saturating_sub(despair)
    }

    /// The gap once relic `i` is `new` instead.
    pub fn gap_after(&self, states: &[Relic], i: usize, new: Relic) -> u32 {
        let Quest::Total { glory, despair } = *self else { return 0 };
        let (g, d) = totals(states);
        let (old_g, old_d) = states[i];
        let g = g + u32::from(new.0) - u32::from(old_g);
        let d = d + u32::from(new.1) - u32::from(old_d);
        glory.saturating_sub(g) + d.saturating_sub(despair)
    }

    pub fn is_total(&self) -> bool {
        matches!(self, Quest::Total { .. })
    }
}

pub fn totals(states: &[Relic]) -> (u32, u32) {
    states.iter().fold((0, 0), |(g, d), &(sg, sd)| (g + u32::from(sg), d + u32::from(sd)))
}

/// Everything a plan decides for one level, flattened for the simulation.
#[derive(Clone, Debug, PartialEq)]
pub struct LevelRules {
    /// the bar relics are rolled toward on "each" and "total" levels
    pub base: Bar,
    /// plan a total level as single-step repairs instead of `base`
    pub surgical: bool,
    /// when nothing the quest needs is affordable, attempt a spare relic for
    /// the pity rather than summon - played and kept as a quest roll would be
    pub filler: bool,
    /// ...but only when the spare relics on hand can fill the pity bar in at
    /// most this many attempts (None: whenever nothing the quest needs is
    /// affordable)
    pub filler_within: Option<u32>,
    /// banked relics: out of total work and filler
    pub locked: RelicSet,
    /// every attempt closes the gap to the totals ahead (the most glory and
    /// the least despair any totals level up to the target asks) rather than
    /// this level's own (`Game::horizon_for`)
    pub ahead: bool,
    /// a relic short of its bar that the keep rule ranks at least as high as
    /// the bar (a trade it took) counts as having met it: once every relic
    /// has, the bars go further
    pub trades: bool,
    /// relics the quest work goes to first, and whether the others must wait
    pub prefer: Option<(RelicSet, bool)>,
}
