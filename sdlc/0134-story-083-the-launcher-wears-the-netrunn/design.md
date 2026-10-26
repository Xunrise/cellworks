---
stage: design
issue: 134
status: frozen
author: claude (freeze)
date: 2026-10-26
---

# Design: STORY-083 — the launcher wears the Netrunner look with switchable theme words

The contract is `issue.md` in this directory: its acceptance scenarios are what
the change must do. `prototype.md` beside it records the confirmed look
(Claude Design canvas, pages v2 and v3) and every answered question; its
Rejected items are not built. Behaviour (search, chips, Tab, Esc, Enter, open
rule, groups) is STORY-082's and does not change; this story changes the look
and the words of the launcher only.

## Open questions

> ASSUMED (Q-1): what does the plain-words eyebrow say? — default: nothing,
> the eyebrow line (header and preview pane) is only drawn in theme words,
> because STORY-082 never had one and requirement 8 names no plain text for
> it.

> ASSUMED (Q-2): what does the plain offline body say? — default: "The <host>
> host is not reachable right now.", because requirement 8 is silent on this
> body text and this is the plainest paraphrase of the themed one.

## Requirements

1. The launcher is drawn in the Cellworks Netrunner tokens: background
   `#07090c` with a faint 100 px grid (`#0e141b`), panel `#0d1117`, deep well
   `#040507`, lines `#1e2731` / `#34506a`, ink `#d7e2ea`, dim ink `#7d8fa0`,
   attention yellow `#fcee0a` (soft `#3a3608`), flow cyan `#5ef6ff`, ok
   `#3ddc84`, warn `#ff9f1c`. Only yellow glows. Every state colour sits beside
   a glyph or a word. The rest of the app keeps its current visuals.
2. Faces: Chakra Petch for labels (upper case, tracked), JetBrains Mono for
   paths, branches, keys and counts. Both are embedded (SIL OFL 1.1, licence
   files beside them) as named families; the default egui families are not
   replaced. If a family is not bound, the view uses egui's default faces.
3. Chamfered corners: card 12 px top-right and bottom-left; CTA and ICE card
   10 px top-left and bottom-right; chips and controls 8 px.
4. Layout and states as on canvas page v3 (`MainNR3.dc.html`,
   `SearchNR3.dc.html`): header, search well (cyan edge when focused, cyan ":"
   prompt, yellow caret), chips (picked: soft-yellow fill, yellow text and
   edge), grouped list (selected row: soft-yellow fill, 2 px yellow left bar,
   yellow name; matched fragment underlined yellow), preview with a 72 px CTA
   (yellow block with breathing glow, or outline when disabled), an orange ICE
   card when the selected project's host is offline, notices below the card
   with an orange ▲ and left bar, and a status bar with a cyan mode badge
   ("NAV" + status text, or "CMD" + query while searching) followed by
   STORY-082's key hints.
5. Motion: the card rises in (18 px, 550 ms, ease-out approximating
   cubic-bezier(.2,.8,.2,1)), list +90 ms, preview +180 ms; the enabled CTA
   glow breathes over 1.6 s; the caret blinks. When the system asks for reduced
   motion, nothing animates and no repaint is scheduled for motion.
6. Theme words are on by default. `[launcher] theme_words = false` in
   `settings.toml` switches them off. Only the boolean `false` does; a missing
   key, another type, or a missing or unreadable file leaves them on. The value
   is read with the scan, so a change applies on the next start.
7. Themed wording (displayed upper case where the canvas shows it): eyebrow
   "// FORTRESS SELECT", "<n> fortresses · <m> nets", "ADD FORTRESS ⌘N",
   placeholder "cd fortress, block or host:/path", chips "ALL", "LOCAL NET",
   "<HOST> CONNECTED|CHECKING|OFFLINE", groups "RECENT", "LOCAL NET",
   "<HOST> NET · <STATE>", branch-less "block", preview eyebrow "FORTRESS",
   kind/location "git fortress · local net" or "block · <host> net", CTA
   "OPEN FORTRESS" with "Jack into <name>", offline CTA subtitle "Connect the
   net first", "SYNC · COMING SOON", ICE card "ICE // NET OFFLINE" with "The
   <host> net does not answer. Its fortresses stay listed but cannot be
   breached.", notice prefix "ICE // " and "Skipped the <host> net: <reason>",
   status text "fortress select".
