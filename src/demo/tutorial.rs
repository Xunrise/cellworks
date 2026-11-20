//! PROTOTYPE (stakeholder demo): the interactive tutorial. Each chapter is an
//! intro card, guided steps the presenter clicks through on the real widgets,
//! and a free-play bar. Widgets report their place with `demo::mark`; the
//! overlay dims everything but the target and rings it.

use std::time::Duration;

use egui::{Align2, Color32, Pos2, Rect, Stroke, pos2, vec2};

use super::{CORE, Tour};
use crate::app::CellworksApp;

use cellworks_workspace::theme::{
    CYAN, DIM, INK, ON_YELLOW, YELLOW, chamfer, eyebrow, font, panel, tracked,
};

/// What a step asks the presenter to do.
#[derive(Clone, Copy)]
pub enum Action {
    /// A primary click on the target.
    Click,
    /// A key press; the target, if any, is only shown.
    Key(egui::Key, &'static str),
    /// Scripted beats play; the step ends when they are done.
    Watch,
}

pub struct Guided {
    /// A mark id (`demo::mark`), `tower:<path>`, `building:<path>`, or empty.
    /// A trailing `*` matches any mark with that prefix.
    pub target: &'static str,
    /// At most 12 words: what this click does.
    pub callout: &'static str,
    /// Shown after the action: what you see now and why it is valuable.
    pub explain: &'static str,
    pub action: Action,
    /// Drive lines run before the highlight shows.
    pub prep: &'static [&'static str],
    /// What N runs in place of the real click or key.
    pub emulate: &'static [&'static str],
    /// Drive lines run after the action (Watch: the beats themselves).
    pub then: &'static [&'static str],
    /// Ends the step when the presenter got there another way (a key, a double click).
    pub done_if: Option<fn(&CellworksApp) -> bool>,
    /// The target button's own shortcut, which also ends the step.
    pub hotkey: Option<egui::Key>,
}

pub struct Chapter {
    pub title: &'static str,
    pub why: &'static str,
    pub steps: &'static [Guided],
}

const fn click(target: &'static str, callout: &'static str, explain: &'static str) -> Guided {
    Guided {
        target,
        callout,
        explain,
        action: Action::Click,
        prep: &[],
        emulate: &[],
        then: &[],
        done_if: None,
        hotkey: None,
    }
}

const fn key(
    key: egui::Key,
    label: &'static str,
    callout: &'static str,
    explain: &'static str,
) -> Guided {
    Guided {
        action: Action::Key(key, label),
        ..click("", callout, explain)
    }
}

const fn watch(
    target: &'static str,
    callout: &'static str,
    explain: &'static str,
    then: &'static [&'static str],
) -> Guided {
    Guided {
        action: Action::Watch,
        then,
        ..click(target, callout, explain)
    }
}

