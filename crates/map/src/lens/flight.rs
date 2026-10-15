//! A switch of lens: every tower lifts off where it stood, arcs over and
//! lands in its new place, staggered so the city rearranges rather than cuts.

use std::collections::HashMap;
use std::path::PathBuf;

use egui::{Rect, vec2};

use crate::graph::{self, ProjectMap};

/// How long one tower is in the air, and the most a tower waits to take off.
const FLIGHT: f64 = 0.9;
const STAGGER: f64 = 0.6;

/// Where every tower stood when the lens changed, and when.
#[derive(Clone, Debug, PartialEq)]
pub struct Flight {
    /// Each tower's ground and the height it stood at.
    from: HashMap<PathBuf, (Rect, f32)>,
    started: f64,
}

impl Flight {
    /// A flight from the towers of `map` as they stand, at time `now` in
    /// seconds.
    pub fn from(map: &ProjectMap, now: f64) -> Flight {
        Flight {
            from: map
                .districts
                .iter()
                .flat_map(graph::items)
                .map(|item| (item.path.clone(), (item.area, item.base)))
                .collect(),
            started: now,
        }
    }

    /// Whether every tower has landed by time `now`: the last takes off
    /// after the longest wait and flies for the full flight.
    pub fn landed(&self, now: f64) -> bool {
        now - self.started >= FLIGHT + STAGGER
    }

    /// Moves every tower of `map` part of the way from where it stood to its
    /// place in `map`, as seen at time `now`; returns whether all have landed.
    /// A tower that did not stand anywhere before is simply there.
    pub fn apply(&self, map: &mut ProjectMap, now: f64) -> bool {
        let elapsed = now - self.started;
        let mut landed = true;
        for node in graph::nodes_mut(&mut map.districts) {
            for item in &mut node.items {
                let Some(&(start, base)) = self.from.get(&item.path) else {
                    continue;
                };
                let p = ((elapsed - delay(&item.path)) / FLIGHT).clamp(0.0, 1.0) as f32;
                landed &= p >= 1.0;
                // Ease in and out, and a hop: up the screen is towards -x, -y.
                let eased = if p < 0.5 {
                    4.0 * p * p * p
                } else {
                    1.0 - (-2.0 * p + 2.0).powi(3) / 2.0
                };
                let distance = (item.area.center() - start.center()).length();
                let hop = (std::f32::consts::PI * p).sin() * (20.0 + distance * 0.25);
                let centre =
                    start.center() + (item.area.center() - start.center()) * eased - vec2(hop, hop);
                let size = start.size() + (item.area.size() - start.size()) * eased;
                item.area = Rect::from_center_size(centre, size);
                item.base = base + (item.base - base) * eased;
            }
        }
        landed
    }
}

/// How long the tower at `path` waits to take off: always the same for it.
fn delay(path: &std::path::Path) -> f64 {
    let hash = crate::arrange::seed(path.as_os_str().as_encoded_bytes());
    f64::from(hash % 1000) / 1000.0 * STAGGER
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::{Directories, default_prefix_rule};

    fn map(files: &[&str]) -> ProjectMap {
        let files: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();
        let strategy = Directories {
            prefix: default_prefix_rule(),
        };
        let mut map = ProjectMap::of("shop", Path::new("/p"), &files, &strategy);
        for (n, node) in graph::nodes_mut(&mut map.districts).into_iter().enumerate() {
            node.items.push(crate::Item {
                name: format!("f{n}"),
                path: node.path.join(format!("f{n}")),
                kind: crate::ItemKind::Function,
                tests: false,
                floors: Vec::new(),
                area: node.area,
                base: 0.0,
                declaration: Default::default(),
            });
        }
        map
    }

    #[test]
    fn towers_travel_from_where_they_stood_and_land_where_they_go() {
        let before = map(&["a/x.rs", "b/y.rs"]);
        let mut after = before.clone();
        for node in graph::nodes_mut(&mut after.districts) {
            for item in &mut node.items {
                item.area = item.area.translate(vec2(100.0, 0.0));
            }
        }
        let flight = Flight::from(&before, 10.0);

        let mut early = after.clone();
        let landed_early = flight.apply(&mut early, 10.0);
        let mut late = after.clone();
        let landed_late = flight.apply(&mut late, 10.0 + FLIGHT + STAGGER);

        let near =
            |a: Rect, b: Rect| (a.min - b.min).length() < 1e-3 && (a.max - b.max).length() < 1e-3;
        let first = |map: &ProjectMap| map.districts[0].nodes[0].items[0].area;
        assert!(!landed_early);
        assert!(near(first(&early), first(&before)), "still where it stood");
        assert!(landed_late);
        assert!(near(first(&late), first(&after)), "landed where it goes");
    }
}