8. Plain wording is STORY-082's text unchanged, except: the open action reads
   "Open project", its offline subtitle "Connect the host first", the ICE card
   heading becomes "Host offline", notices have no "ICE //" prefix, status
   text "launcher".
9. Counts use the singular for one ("1 fortress · 1 net", "1 project ·
   1 host"). The second number counts remote hosts, as STORY-082 does.
10. The Gherkin's quoted strings are matched case-insensitively against the
    displayed text, because the themed table displays several of them upper
    case (for example "Local net 2" is drawn "LOCAL NET 2").

## Decisions

1. Words live in a new `crates/workspace/src/words.rs`: `pub(crate) struct
   Words` of `&'static str` fields and small `fn`s, with consts `THEMED` and
   `PLAIN`; the user strings now in `launcher.rs` (`THIS_MAC`, "All",
   "folder", `open_label`, `HostState::word`) move there. Rejected: a lookup by
   key (HashMap or enum index), which the compiler cannot check.
2. The setting is `[launcher] theme_words`. Rejected: a top-level key; the
   table scopes it to the launcher, which is what STORY-036 replaces.
3. `scan` sends `WorkspaceEvent::ThemeWordsTurnedOff` only when the setting is
   off; `Launcher::apply` switches to `PLAIN`, `Universe` ignores it. Rejected:
   an event on every scan, which breaks the tests that expect exactly one scan
   event.
4. Only strings the v3 canvas shows in theme words differ between the tables;
   scanning, failed, empty, no-match texts, key hints, "<k> matches",
   "CELLWORKS", NAV/CMD and the PATH/BRANCH/FILES captions are one string.
5. Fonts are named families (`chakra-petch`, `jetbrains-mono`) installed by
   `ui::install_fonts`, called from `src/app.rs` after `set_theme`. Rejected:
   replacing the default Proportional/Monospace families, which restyles the
   whole app.
6. Glow is drawn as three or four expanded chamfer outlines with falling alpha
   (egui has no blur), scaled by the breath factor.
7. Reduced motion is read once at start-up in `src/app.rs` by child process:
   `defaults read com.apple.universalaccess reduceMotion` on macOS ("1" =
   reduced), `gsettings get org.gnome.desktop.interface enable-animations` on
   Linux ("false" = reduced); failure means animate. When reduced,
   `animation_time = 0.0` and `text_cursor.blink = false` are set on all
   styles, and the view treats `animation_time == 0.0` as reduced. Rejected:
   the objc2-app-kit dependency (ADR-0003 prefers child processes).
8. Motion repaints use `request_repaint_after_secs(1.0 / 30.0)`, never an
   immediate repaint, so `egui_kittest`'s `run()` settles.
9. The canvas's overflow lines ("+ 6 more on the local net") are not built:
   STORY-082 has no overflow cap and behaviour does not change.
10. The STORY-078 and STORY-082 acceptance tests run with theme words off; only
    their open-action assertions move to "Open project" and "Connect the host
    first". Rejected: rewriting them to themed strings.

## Order of work

1. [x] Words and switch. Add `crates/workspace/src/words.rs` (`Words`, `THEMED`,
   `PLAIN`, singular/plural counts, host-dependent strings) and declare it in
   `lib.rs`; `settings.rs` consts `LAUNCHER`, `THEME_WORDS` and
   `theme_words_off`; `event.rs` `WorkspaceEvent::ThemeWordsTurnedOff`; `scan`
   in `lib.rs` sends it; `universe.rs` ignores it; `launcher.rs` holds
   `words: &'static Words` (default `&THEMED`), switches on the event, exposes
   `words()`, and reads its strings from the table. Unit tests first.
   Files: `crates/workspace/src/{words,settings,event,lib,universe,launcher}.rs`.
2. [x] The look. Add `crates/workspace/assets/fonts/ChakraPetch-SemiBold.ttf`,
   `JetBrainsMono-Regular.ttf` and their `OFL.txt`; in `ui.rs` the Netrunner
   colour consts, `chamfer`, grid, `install_fonts`, family fallback, and the
   restyled header, search row, chips, list, rows, preview with CTA and ICE
   card, notices and status bar, all mode-dependent strings from
   `launcher.words()`; the remote-on-connected-host hover keys on the project,
   not on a label string. Files: `crates/workspace/assets/fonts/*`,
   `crates/workspace/src/ui.rs`.
3. [x] Motion and start-up. In `ui.rs` pure `rise` and `breath`, first-shown time
   in egui temp memory, `with_visual_transform` / `multiply_opacity`, scheduled
   repaints only while moving and not reduced. In `src/app.rs` call
   `install_fonts`, add `reduced_motion_requested()` and the pure
   `reduces_motion(output, macos)`, and apply the reduced style.
   Files: `crates/workspace/src/ui.rs`, `src/app.rs`.
4. [x] Acceptance tests. Add
   `tests/story_083_the_launcher_wears_the_netrunner_look_with_switchable_theme_words.rs`
   (one test per scenario) and write `[launcher]\ntheme_words = false` into the
   STORY-078 and STORY-082 fixtures, moving their open-action assertions.
   Files: `tests/`.

## Risks

- The font files are fetched from upstream (Google Fonts, JetBrains releases)
  during the build; they add about 350 KB to the binary.
- The breathing CTA and caret repaint the launcher about 30 times a second
  while it is open; the cost is not measured.
- Reduced motion and the theme-words setting are read once per start.
- Upper-cased names change accessible labels; themed tests assert the
  displayed case.
- A remote project reads "block · <host> net" in theme words although its kind
  is unknown, as the canvas shows.
- Out of scope: skins and Clean/40k word tables (STORY-036, #58), restyling
  anything outside the launcher, any behaviour change, overflow lines.

## Proof

Commands (all must pass, as in `CLAUDE.md` § Verifying your work):

    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    SHELLCHECK_OPTS=--exclude=SC2016 actionlint

Unit tests:
- `settings.rs`: `[launcher] theme_words = false` sets `theme_words_off`;
  `true`, a string, a missing table and a missing file leave it unset.
- `lib.rs`: `scan` reports `ThemeWordsTurnedOff` once when off, never
  otherwise; the existing one-event tests stay green.
- `words.rs`: counts singular at 1 and plural otherwise, in both tables.
- `launcher.rs`: the default launcher names the local location "LOCAL NET" and
  "This Mac" after `ThemeWordsTurnedOff`; `can_open` unchanged.
- `ui.rs`: `chamfer` returns six points inside the rect with the named corners
  cut; `rise` is (0, 1.0) when reduced and after 550 ms + delay, 18 px at 0;
  `breath` is constant when reduced and periodic over 1.6 s otherwise.
- `src/app.rs`: `reduces_motion` reads "1" (macOS) and "false" (Linux) as
  reduced, anything else as not.

Acceptance tests, one per scenario of issue #134, headless `egui_kittest`
through `cellworks::draw`, harness width 1280, STORY-082's fixture (cellworks
and The-Factory local, `wsl` connected with factory-evals, `devbox` offline
with surface-kit):
- Theme words are shown by default: header "4 fortresses · 2 nets", chips "All
  4", "Local net 2", "wsl connected 1", "devbox offline 1", open action "Open
  fortress" (case-insensitive).
- An offline host is reported as ICE: selecting surface-kit shows "ICE // NET
  OFFLINE" and the disabled action with "Connect the net first".
- Theme words can be switched off: "4 projects · 2 hosts", chips "All 4",
  "This Mac 2", …, "Open project".
- Plain words for an offline host: "Host offline", disabled action with
  "Connect the host first".
- Switching words does not change behaviour: with theme words off, "fac"
  lists the same three projects; Enter on cellworks opens it.

Also: a reduced-motion harness settles with no repaint scheduled; without
`install_fonts` the launcher draws with default faces and does not panic. The
STORY-078 and STORY-082 acceptance tests pass with theme words off.

## Docs impact

Written at freeze in the commit `docs: ingest STORY-083 prototype decisions`;
the build changes no other doc.

- **Business decisions** — None.
- **Glossary** — `docs/domain/contexts/Workspace/glossary.md`: new term
  "Theme words".
- **Guidelines** — `docs/services/cellworks/guidelines/naming.md`: the
  theme-words rule covers the Netrunner words;
  `docs/services/cellworks/guidelines/architecture.md`: `defaults` and
  `gsettings` as read-only external tools for reduced motion.
- **arc42** — None.
- **ADR needed** — No.