pub const CHAPTERS: [Chapter; 12] = [
    Chapter {
        title: "What Cellworks is and where it is going",
        why: "One map for your projects, code, tickets, agents and servers.",
        steps: &[],
    },
    Chapter {
        title: "Pick a project",
        why: "The launcher lists every project on this machine and the hosts you connected, so you start where the work is.",
        steps: &[
            Guided {
                emulate: &["select {ROOT}/projects/shopfront"],
                done_if: Some(project_open),
                ..click(
                    "launcher.row.shopfront",
                    "Select shopfront (j/k moves, / searches)",
                    "The preview shows what shopfront holds: its code, docs, open tickets and the outside systems it touches.",
                )
            },
            Guided {
                emulate: &["open {ROOT}/projects/shopfront"],
                done_if: Some(project_open),
                ..click(
                    "launcher.open",
                    "Open shopfront as a city",
                    "Every folder is a district and every file a tower: the whole project in one view.",
                )
            },
        ],
    },
    Chapter {
        title: "Read the city",
        why: "Code, docs, infrastructure and servers become one map you can walk, instead of five tabs you keep in your head.",
        steps: &[
            Guided {
                prep: &["wait 2500"],
                ..watch(
                    "",
                    "Watch: an agent reads the code, tower by tower",
                    "Agents show up on the map as figures walking between the files they read.",
                    &[
                        "agent explorer src/catalog/search.ts",
                        "wait 3500",
                        "agent explorer src/cart/totals.ts",
                        "wait 1500",
                    ],
                )
            },
            Guided {
                prep: &["fit", "wait 900"],
                emulate: &["jump docs"],
                ..click(
                    "building:docs",
                    "Click the docs Library",
                    "Documents stand as Libraries; Terraform becomes Plants, YAML Switchboards and the Dockerfile a Depot.",
                )
            },
            Guided {
                prep: &["fit", "wait 900"],
                ..click(
                    "server",
                    "Click the staging server",
                    "Servers live on the map too: staging shows the version it runs, v1.3.0.",
                )
            },
            Guided {
                prep: &["fit", "wait 900"],
                emulate: &["jump src/cart/totals.ts"],
                ..click(
                    "tower:src/cart/totals.ts",
                    "Click the totals tower to read its floors",
                    "Each floor is a line of code, and the cellar below holds its tests.",
                )
            },
            Guided {
                emulate: &["lanes imports"],
                ..click(
                    "lanes.imports",
                    "Show the import lanes along the ground",
                    "Lines on the ground show which files import which, so dependencies are visible at a glance.",
                )
            },
            Guided {
                emulate: &["lanes calls"],
                ..click(
                    "lanes.calls",
                    "Show the call lanes through the air",
                    "Arcs in the air are calls, from the line that calls to the function it reaches.",
                )
            },
            Guided {
                emulate: &["tree"],
                ..key(
                    egui::Key::E,
                    "E",
                    "Press E for the file tree",
                    "The tree and the city stay in step: pick a file on one and it lights up on the other.",
                )
            },
            Guided {
                emulate: &["lens-next"],
                ..key(
                    egui::Key::L,
                    "L",
                    "Press L to switch the lens",
                    "A lens is another way to read the same project: folders, imports, cells.",
                )
            },
        ],
    },
    Chapter {
        title: "Reshape the city for your question",
        why: "The city answers a general question. A lens answers yours, per project and per person, and Claude writes it for you.",
        steps: &[
            Guided {
                prep: &["lens-reset"],
                emulate: &["lens-panel on"],
                ..click(
                    "lens.toggle",
                    "Open the lens panel",
                    "Lenses live in a panel: ask a question and Claude writes the lens.",
                )
            },
            Guided {
                prep: &["lens-question Is the backend hexagonal?"],
                then: &["lens-ask \"Is the backend hexagonal?\" hexagonal"],
                ..click(
                    "lens.create",
                    "Click Create with Claude to ask the question",
                    "Claude read the facts and wrote a lens: domain, ports and adapters as blocks, one violation flagged.",
                )
            },
            Guided {
                prep: &["lens-question Show me how the frontend talks to the backend"],
                then: &["lens-ask \"Show me how the frontend talks to the backend\" front-back"],
                ..click(
                    "lens.create",
                    "Ask how the frontend talks to the backend",
                    "Same project, new question: the UI's HTTP calls now run to the API routes.",
                )
            },
            Guided {
                prep: &[
                    "lens-question Hide the tests and group the adapters by the system they talk to",
                ],
                then: &[
                    "lens-ask \"Hide the tests and group the adapters by the system they talk to\" hexagonal-v2",
                ],
                ..click(
                    "lens.create",
                    "Refine it: hide tests, group adapters by system",
                    "Feedback refines the lens in a second turn, with no hand-made diagram.",
                )
            },
            Guided {
                prep: &["lens-question Look at this project from SHOP-42's perspective"],
                then: &["lens-ask \"Look at this project from SHOP-42's perspective\" shop-42"],
                ..click(
                    "lens.create",
                    "Look at the project from SHOP-42's perspective",
                    "Only what the ticket touches stays lit; the rest recedes.",
                )
            },
        ],
    },
    Chapter {
        title: "A ticket arrives",
        why: "Work starts from a ticket. The case shows the problem plainly, and the code and systems it touches.",
        steps: &[
            Guided {
                emulate: &["tickets"],
                ..key(
                    egui::Key::T,
                    "T",
                    "Press T to open your tickets",
                    "Your open Jira tickets, one key away from the map.",
                )
            },
            Guided {
                emulate: &["case-open SHOP-42"],
                then: &["lens-panel off"],
                ..click(
                    "ticket.SHOP-42",
                    "Open SHOP-42: apply a promo code at checkout",
                    "The case states the problem in plain words, with a schematic of the pod, the database and the secret it touches.",
                )
            },
            Guided {
                emulate: &["case-ingest"],
                ..click(
                    "btn.INGEST*",
                    "Click INGEST so the case gets written",
                    "The case is written from the ticket and the code, and the map keeps SHOP-42's perspective.",
                )
            },
        ],
    },
    Chapter {
        title: "Iterate with the app running",
        why: "The ticket becomes a running prototype you steer in rounds, while every file the agent touches shows on the map.",
        steps: &[
            Guided {
                prep: &["lens-reset"],
                emulate: &["case-prototype"],
                ..click(
                    "btn.PROTOTYPE IT",
                    "Click PROTOTYPE IT",
                    "A worktree and a branch are made for the ticket and an agent session starts.",
                )
            },
            watch(
                "",
                "Watch: the shop starts and the agent builds promo codes",
                "The towers rise as the agent writes files, and the app hot-reloads with a promo field.",
                &[
                    "wait 1500",
                    "devserver",
                    "wait 4000",
                    "app-window",
                    "wait 1500",
                    "write round-1",
                    "agent shop-42 src/cart/promo.ts",
                    "wait 2500",
                    "agent shop-42 src/ui/checkoutView.ts",
                    "wait 1500",
                ],
            ),
            Guided {
                prep: &[
                    "pin src/ui/checkoutView.ts",
                    "type Show the discount on the total, with the old total struck through",
                ],
                hotkey: Some(egui::Key::C),
                ..click(
                    "btn.CAPTURE",
                    "Click CAPTURE, then drag over the shop window",
                    "Your note travels with a screenshot and the line you pinned.",
                )
            },
            Guided {
                emulate: &["send"],
                hotkey: Some(egui::Key::Enter),
                ..click(
                    "btn.SEND",
                    "Send round 1 to the agent",
                    "The agent gets the note, the screenshot and the pinned line.",
                )
            },
            watch(
                "",
                "Watch: the agent answers with round 2",
                "Round 2 arrives and the shop reloads with the discount on the total.",
                &[
                    "wait 1500",
                    "write round-2",
                    "agent shop-42 src/cart/totals.ts",
                    "wait 2500",
                    "agent shop-42 src/ui/checkoutView.ts",
                    "wait 1500",
                ],
            ),
            Guided {
                emulate: &["round-close"],
                hotkey: Some(egui::Key::R),
                ..click(
                    "btn.CLOSE ROUND*",
                    "Close the round",
                    "The rounds list keeps what was asked and what changed, so the steering is recorded.",
                )
            },
        ],
    },
    Chapter {
        title: "Freeze the prototype",
        why: "When the behaviour is right, freezing hands it to the pipeline. Nothing is built by hand from here.",
        steps: &[Guided {
            prep: &["case-go 5", "wait 800"],
            emulate: &["case-freeze"],
            then: &["wait 600", "case-file prototyped", "case-go 5"],
            ..click(
                "btn.FREEZE",
                "Freeze the prototype",
                "The session is distilled into a prototype note and the work goes to the Factory.",
            )
        }],
    },
    Chapter {
        title: "Plan",
        why: "The pipeline plans from the frozen prototype. You read the steps, the evidence and the risks, then approve.",
        steps: &[
            Guided {
                prep: &["case-file planned", "wait 1500", "case-go 2"],
                emulate: &["case-go 3"],
                ..click(
                    "btn.S1",
                    "Open step S1 to read its evidence and risks",
                    "Each step names the files it touches, the evidence behind it and what could go wrong.",
                )
            },
            Guided {
                emulate: &["case-go 3"],
                ..click(
                    "btn.S3",
                    "Check step S3 as well",
                    "The steps are small and ordered, so the review is too.",
                )
            },
            Guided {
                emulate: &["case-approve"],
                ..click(
                    "btn.APPROVE*",
                    "Approve the plan",
                    "Approval is the only gate: from here the Factory builds.",
                )
            },
        ],
    },
    Chapter {
        title: "The Factory builds",
        why: "The pipeline builds, checks, delivers and reviews on its own, and every commit shows on the map.",
        steps: &[Guided {
            prep: &["lens-reset"],
            ..watch(
                "pipeline",
                "Watch: the pipeline builds, checks, delivers and reviews",
                "Three agents committed in turn and the buildings rose; the case reached fixed.",
                &[
                    "case-close",
                    "open {ROOT}/projects/shopfront-shop-42",
                    "wait 2000",
                    "pipeline build",
                    "write factory/step-1",
                    "agent factory-1 src/cart/promo.ts",
                    "wait 2000",
                    "commit feat(cart): apply promo codes\\n\\nRefs: SHOP-42",
                    "wait 2500",
                    "pipeline checks",
                    "write factory/step-2",
                    "agent factory-2 src/checkout/order.ts",
                    "wait 2000",
                    "commit feat(checkout): charge the discounted total\\n\\nRefs: SHOP-42",
                    "wait 2500",
                    "pipeline deliver",
                    "write factory/step-3",
                    "agent factory-3 docs/domain/contexts/Cart/glossary.md",
                    "wait 2000",
                    "commit docs(cart): promo code in the glossary and story\\n\\nRefs: SHOP-42",
                    "wait 1500",
                    "pipeline review",
                    "case-file fixed",
                    "wait 1500",
                ],
            )
        }],
    },
    Chapter {
        title: "Review",
        why: "You review what the agents did, file by file, against main, and sign it off.",
        steps: &[
            Guided {
                prep: &["case-open SHOP-42", "wait 1500", "case-go 6"],
                emulate: &["file src/cart/promo.ts"],
                ..click(
                    "btn.src/*",
                    "Open a changed file to read its diff",
                    "The diff is against main, so you see only what this ticket changed.",
                )
            },
            Guided {
                emulate: &["case-review"],
                ..click(
                    "btn.REVIEWED",
                    "Mark the case reviewed",
                    "The case is signed off and ready to merge.",
                )
            },
        ],
    },
    Chapter {
        title: "Deploy",
        why: "The reviewed change merges and goes to staging, and the map shows the server taking the new version.",
        steps: &[Guided {
            prep: &[
                "case-close",
                "merge-case",
                "open {ROOT}/projects/shopfront",
                "wait 2000",
                "lens-reset",
            ],
            then: &["deploy", "wait 3500"],
            ..click(
                "server",
                "Click the staging server to deploy",
                "A packet travelled from the Depot to staging, which now runs v1.4.0.",
            )
        }],
    },
    Chapter {
        title: "History",
        why: "The map remembers every commit: scrub back and the city shrinks, and any line says why it exists.",
        steps: &[
            Guided {
                prep: &["history last", "wait 1500"],
                emulate: &[
                    "history 0",
                    "wait 2500",
                    "history 3",
                    "wait 2500",
                    "history last",
                    "wait 1500",
                ],
                ..click(
                    "history.slider",
                    "Drag the slider back through the commits",
                    "The city is rebuilt from each commit, so you watch the shop grow.",
                )
            },
            Guided {
                // The Explorer framing may have left the tower off screen.
                prep: &["history last", "wait 2500", "frame src/cart", "wait 1200"],
                then: &["enter src/cart/promo.ts"],
                done_if: Some(inside_promo),
                ..click(
                    "tower:src/cart/promo.ts",
                    "Click the promo tower to step inside",
                    "Inside, every floor is a line of code, tagged with the pull request and story that wrote it.",
                )
            },
            Guided {
                emulate: &["floor-story"],
                ..click(
                    "floor.story",
                    "Click a floor to see why it was built",
                    "The why panel traces the floor back to SHOP-42, the ticket that asked for it.",
                )
            },
        ],
    },
];

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase {
    Intro,
    Step(usize),
    /// After step `n` finished: its explanation shows for a few seconds.
    Explain(usize),
    Free,
}

