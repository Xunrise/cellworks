//! The Tickets panel (`T`): my tickets from every enabled source in one list,
//! filtered by source, status category and search, grouped by source or by
//! project. A row opens as a case; sources that can write get a status menu.

use std::collections::BTreeSet;
use std::path::Path;

use egui::{Color32, RichText, Ui};

use super::case::{ATTENTION, DANGER_TEXT, FLOW, INFO, INK, INK_DIM, LANE_CALLS, OK, WARN};
use super::casefile::{self, Status};
use crate::tickets::{Category, SignIn, SourceId, Ticket, TicketDesk, TicketRef};

/// What the panel groups its rows by.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Grouping {
    #[default]
    Source,
    Project,
}

/// The panel's own state: the filters and the typed key.
pub struct TicketsPanel {
    pub typed: String,
    search: String,
    hidden_sources: BTreeSet<SourceId>,
    categories: BTreeSet<Category>,
    grouping: Grouping,
    /// Only the tickets whose repository is the opened project.
    this_project: bool,
}

impl Default for TicketsPanel {
    fn default() -> Self {
        TicketsPanel {
            typed: String::new(),
            search: String::new(),
            hidden_sources: BTreeSet::new(),
            categories: [Category::Todo, Category::InProgress].into(),
            grouping: Grouping::default(),
            this_project: false,
        }
    }
}

fn colour(source: SourceId) -> Color32 {
    match source {
        SourceId::Jira => INFO,
        SourceId::Cockpit => FLOW,
        SourceId::GitHub => INK,
        SourceId::Linear => LANE_CALLS,
    }
}

fn category_colour(category: Category) -> Color32 {
    match category {
        Category::Todo => INK_DIM,
        Category::InProgress => ATTENTION,
        Category::Done => OK,
        Category::Canceled => WARN,
    }
}

/// Whether `ticket` passes the search: every word in its key, title, project
/// or labels.
fn matches(ticket: &Ticket, search: &str) -> bool {
    let hay = format!(
        "{} {} {} {} {}",
        ticket.key,
        ticket.title,
        ticket.project,
        ticket.labels.join(" "),
        ticket.status
    )
    .to_lowercase();
    search
        .to_lowercase()
        .split_whitespace()
        .all(|word| hay.contains(word))
}

