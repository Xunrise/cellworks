# Prototype: STORY-083 — the launcher wears the Netrunner look with switchable theme words

Issue #134, epic EPIC-025. The prototype is a Claude Design canvas, not code:
https://claude.ai/artifact/PY7i4CJwQ4zRXEfGy26XEi, pages v2 (Netrunner palette)
and v3 (full theme words). Page v1 is the STORY-082 layout, already merged into
`test`; this story restyles it and changes its words, not its behaviour.

## Confirmed behavior

- The launcher uses the Cellworks Netrunner design system (v4): near-black
  surfaces on a faint 100 px grid, panel `#0d1117`, ink `#d7e2ea`, dim ink
  `#7d8fa0`, attention yellow `#fcee0a` as the only glowing colour, flow cyan
  `#5ef6ff` for focus and the mode badge, state colours always paired with a
  glyph or a word (● ok green, ○ offline dim, ▲ warn orange).
- Faces: Chakra Petch for labels (upper case, tracked), JetBrains Mono for
  paths, branches, keys and counts.
- Chamfered corners: the card cut top-right and bottom-left (12 px), the CTA
  and ICE card top-left and bottom-right (10 px), chips and controls 8 px.
- Header: "CELLWORKS" with "WORKS" in yellow, the eyebrow "// FORTRESS SELECT",
  "<n> fortresses · <m> nets", a disabled "ADD FORTRESS ⌘N".
- Search row: a deep well with a cyan edge when focused, a cyan ":" prompt,
  placeholder "cd fortress, block or host:/path", the ⌘K key; while searching a
  yellow blinking caret, "<k> matches" in cyan and "Esc clear".
- Chips: "ALL <n>", "LOCAL NET <n>", "● WSL CONNECTED <n>", "○ DEVBOX OFFLINE
  <n>" (offline dim). The picked chip has a dark-yellow fill, yellow text and a
  yellow edge.
- List: group headers as eyebrows with a rule ("RECENT", "LOCAL NET ~/Projects",
  "● WSL NET · CONNECTED", "○ DEVBOX NET · OFFLINE"). Rows in mono: name, path,
  branch or "block". The selected row has a dark-yellow fill, a 2 px yellow bar
  on the left and a yellow name. Matched fragment underlined in yellow.
  Overflow lines read "+ 6 more on the local net".
- Preview pane: eyebrow "FORTRESS", the name in upper case, "git fortress ·
  local net" or "block · devbox net", PATH / BRANCH / FILES, then the CTA.
- CTA: a 72 px yellow block "OPEN FORTRESS / Jack into <name> ⏎" with a
  breathing yellow glow (1.6 s). Disabled for an offline host: an outline block
  "OPEN FORTRESS / Connect the net first". Below it a disabled "SYNC · COMING
  SOON".
- Offline host selected: an orange ICE card "ICE // NET OFFLINE — The devbox net
  does not answer. Its fortresses stay listed but cannot be breached."
- Notice below the card: "ICE // Skipped the devbox net: connection timed out."
  with an orange ▲ and left bar.
- Status bar at the bottom: a cyan mode badge "NAV" with "fortress select", or
  "CMD" with the query while searching, then key hints.
- Motion: the card rises in (18 px, 550 ms, cubic-bezier(.2,.8,.2,1)); list and
  preview follow 90 ms apart; the CTA breathes; the caret blinks. With reduced
  motion requested by the system, none of it animates.
- Theme words are on by default. One setting in `settings.toml` switches them
  off; then the launcher keeps the Netrunner look but uses plain words:
  projects, hosts, "This Mac", folder, "Open project", "Connect the host first",
  "Host offline", no "ICE //" prefix, status text "launcher".
- Behaviour (search, chips, Tab, Esc, Enter, open rule, groups) is exactly
  STORY-082.

## Feedback log

- "actually redesign with our new v4 netrunner design system" → page v2: the v1
  layout redrawn on the Cellworks Netrunner tokens, faces, chamfers and motion.
- "go full theme but allow an option to disable it" → page v3: every launcher
  string in theme words (fortress, net, block, ICE, breach); the plain wording
  stays available behind a setting.
- "Launcher-only toggle" (asked how the switch relates to the skins story
  STORY-036) → the switch is a launcher setting now; STORY-036 replaces it with
  the skin choice later.

## Rejected

- Keeping the plain Cellworks design system look (page v1): superseded by the
  Netrunner redesign.
- Theme words only, with no way to switch them off: the operator wants an opt-out.
- Building the skins module (STORY-036, #58) first or folding it into this
  story: too large; this story ships a launcher-only switch that STORY-036
  absorbs.
- A Clean or 40k word table in this story: those belong to STORY-036.

## Edge cases

- Setting missing or unreadable → theme words on (the default).
- Theme words off → the look stays Netrunner; only the words change.
- One project or one host → singular count words ("1 fortress · 1 net",
  "1 project · 1 host").
- System asks for reduced motion → no rise, no breathing glow, no caret blink.
- Fonts: if the embedded faces fail to load, egui's default faces are used and
  the launcher still works.

## Decisions

- Q: Is v2 the look you want? → A: "Yes, freeze it".
- Q: Plain words or theme words in the UI? → A: "go full theme but allow an
  option to disable it".
- Q: How does the switch relate to STORY-036 skins? → A: "Launcher-only toggle":
  a launcher setting now, replaced by skins later.

## Evidence

- Canvas https://claude.ai/artifact/PY7i4CJwQ4zRXEfGy26XEi: page v2 boards
  `MainNR.dc.html` (default) and `SearchNR.dc.html` (searching ":fac", offline
  host selected); page v3 boards `MainNR3.dc.html` and `SearchNR3.dc.html`
  (same states, full theme words).
- Design system: Cellworks Netrunner, https://claude.ai/artifact/FQBGWeM9B6HhfaeTzwBMrT,
  tokens in `project/ds/CellworksNetrunner/tokens.json` on the canvas.
- Rendered and checked with headless Microsoft Edge at 1280×800; no app code was
  changed on `proto/launcher-netrunner`.