/// What the headless driver needs to know.
#[derive(Clone, Debug)]
pub struct Info {
    pub chapter: usize,
    pub phase: Phase,
    pub steps: usize,
    pub target: &'static str,
    pub rect: Option<Rect>,
    pub action: Option<&'static str>,
    pub key: Option<egui::Key>,
    pub ready: bool,
}

pub struct State {
    pub chapter: usize,
    pub phase: Phase,
    prepped: bool,
    ran: bool,
    quiet_since: Option<f64>,
    explain_until: f64,
    anchor: Option<Rect>,
    pub hidden: bool,
    /// Set by the `tour-next` drive command.
    pub press_n: bool,
    pub info: Option<Info>,
}

impl Default for State {
    fn default() -> Self {
        State {
            chapter: 0,
            phase: Phase::Intro,
            prepped: false,
            ran: false,
            quiet_since: None,
            explain_until: 0.0,
            anchor: None,
            hidden: false,
            press_n: false,
            info: None,
        }
    }
}

impl State {
    pub fn go_chapter(&mut self, chapter: usize) {
        self.chapter = chapter.min(CHAPTERS.len() - 1);
        self.phase = Phase::Intro;
        self.reset_step();
    }

    fn reset_step(&mut self) {
        self.prepped = false;
        self.ran = false;
        self.quiet_since = None;
        self.anchor = None;
    }

