# Relic advisor

Reads a screenshot of the Hero's Legacy grid (all twelve relics) and says
what the look-ahead plan does next: which relic to attempt and how to play
it, which results to keep, or to summon. The decisions are the Monte Carlo's
own (`Game::advise` in the main crate), so following the advice plays the plan
LEVELS.md costs out.

## Setup

```
cd advisor
cargo build --release
cargo test --release
```

## Use

```
target/release/relic-advisor shot.png                 # toward level 20 + 46% crit amp
target/release/relic-advisor shot.png --target 20+48  # another look-ahead target (2-20, 20+43/46/48/50)
target/release/relic-advisor shot.png --level 19      # if the level can't be read
target/release/relic-advisor --level 18 --board 7/1/207 7/1/115 ...   # typed in, screen order
```

It prints the board as read, the level, and one of:

- **ATTEMPT** a relic, with how to play the attempt (and the autoplayer setting
  that plays it that way), which results to take with *Replace with New
  Effect*, and the odds
- **ATTEMPT (filler)**: nothing the quest needs has 10 on hand, so a spare
  relic is rolled for the pity instead of summoning
- **SUMMON**
- **LEVEL UP**: the board already meets the next level's requirement
- at level 20: **CONVERT** the other relics into Demon Eyes, then **ATTEMPT**
  the Demon Eye for the tier, then **DONE**

The level comes from the requirement at the bottom of the screen (the next
level's), so the screen shows level N while it asks for level N+1's totals.

## Reading the screen

Everything is placed from the grid of gem icons, so the game can be anywhere
in the screenshot at any size. Digits are read by template, with the templates
learned from `tests/fixtures/grid-l18.png`. Every relic's numbers are checked
against the screen's Total box. A mismatch is flagged under the table.

## Limits

- One screenshot is all the templates come from. The tests read every number
  with its own glyphs left out, at 0.9x to 3x, through JPEG, and inside a
  bigger browser screenshot. Despair and glory counts never seen in their
  colour (a red "x 0") rely on the white stock digits.
- If a number misreads, `--dump` shows the ink each number was read from, and
  `--debug out.png` outlines every box read. Typing the board with `--board`
  always works.
- A screenshot shows one moment. Nothing is known of the pity meter (it only
  changes when a level comes, not what is rolled) or of how far earlier
  attempts at this level pushed the plan's bars.
