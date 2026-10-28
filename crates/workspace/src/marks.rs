//! PROTOTYPE (stakeholder demo): widgets report where they are, by id, so the
//! tutorial overlay can point at them. Cheap enough to run in normal mode.

use std::collections::HashMap;

#[derive(Clone, Default)]
struct Marks(HashMap<String, (egui::Rect, f64)>);

fn store() -> egui::Id {
    egui::Id::new("demo-marks")
}

/// Records that the widget `id` was drawn this frame at `rect`.
pub fn mark(ctx: &egui::Context, id: &str, rect: egui::Rect) {
    let now = ctx.input(|i| i.time);
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Marks>(store())
            .0
            .insert(id.to_owned(), (rect, now));
    });
}

/// Where `id` was last drawn, if that was within the last half second.
pub fn marked(ctx: &egui::Context, id: &str) -> Option<egui::Rect> {
    let now = ctx.input(|i| i.time);
    ctx.data(|d| {
        let marks = d.get_temp::<Marks>(store())?;
        match id.strip_suffix('*') {
            Some(prefix) => marks
                .0
                .iter()
                .filter(|(key, (_, at))| key.starts_with(prefix) && now - at < 0.5)
                .min_by(|a, b| a.0.cmp(b.0))
                .map(|(_, v)| *v),
            None => marks.0.get(id).copied(),
        }
    })
    .filter(|(_, at)| now - at < 0.5)
    .map(|(rect, _)| rect)
}