    fn enter(&mut self, phase: Phase) {
        self.phase = phase;
        self.reset_step();
    }

    /// The phase after the intro or after step `done`.
    fn after(&self, done: Option<usize>) -> Phase {
        let chapter = &CHAPTERS[self.chapter];
        let next = done.map_or(0, |d| d + 1);
        if next < chapter.steps.len() {
            Phase::Step(next)
        } else {
            Phase::Free
        }
    }
}

fn wrapped(
    ctx: &egui::Context,
    text: &str,
    size: f32,
    colour: Color32,
    mono: bool,
    width: f32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        text,
        0.0,
        egui::TextFormat {
            font_id: font(ctx, size, mono),
            color: colour,
            line_height: Some(size * 1.4),
            ..Default::default()
        },
    );
    job.wrap.max_width = width;
    ctx.fonts_mut(|f| f.layout_job(job))
}

/// The text block of a callout or card: eyebrow, body, hint. Returns its size.
struct Block {
    eyebrow: std::sync::Arc<egui::Galley>,
    body: std::sync::Arc<egui::Galley>,
    hint: Option<std::sync::Arc<egui::Galley>>,
}

impl Block {
    fn new(
        ctx: &egui::Context,
        eye: &str,
        eye_colour: Color32,
        body: &str,
        size: f32,
        hint: Option<&str>,
        width: f32,
    ) -> Block {
        Block {
            eyebrow: eyebrow(ctx, eye, eye_colour),
            body: wrapped(ctx, body, size, INK, false, width),
            hint: hint.map(|h| wrapped(ctx, h, 11.0, DIM, true, width)),
        }
    }

