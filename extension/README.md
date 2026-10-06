# Relic Autoplayer (Chrome extension)

Plays relic inheritance attempts in the cloud-phone tab with the exact
solver from this repo, compiled to WebAssembly. It reads the game from tab
screenshots and clicks through the Chrome DevTools Protocol.

## Setup

```
cd extension
node build.mjs          # cargo build --target wasm32-unknown-unknown, copies chrome/relic.wasm
node test/run.mjs       # unit tests (screen reader, rules, run loop)
node test/browser.mjs   # the solver + screen reader inside headless Chrome
```

Then go to `chrome://extensions`, turn on Developer mode, choose **Load
unpacked**, and pick `extension/chrome`. The toolbar button opens the side
panel.

## Modes

**Advisor** (the default) plays the repo's look-ahead plan (LEVELS.md)
toward a goal: level 19 or 20, or 20 and then Demon Eye at 43-50%. Start it
on the main page or a relic's Hero's Legacy.

1. The **levels view** gives the inheritor level and the pity bar. If the
   bar is full it stops: level up in the game, then Start again.
2. The **counts view** (the circular-arrows icon) gives every relic's glory
   and despair, which must add up to the Total line, and its stock from
   the badge under its tile.
3. Stock that won't read off a badge is read off that relic's **Hero's
   Legacy** instead ("127/10"; the "/10" must read too). Whenever a relic's
   Hero's Legacy is opened, its stock there is checked against the badge
   and its memory against the counts view. If a badge was misread, it stops
   trusting badges and re-reads every stock off Hero's Legacy. If a stock
   won't read anywhere, the panel asks you for it.
4. The plan's advice decides what happens next:
   - **level up** (the next level's requirement is met), **summon**
     (nothing it needs is affordable), **convert** (at 20, short of the
     tier) or **done**: it stops and says so.
   - **roll a relic**: it opens that relic and plays its attempts the way
     the plan says (all-or-nothing for a bar, a weighted score, or a mark).
     Keep or Replace follows the plan's own keep rule, for example keeping
     any roll that closes the gap to a level's totals.
5. While the plan, re-run on the updated board, keeps picking the same
   relic, it keeps rolling it. Otherwise it goes back (the red return
   button) and starts over from step 1. Each attempt adds pity, and once
   the bar could be nearly full it goes back to read it.

Started part-way through an attempt, it finishes that attempt first. If the
advisor started it (and the run was stopped), it uses the same targeting and
keep rule, saved when it clicked Inheritance. Otherwise it uses the
**Inheritor level** and **Play for** settings. Then it goes on from the main
page.

**One relic** plays only the relic whose Hero's Legacy is showing, at the
level and with the "Play for" objective set in the panel. It stops when a
result beats the current memory.

## Use

1. Open the game tab, go to Memory Inheritance, and open the side panel.
2. Pick the **mode**. In One relic mode, set the **inheritor level**: it sets
   the slots, spirit power, head starts and rate bonuses. If an attempt
   doesn't open the way that level should (free glory and despair slots),
   the run stops and names the levels that fit. The advisor reads the level
   itself.
3. Pick what to play for. **Most amplification** gives +5% per glory success
   and -2% per despair success. A **target** (>= G glory, <= D despair) or a
   **mark** is all or nothing. Wipe penalty trades amplification for fewer
   wipes (see STRATEGY.md).
4. Press **Read screen** first. The view marks every slot it read and the
   button it would press, without clicking anything.
5. Press **Start**. Chrome shows a "started debugging this browser" bar while
   it runs. Pressing Cancel on that bar stops the run.

The loop is Hero's Legacy, then **Inheritance**, then attempts until both
bars are full, then **Complete Inheritance**, then **Keep** or **Replace**,
then back to Hero's Legacy. Clicks are spaced by the **Click delay**
setting, give or take a random 0.1 s (1.5 s gives 1.4-1.6 s).

It stops when:

- the screen is not one of those it knows, settled or after a click
  (popups, the abandon confirmation, loading, anything else)
- a click does something other than what that button does
- a result beats the current memory: it presses Replace, checks Hero's
  Legacy shows the new memory, then stops
- **Max attempts** attempts have finished, each one kept with Keep
- the current memory already meets a target or mark
- the attempt wipes, or you press Stop

An attempt nothing useful can come of is abandoned at once. Every count of
successes in the slots left is still possible, so if none of those results
would be kept (the plan's keep rule, or in One relic mode a target hit or
more amplification than the current memory), it clicks Abandon Inheritance
and Confirm instead of playing on. Once a target is out of reach, the solver
plays the rest of the attempt for the result nearest to it still possible
(7/1 gone with 2 despair successes: 7/2, which the keep rule then keeps over
a 7/3), then for amplification; once it is certain, for amplification.

A wiped attempt (no spirit power or mental strength left, the bars
unfinished, no Attempt buttons) is abandoned: it clicks **Abandon
Inheritance** and then **Confirm**, checks the applied memory is unchanged,
and counts it as an attempt. Those two cases are the only times it touches
Abandon: clicks there are refused otherwise, and the confirmation is only
looked for right after its own Abandon click.

On the results screen it reads both cards ("Glory x 8", "Despair x 3"). A
card counts only if its Final Boost Value equals 5 x glory - 2 x despair,
so a misread digit makes the card unreadable instead of wrong. The cards
are checked against what the run saw itself, so it can decide even when
it started mid-attempt or on the results screen. If a card won't read and
the run didn't see that memory itself, it stops and leaves the choice to
you.

## The rate

The 80/65/50/35/20% rate is read off the Attempt buttons, from the shape of
the digits. The second digit has a hole in "0" but not in "5", the first
digit has holes in 8 and 6 but not in 5, 3 or 2, and 5 and 2 differ in
which side the upper stroke is on. Every visible button is read, and they
must agree. The reading is what the solver plays by. If the buttons can't
be read (text under ~0.8x), the rate is tracked through the ladder rules.
Only if neither works does it ask you.

A screen counts only after it has held still for 0.9 s, because the spirit
bar animates after a training and reading it too early once made a success
look like a failure.

## Limits

- Numbers are read by matching each digit against examples cut from the
  captures (`node test/make-glyphs.mjs` rebuilds them from
  `test/images`). A digit reads only when one match is clearly best, and
  numbers are checked against each other wherever the game shows a check.
- Every screen is measured on real captures in `test/images`. Boards
  and their rates are tested from 0.8x to 1.5x with noise, the abandon
  popup is tested never to pass for Compare, and the Mental Training
  button can be missing (0 mental strength). All five rates have been read
  in a live run. If a screen is not recognised the run stops safely: use
  **Save frame** on it and add the image to `test/images`.
- Automating a game may be against its terms of service.
