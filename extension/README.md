# Relic Autoplayer (Chrome extension)

Plays relic inheritance attempts in the cloud-phone tab with the exact
solver from this repo, compiled to WebAssembly. It reads the game from tab
screenshots (Chrome's tab capture: up to 2 a second, and no redraw, so the
phone's video does not flicker) and clicks through the Chrome DevTools
Protocol. Tab capture needs the extension's access to the site, which Chrome
asks for when the extension is loaded. Keep the game tab the one showing in
its window: otherwise it falls back to the debugger's screenshots (and says
so), which work on a hidden tab but make the page flicker.

## Setup

```
cd extension
node build.mjs          # the advisor's build (cargo, WebAssembly), then its library into chrome/
node test/run.mjs       # unit tests: the run loop and the Smart Leveler
```

The solver, the advice, the rules and the screen reader are the advisor's
(`../advisor/js`, with its own tests, see [advisor/README.md](../advisor/README.md)).
`build.mjs` copies them into `chrome/lib/` and `chrome/relic.wasm`, because an
unpacked extension loads nothing from outside its folder. Edit them in
`advisor/js`, not in the copies. `node test/run.mjs` copies them first too.

Then go to `chrome://extensions`, turn on Developer mode, choose **Load
unpacked**, and pick `extension/chrome`. The toolbar button opens the side
panel.

## Modes

**Advisor** (the default) plays the repo's Look Ahead plan (COST.md)
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
     the plan says. A relic the level names a bar for: the best chance of
     the bar, then the goal's totals. On a totals level: closing the gap to
     the level's totals, or, where the plan says so, to the totals ahead
     (the most glory and the least despair any level up to the goal asks),
     then more glory and less despair weighed as those totals still need
     them, so 9/0 beats a 9/1 that clears it. Keep or Replace ranks the
     result the same way: its bar met, then the board nearer those totals,
     then more glory less despair. When every relic meets its bar but the
     requirement is still short, the plan pushes the bars further (where the
     plan says so, a relic traded off its bar for a result the keep rule
     ranks as high counts as meeting it) - worked out from the board each
     time, like everything else it decides.
5. While the plan, re-run on the updated board, keeps picking the same
   relic, it keeps rolling it. Otherwise it goes back (the red return
   button) and starts over from step 1. Each attempt adds pity, and once
   the bar could be nearly full it goes back to read it.

Started part-way through an attempt, it finishes that attempt first. If the
advisor started it (and the run was stopped), it uses the same targeting and
keep rule, saved when it clicked Inheritance. Otherwise it uses the
**Inheritor level** and **Strategy** settings. Then it goes on from the main
page.

**Single relic** plays only the relic whose Hero's Legacy is showing, at the
level and with the **Strategy** set in the panel. It stops when a
result beats the current memory.

## Use

1. Open the game tab, go to Memory Inheritance, and open the side panel.
2. Pick the **mode**. In Single relic mode, set the **inheritor level**: it sets
   the slots, spirit power, head starts and rate bonuses. The board checks
   it. The game doesn't show the level during an attempt, so the run works
   out which levels the board fits: its slots, a spirit bar that's a whole
   number of that level's spirit power, and its free first slots. An
   attempt nothing has been spent on also has to be that level's opening.
   - The level set is kept if the board fits it.
   - Otherwise, if the board fits one level, or several that play the same
     from here (18 and 19 do, part-way), the run switches to it, says so,
     and updates the setting.
   - Otherwise it asks which level, with a button for each. Part-way
     through, levels 16-19 all show 9 slots, and only the bonuses tell them
     apart.

   **Read screen** takes the highest level that fits, without asking. The
   advisor reads the level off the main page.
3. Pick the **Strategy**:
   - **Maximize amplification above a target** (+5% per glory success, -2%
     per despair success), with a **Minimum Useful Amplification**: a result is
     worth nothing below the goal, something at it and more above it, so it
     plays first for the best chance of the goal or more, then for the most
     amplification. If the goal slips out of reach part-way, it plays for
     the highest amplification still possible (and says so in the log),
     never for less than beats the current memory. Left blank, the goal is
     0: raised to beat the memory, like any goal under it, or with no memory
     (or one at +0%) the most amplification, expected. A result is kept
     only if it beats the memory. The advisor farms
     Demon Eye this way,
     with the goal's tier as the Minimum Useful Amplification.
   - **Maximize glory, minimize despair above a target**, with a **Glory
     Target** and a **Despair Target**: plays for the best chance of both at
     once (that much glory or more, that much despair or less), then for
     more glory and less despair; a result is kept if it hits the target
     where the memory did not, or as much and with more glory less despair.
   - **Maximize glory, minimize despair until a limit**, with a **Max
     useful Glory** and a **Min useful Despair**: every glory gained up to
     the max and every despair shed down to the min counts, and it plays for
     the most of those steps, then for more glory, less despair. The advisor
     plays a totals level this way, the limit being what closes the totals
     gap for that relic.

   In Advisor mode these show how an attempt in progress is finished when
   the advisor has no record of it, and the advisor sets them to whatever
   it last played, so a run stopped and started again picks up the same
   way.
4. Press **Read screen** first. The view marks every slot it read and the
   button it would press, without clicking anything.
5. Press **Start**. Chrome shows a "started debugging this browser" bar while
   it runs. Pressing Cancel on that bar stops the run.

The loop is Hero's Legacy, then **Inheritance**, then attempts until both
bars are full, then **Complete Inheritance**, then **Keep** or **Replace**,
then back to Hero's Legacy. Clicks are spaced by the **Click delay**
setting, give or take a random 0.1 s (1.5 s gives 1.4-1.6 s).

Each click is given 10x the click delay (15 s at 1.5 s) to show its effect.
If the game is still on the screen it clicked, settled and unchanged, the
click did not land and is made again, up to 3 times in all. A screen it does
not recognise (a stream hiccup, a loading overlay) is watched for as long,
without clicking, and the run carries on if a known screen comes back.

It stops when:

- the screen is not one of those it knows and does not clear (popups, the
  abandon confirmation, anything else). The frame is saved to Downloads.
- a click leads somewhere other than where that button goes, or 3 clicks
  on it in a row do nothing
- a click does something other than what that button does
- a result beats the current memory: it presses Replace, checks Hero's
  Legacy shows the new memory, then stops
- the current memory already meets a target or mark
- the attempt wipes, or you press Stop

An attempt nothing useful can come of is abandoned at once. Every count of
successes in the slots left is still possible, so if none of those results
would be kept (the plan's keep rule, or in Single relic mode a narrower gap to
the target or more amplification than the current memory), it clicks Abandon
Inheritance and Confirm instead of playing on. Once nothing more of the gap
can be closed, the solver plays the rest of the attempt for amplification.

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
  captures (`node advisor/js/test/make-glyphs.mjs` rebuilds them from
  `advisor/js/test/images`). A digit reads only when one match is clearly best, and
  numbers are checked against each other wherever the game shows a check.
- Every screen is measured on real captures in `advisor/js/test/images`. Boards
  and their rates are tested from 0.8x to 1.5x with noise, the abandon
  popup is tested never to pass for Compare, and the Mental Training
  button can be missing (0 mental strength). All five rates have been read
  in a live run. If a screen is not recognised the run stops safely: use
  **Save frame** on it and add the image to `advisor/js/test/images`.
- Automating a game may be against its terms of service.