    fn height(&self) -> f32 {
        self.eyebrow.size().y
            + 8.0
            + self.body.size().y
            + self.hint.as_ref().map_or(0.0, |h| 10.0 + h.size().y)
    }

    fn paint(self, painter: &egui::Painter, at: Pos2) {
        let mut y = at.y;
        let h = self.eyebrow.size().y;
        painter.galley(pos2(at.x, y), self.eyebrow, YELLOW);
        y += h + 8.0;
        let bh = self.body.size().y;
        painter.galley(pos2(at.x, y), self.body, INK);
        y += bh + 10.0;
        if let Some(hint) = self.hint {
            painter.galley(pos2(at.x, y), hint, DIM);
        }
    }
}

/// Puts a `size` box near `target`, where it covers the least of `taken`:
/// beside the target when a side is clear, else on the free map (`free`),
/// else wherever overlaps least. The panels around `free` count as taken.
fn beside(
    screen: Rect,
    target: Rect,
    size: egui::Vec2,
    free: Option<Rect>,
    taken: &[Rect],
) -> Rect {
    let gap = 18.0;
    let room = screen.shrink(12.0);
    let mut taken = taken.to_vec();
    taken.push(target.expand(8.0));
    let mut candidates = vec![
        pos2(target.right() + gap, target.center().y - size.y / 2.0),
        pos2(
            target.left() - gap - size.x,
            target.center().y - size.y / 2.0,
        ),
        pos2(target.center().x - size.x / 2.0, target.bottom() + gap),
        pos2(
            target.center().x - size.x / 2.0,
            target.top() - gap - size.y,
        ),
    ];
    if let Some(free) = free {
        taken.extend([
            Rect::from_min_max(screen.min, pos2(screen.right(), free.top())),
            Rect::from_min_max(pos2(screen.left(), free.bottom()), screen.max),
            Rect::from_min_max(pos2(screen.left(), free.top()), free.left_bottom()),
            Rect::from_min_max(free.right_top(), pos2(screen.right(), free.bottom())),
        ]);
        let inner = free.shrink(12.0);
        let ys = [
            target.center().y - size.y / 2.0,
            target.bottom() + gap,
            target.top() - gap - size.y,
            inner.top(),
            inner.bottom() - size.y,
        ];
        let xs = [
            target.center().x - size.x / 2.0,
            inner.left(),
            inner.center().x - size.x / 2.0,
            inner.right() - size.x,
        ];
        for y in ys {
            for x in xs {
                candidates.push(pos2(x, y));
            }
        }
    }
    let overlap = |rect: Rect| -> f32 {
        taken
            .iter()
            .map(|t| {
                let i = t.intersect(rect);
                if i.is_positive() { i.area() } else { 0.0 }
            })
            .sum()
    };
    candidates
        .into_iter()
        .map(|at| {
            // Kept on screen.
            let at = pos2(
                at.x.clamp(room.left(), (room.right() - size.x).max(room.left())),
                at.y.clamp(room.top(), (room.bottom() - size.y).max(room.top())),
            );
            Rect::from_min_size(at, size)
        })
        .min_by(|a, b| {
            let key = |r: &Rect| (overlap(*r), (r.center() - target.center()).length());
            key(a)
                .partial_cmp(&key(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(Rect::from_min_size(room.min, size))
}

/// Dims everything but a hole at `hole`, with a pulsing chamfered ring.
fn highlight(ctx: &egui::Context, hole: Rect, now: f64) {
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("tutorial-dim"),
    ));
    let hole = hole.expand(6.0).intersect(screen);
    let dim = Color32::from_black_alpha(130);
    for rect in [
        Rect::from_min_max(screen.min, pos2(screen.right(), hole.top())),
        Rect::from_min_max(pos2(screen.left(), hole.bottom()), screen.max),
        Rect::from_min_max(
            pos2(screen.left(), hole.top()),
            pos2(hole.left(), hole.bottom()),
        ),
        Rect::from_min_max(
            pos2(hole.right(), hole.top()),
            pos2(screen.right(), hole.bottom()),
        ),
    ] {
        painter.rect_filled(rect, 0.0, dim);
    }
    let pulse = ((now * std::f64::consts::TAU / 1.2).sin() * 0.5 + 0.5) as f32;
    let cut = 8.0_f32.min(hole.height() / 3.0);
    for step in 0..10 {
        let spread = 2.0 + 2.0 * pulse + step as f32 * 1.4;
        let fade = 1.0 - step as f32 / 10.0;
        painter.add(egui::Shape::closed_line(
            chamfer(hole.expand(spread), cut + spread),
            Stroke::new(
                1.5,
                YELLOW.gamma_multiply((0.25 + 0.45 * pulse) * fade * fade),
            ),
        ));
    }
    painter.add(egui::Shape::closed_line(
        chamfer(hole, cut),
        Stroke::new(2.5, YELLOW),
    ));
    ctx.request_repaint_after(Duration::from_millis(33));
}

fn callout(
    ctx: &egui::Context,
    anchor: Option<Rect>,
    eye: &str,
    eye_colour: Color32,
    text: &str,
    hint: &str,
) {
    let screen = ctx.content_rect();
    let width = 300.0;
    let mut block = Block::new(ctx, eye, eye_colour, text, 15.0, Some(hint), width);
    // The eyebrow takes the colour asked for.
    block.eyebrow = eyebrow(ctx, eye, eye_colour);
    let size = vec2(width + 32.0, block.height() + 32.0);
    let free = super::free_rect(ctx);
    let taken = super::overlay_rects(ctx);
    let rect = match (anchor, free) {
        (Some(target), _) => beside(screen, target, size, free, &taken),
        // No target: near the bottom of the free map, clear of its overlays.
        (None, Some(free)) => {
            let near = Rect::from_center_size(
                pos2(free.center().x, free.bottom() - size.y / 2.0 - 12.0),
                vec2(0.0, 0.0),
            );
            beside(screen, near, size, Some(free), &taken)
        }
        (None, None) => Rect::from_min_size(
            pos2(
                screen.center().x - size.x / 2.0,
                screen.bottom() - size.y - 56.0,
            ),
            size,
        ),
    };
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::new("tutorial-callout"),
    ));
    panel(&painter, rect);
    block.paint(&painter, rect.min + vec2(16.0, 16.0));
}