impl TicketsPanel {
    /// Draws the panel; answers the ticket (in its stable form) to open a
    /// case for. `repo` is the opened project's folder, when there is one.
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        desk: &mut dyn TicketDesk,
        repo: Option<&Path>,
    ) -> Option<String> {
        desk.poll();
        let mut open = None;
        ui.horizontal(|ui| {
            let field = ui.add(
                egui::TextEdit::singleline(&mut self.typed)
                    .hint_text("any key: SHOP-42, cockpit:TIC-2, owner/repo#12")
                    .desired_width(300.0),
            );
            let entered =
                field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            if (ui.button("Open").clicked() || entered) && !self.typed.trim().is_empty() {
                open = Some(TicketRef::parse(&self.typed).to_string());
            }
        });
        ui.separator();

        let sources = desk.sources();
        let all = desk.all();
        let here: BTreeSet<TicketRef> = repo
            .map(|repo| desk.for_repo(repo))
            .unwrap_or_default()
            .iter()
            .map(Ticket::reference)
            .collect();

        // Source chips: a click hides or shows the source; the hover says how
        // it stands.
        ui.horizontal_wrapped(|ui| {
            for state in &sources {
                let count = all.iter().filter(|(t, _)| t.source == state.id).count();
                let shown = !self.hidden_sources.contains(&state.id);
                let line = match (&state.failure, state.fetching, state.synced) {
                    (_, true, _) => "syncing…".to_owned(),
                    (Some(reason), _, _) => format!("sync failed: {reason}"),
                    (None, _, Some(synced)) => synced.label(),
                    (None, _, None) => "not synced".to_owned(),
                };
                let mut label = RichText::new(format!("{} {count}", state.id.name())).small();
                if state.failure.is_some() {
                    label = label.color(DANGER_TEXT);
                } else if shown {
                    label = label.color(colour(state.id)).strong();
                }
                let chip = ui
                    .add(egui::Button::selectable(shown, label))
                    .on_hover_text(line);
                cellworks_workspace::mark(
                    ui.ctx(),
                    &format!("tickets.source.{}", state.id.id()),
                    chip.rect,
                );
                if chip.clicked() && !self.hidden_sources.remove(&state.id) {
                    self.hidden_sources.insert(state.id);
                }
            }
            if ui.small_button("Refresh").clicked() {
                desk.refresh();
            }
        });
        for state in &sources {
            match &state.sign_in {
                SignIn::Needed | SignIn::Failed(_) => {
                    if ui
                        .button(format!("Sign in to {}", state.id.name()))
                        .clicked()
                    {
                        desk.start_sign_in(state.id);
                    }
                }
                SignIn::Pending => {
                    ui.label(
                        RichText::new(format!(
                            "{}: sign-in pending in the browser",
                            state.id.name()
                        ))
                        .weak(),
                    );
                }
                _ => {}
            }
        }
        ui.horizontal_wrapped(|ui| {
            for category in Category::ALL {
                let on = self.categories.contains(&category);
                let chip = ui.add(egui::Button::selectable(
                    on,
                    RichText::new(category.name())
                        .small()
                        .color(category_colour(category)),
                ));
                if chip.clicked() && !self.categories.remove(&category) {
                    self.categories.insert(category);
                }
            }
            ui.separator();
            for (grouping, name) in [
                (Grouping::Source, "by source"),
                (Grouping::Project, "by project"),
            ] {
                if ui
                    .add(egui::Button::selectable(
                        self.grouping == grouping,
                        RichText::new(name).small(),
                    ))
                    .clicked()
                {
                    self.grouping = grouping;
                }
            }
            if repo.is_some() {
                ui.separator();
                let chip = ui.add(egui::Button::selectable(
                    self.this_project,
                    RichText::new(format!("this project {}", here.len())).small(),
                ));
                cellworks_workspace::mark(ui.ctx(), "tickets.this-project", chip.rect);
                if chip.clicked() {
                    self.this_project = !self.this_project;
                }
            }
        });
        ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .hint_text("search key, title, project, label")
                .desired_width(f32::INFINITY),
        );

        let mut rows: Vec<&Ticket> = all
            .iter()
            .map(|(ticket, _)| ticket)
            .filter(|t| !self.hidden_sources.contains(&t.source))
            .filter(|t| self.categories.contains(&t.category))
            .filter(|t| !(self.this_project && repo.is_some()) || here.contains(&t.reference()))
            .filter(|t| matches(t, &self.search))
            .collect();
        rows.sort_by(|a, b| b.updated.cmp(&a.updated).then(a.key.cmp(&b.key)));
        let group_of = |t: &Ticket| match self.grouping {
            Grouping::Source => t.source.name().to_owned(),
            Grouping::Project if t.project.is_empty() => "no project".to_owned(),
            Grouping::Project => t.project.clone(),
        };
        let mut groups: Vec<String> = rows.iter().map(|t| group_of(t)).collect();
        groups.sort();
        groups.dedup();

        ui.separator();
        egui::ScrollArea::vertical()
            .max_height(420.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                if rows.is_empty() {
                    ui.label(RichText::new("no tickets match").weak());
                }
                for group in groups {
                    ui.label(
                        RichText::new(group.to_uppercase())
                            .small()
                            .strong()
                            .color(INK_DIM),
                    );
                    for ticket in rows.iter().filter(|t| group_of(t) == group) {
                        if let Some(key) = self.row(ui, desk, ticket, &sources) {
                            open = Some(key);
                        }
                    }
                }
            });
        open
    }

    fn row(
        &self,
        ui: &mut Ui,
        desk: &mut dyn TicketDesk,
        ticket: &Ticket,
        sources: &[crate::tickets::SourceState],
    ) -> Option<String> {
        let which = ticket.reference();
        let mut open = None;
        let case = casefile::status_of(&which.to_string());
        ui.horizontal(|ui| {
            let (mark, tone) = match case {
                Some(Status::Fixed) => ("FIXED", OK),
                Some(status) => (status.name(), ATTENTION),
                None => ("", INK_DIM),
            };
            ui.add_sized(
                [60.0, 16.0],
                egui::Label::new(
                    RichText::new(ticket.source.name().to_uppercase())
                        .small()
                        .strong()
                        .color(colour(ticket.source)),
                ),
            );
            let writable = sources.iter().any(|s| s.id == ticket.source && s.writable);
            let status = RichText::new(&ticket.status)
                .small()
                .color(category_colour(ticket.category));
            if writable {
                ui.menu_button(status, |ui| {
                    for category in Category::ALL {
                        if ui
                            .selectable_label(ticket.category == category, category.name())
                            .clicked()
                        {
                            desk.set_status(&which, category);
                            ui.close();
                        }
                    }
                })
                .response
                .on_hover_text("move it to another status at its tracker");
            } else {
                ui.add_sized([90.0, 16.0], egui::Label::new(status).truncate());
            }
            if !mark.is_empty() {
                ui.label(RichText::new(mark).small().strong().color(tone))
                    .on_hover_text("where the case stands");
            }
            let entry = ui.add(
                egui::Button::selectable(false, format!("{}  {}", ticket.key, ticket.title)).wrap(),
            );
            cellworks_workspace::mark(ui.ctx(), &format!("ticket.{}", ticket.key), entry.rect);
            if entry
                .on_hover_text(format!(
                    "{} · {} · updated {}",
                    ticket.source.name(),
                    ticket.status,
                    ticket.updated
                ))
                .clicked()
            {
                open = Some(which.to_string());
            }
        });
        open
    }
}