/// A yellow chamfered button with a breathing glow, like the launcher's CTA.
fn cta(ui: &mut egui::Ui, id: &str, label: &str, key: &str) -> egui::Response {
    let ctx = ui.ctx().clone();
    let label_job = tracked(&ctx, &label.to_uppercase(), 14.0, ON_YELLOW, false);
    let key_font = font(&ctx, 12.0, true);
    let label_galley = ctx.fonts_mut(|f| f.layout_job(label_job));
    let key_galley = ctx.fonts_mut(|f| f.layout_no_wrap(key.to_owned(), key_font, ON_YELLOW));
    let size = vec2(label_galley.size().x + key_galley.size().x + 52.0, 40.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    super::mark(&ctx, id, rect);
    let breath = ((ctx.input(|i| i.time) * std::f64::consts::TAU / 1.6).sin() * 0.5 + 0.5) as f32;
    for step in 0..14 {
        let grow = step as f32 + 0.5;
        let fade = 1.0 - step as f32 / 14.0;
        ui.painter().add(egui::Shape::closed_line(
            chamfer(rect.expand(grow), 8.0 + grow),
            Stroke::new(
                1.0,
                YELLOW.gamma_multiply((0.2 + 0.35 * breath) * fade * fade),
            ),
        ));
    }
    let fill = if response.hovered() {
        Color32::from_rgb(0xff, 0xf5, 0x5a)
    } else {
        YELLOW
    };
    ui.painter().add(egui::Shape::convex_polygon(
        chamfer(rect, 8.0),
        fill,
        Stroke::NONE,
    ));
    let mid = rect.center().y;
    let left = rect.left() + 18.0;
    ui.painter().galley(
        pos2(left, mid - label_galley.size().y / 2.0),
        label_galley.clone(),
        ON_YELLOW,
    );
    ui.painter().galley(
        pos2(
            rect.right() - 16.0 - key_galley.size().x,
            mid - key_galley.size().y / 2.0,
        ),
        key_galley,
        ON_YELLOW,
    );
    ctx.request_repaint_after(Duration::from_millis(33));
    response
}

/// The intro card. `true` when START was pressed.
fn intro_card(ctx: &egui::Context, chapter: usize) -> bool {
    let c = &CHAPTERS[chapter];
    let mut go = false;
    egui::Area::new(egui::Id::new("tutorial-intro"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_BOTTOM, [0.0, -40.0])
        .show(ctx, |ui| {
            let width = 560.0;
            let eye = eyebrow(
                ctx,
                &format!("Chapter {} of {}", chapter, CHAPTERS.len() - 1),
                YELLOW,
            );
            let title = wrapped(ctx, &c.title.to_uppercase(), 22.0, INK, false, width);
            let why = wrapped(ctx, c.why, 15.0, DIM, false, width);
            let core: Vec<_> = if chapter == 0 {
                CORE.iter()
                    .flat_map(|(head, text)| {
                        [
                            wrapped(ctx, head, 15.0, CYAN, false, width),
                            wrapped(ctx, text, 14.0, INK, false, width),
                        ]
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let core_h: f32 = core.iter().map(|g| g.size().y + 6.0).sum();
            let height = eye.size().y
                + 8.0
                + title.size().y
                + 10.0
                + why.size().y
                + 14.0
                + core_h
                + 40.0
                + 14.0;
            let (rect, _) =
                ui.allocate_exact_size(vec2(width + 40.0, height + 28.0), egui::Sense::hover());
            panel(ui.painter(), rect);
            let mut y = rect.top() + 20.0;
            let x = rect.left() + 20.0;
            let h = eye.size().y;
            ui.painter().galley(pos2(x, y), eye, YELLOW);
            y += h + 8.0;
            let h = title.size().y;
            ui.painter().galley(pos2(x, y), title, INK);
            y += h + 10.0;
            let h = why.size().y;
            ui.painter().galley(pos2(x, y), why, DIM);
            y += h + 14.0;
            for g in core {
                let h = g.size().y;
                ui.painter().galley(pos2(x, y), g, INK);
                y += h + 6.0;
            }
            let label = if c.steps.is_empty() { "Next" } else { "Start" };
            let mut button = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(Rect::from_min_size(pos2(x, y + 4.0), vec2(200.0, 40.0))),
            );
            go = cta(&mut button, "tour.start", &format!("{label} >"), "N").clicked();
        });
    go
}

/// The free-play bar. `true` when NEXT FEATURE was pressed.
fn free_bar(ctx: &egui::Context, last: bool) -> bool {
    let mut go = false;
    egui::Area::new(egui::Id::new("tutorial-free"))
        .order(egui::Order::Foreground)
        .anchor(Align2::RIGHT_BOTTOM, [-16.0, -16.0])
        .show(ctx, |ui| {
            let eye = eyebrow(ctx, "Free play · explore this screen", CYAN);
            let button_w = 230.0;
            let size = vec2(eye.size().x + button_w + 56.0, 64.0);
            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            panel(ui.painter(), rect);
            let h = eye.size().y;
            ui.painter().galley(
                pos2(rect.left() + 18.0, rect.center().y - h / 2.0),
                eye,
                CYAN,
            );
            if !last {
                let mut button =
                    ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(
                        pos2(rect.right() - button_w - 14.0, rect.top() + 12.0),
                        vec2(button_w, 40.0),
                    )));
                go = cta(&mut button, "tour.next", "Next feature >", "N").clicked();
            }
        });
    go
}

/// Where `target` is on screen right now.
fn resolve(app: &CellworksApp, ctx: &egui::Context, target: &str) -> Option<Rect> {
    let screen = ctx.content_rect();
    let found = if let Some(rel) = target.strip_prefix("tower:") {
        // A file's towers are its items: `<file>/<item>`. Their outlines join.
        app.towers()
            .iter()
            .filter(|t| t.path.ancestors().any(|a| a.ends_with(rel)))
            .map(|t| t.outline)
            .reduce(Rect::union)
    } else if let Some(rel) = target.strip_prefix("building:") {
        let (map, view) = (app.map()?, app.view.as_ref()?);
        let area = map.find(&map.root.join(rel))?.area;
        let corners = [
            area.left_top(),
            area.right_top(),
            area.left_bottom(),
            area.right_bottom(),
        ];
        let points = corners
            .iter()
            .flat_map(|c| [view.to_screen(*c, 0.0), view.to_screen(*c, 30.0)]);
        Some(Rect::from_points(&points.collect::<Vec<_>>()))
    } else {
        cellworks_workspace::marked(ctx, target)
    };
    found.filter(|r| r.intersects(screen) && r.width() > 2.0 && r.height() > 2.0)
}

fn inside_promo(app: &CellworksApp) -> bool {
    app.navigation
        .inside()
        .is_some_and(|(item, _)| item.ancestors().any(|a| a.ends_with("src/cart/promo.ts")))
}

fn project_open(app: &CellworksApp) -> bool {
    app.opened().is_some()
}

fn released_in(ctx: &egui::Context, rect: Rect) -> bool {
    ctx.input(|i| {
        i.pointer.primary_released()
            && i.pointer
                .interact_pos()
                .is_some_and(|p| rect.expand(4.0).contains(p))
    })
}

fn released(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.pointer.primary_released())
}

fn idle(tour: &Tour) -> bool {
    tour.queue.is_empty() && tour.resume.is_none()
}

/// What the tutorial asks of the tour's queue, applied after it ran.
#[derive(Default)]
struct Out {
    lines: Vec<String>,
    clear: bool,
    fast_forward: bool,
}

impl Out {
    fn queue(&mut self, lines: &[&str]) {
        self.lines.extend(lines.iter().map(|l| (*l).to_owned()));
    }
}

/// Draws the tutorial and advances it. Runs after the queue was drained.
pub fn run(ctx: &egui::Context, app: &mut CellworksApp, mut n: bool, back: bool, esc: bool) {
    let now = ctx.input(|i| i.time);
    let Some(tour) = app.demo.as_mut() else {
        return;
    };
    let mut tut = std::mem::take(&mut tour.tut);
    let mut quiet = idle(tour);
    let mut out = Out::default();
    n |= std::mem::take(&mut tut.press_n);
    if back {
        tut.go_chapter(tut.chapter.saturating_sub(1));
        out.clear = true;
    }
    if esc {
        tut.hidden = !tut.hidden;
    }
    let hidden = tut.hidden;
    let n = n && !hidden;
    let chapter = tut.chapter;
    let c = &CHAPTERS[chapter];
    let phase = tut.phase;
    let last_chapter = chapter + 1 == CHAPTERS.len();
    let mut info = Info {
        chapter,
        phase,
        steps: c.steps.len(),
        target: "",
        rect: None,
        action: None,
        key: None,
        ready: false,
    };

    match phase {
        Phase::Intro => {
            info.ready = true;
            if (!hidden && intro_card(ctx, chapter)) || n {
                let next = tut.after(None);
                tut.enter(next);
            }
        }
        Phase::Free => {
            info.ready = true;
            if !hidden && (free_bar(ctx, last_chapter) || n) && !last_chapter {
                tut.go_chapter(chapter + 1);
            }
        }
        Phase::Explain(i) => {
            info.ready = true;
            if !hidden {
                callout(
                    ctx,
                    tut.anchor,
                    &format!("What you see · step {}/{}", i + 1, c.steps.len()),
                    CYAN,
                    c.steps[i].explain,
                    "any click or N continues",
                );
            }
            ctx.request_repaint_after(Duration::from_millis(100));
            if now >= tut.explain_until || released(ctx) || n {
                let next = tut.after(Some(i));
                tut.enter(next);
            }
        }
        Phase::Step(i) => {
            let step = &c.steps[i];
            if !tut.prepped {
                tut.prepped = true;
                out.queue(step.prep);
                quiet &= step.prep.is_empty();
            }
            info.target = step.target;
            info.ready = quiet;
            info.action = Some(match step.action {
                Action::Click => "click",
                Action::Key(..) => "key",
                Action::Watch => "watch",
            });
            if let Action::Key(k, _) = step.action {
                info.key = Some(k);
            }
            let eye = format!("Step {}/{} · {}", i + 1, c.steps.len(), c.title);
            if quiet {
                let rect = if step.target.is_empty() {
                    None
                } else {
                    resolve(app, ctx, step.target)
                };
                info.rect = rect;
                let mut done = false;
                match step.action {
                    Action::Watch => {
                        if !tut.ran {
                            tut.ran = true;
                            out.queue(step.then);
                        } else {
                            let since = *tut.quiet_since.get_or_insert(now);
                            done = now - since > 0.4;
                        }
                        if !hidden {
                            if let Some(rect) = rect {
                                highlight(ctx, rect, now);
                            }
                            callout(
                                ctx,
                                rect,
                                &eye,
                                YELLOW,
                                step.callout,
                                "watching · N skips ahead",
                            );
                        }
                        ctx.request_repaint_after(Duration::from_millis(100));
                    }
                    Action::Click | Action::Key(..) => {
                        if !hidden {
                            if let Some(rect) = rect {
                                highlight(ctx, rect, now);
                            }
                            let hint = match step.action {
                                Action::Key(_, label) => {
                                    format!("press {label} · or N to do it for me")
                                }
                                _ => "click · or N to do it for me".to_owned(),
                            };
                            // Without its target the callout still shows, so N is never hidden.
                            callout(ctx, rect, &eye, YELLOW, step.callout, &hint);
                        }
                        let typing = ctx.egui_wants_keyboard_input();
                        done = match step.action {
                            Action::Key(k, _) => !typing && ctx.input(|i| i.key_pressed(k)),
                            _ => rect.is_some_and(|r| released_in(ctx, r)),
                        };
                        done |= step.done_if.is_some_and(|f| f(app));
                        // Enter sends from inside the note field; the letter keys only work outside it.
                        done |= step.hotkey.is_some_and(|k| {
                            (k == egui::Key::Enter || !typing) && ctx.input(|i| i.key_pressed(k))
                        });
                        if n && !done {
                            out.queue(step.emulate);
                            done = true;
                        }
                    }
                }
                if done {
                    if !matches!(step.action, Action::Watch) {
                        out.queue(step.then);
                    }
                    tut.enter(Phase::Explain(i));
                    tut.explain_until = now + 4.0;
                    tut.anchor = rect;
                }
            } else if n && matches!(step.action, Action::Watch) {
                out.fast_forward = true;
            }
        }
    }
    tut.info = Some(info);
    let tour = app.demo.as_mut().unwrap();
    tour.tut = tut;
    if out.clear {
        tour.queue.clear();
        tour.resume = None;
    }
    if out.fast_forward {
        tour.resume = None;
    }
    tour.queue.extend(out.lines);
}
