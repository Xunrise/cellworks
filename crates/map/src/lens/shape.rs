//! Where the districts of a lens stand towards each other: the shape that
//! says which lens is on before a name is read.

use std::collections::HashMap;

use egui::{Pos2, Rect, Vec2, pos2};

use super::cluster::Weights;
use crate::graph::{District, DistrictKind};
use crate::layout;

/// The space kept between districts placed by a lens.
const GAP: f32 = 40.0;

/// The districts the lens arranges: every one but the tests and the
/// outskirts, which stand aside.
pub(crate) fn core(districts: &[District]) -> Vec<usize> {
    (0..districts.len())
        .filter(|d| {
            !matches!(
                districts[*d].kind,
                DistrictKind::Tests | DistrictKind::Outskirts
            )
        })
        .collect()
}

fn radius(district: &District) -> f32 {
    district.area.width() / 2.0
}

/// A network: districts linked by `weights` pull together, all push each
/// other away, a light pull keeps loose ones from drifting off, and no two
/// end up overlapping. Starts where the spiral put them.
pub(crate) fn network(districts: &mut [District], weights: &Weights) {
    let core = core(districts);
    let mut at: HashMap<usize, Pos2> = core
        .iter()
        .map(|d| (*d, districts[*d].area.center()))
        .collect();
    let strongest = weights.values().copied().fold(1.0, f32::max);
    const STEPS: usize = 400;
    for step in 0..STEPS {
        let cool = 1.0 - step as f32 / STEPS as f32;
        let mut push: HashMap<usize, Vec2> = HashMap::new();
        for (i, a) in core.iter().enumerate() {
            for b in &core[i + 1..] {
                let delta = at[b] - at[a];
                let distance = delta.length().max(1.0);
                let rest = radius(&districts[*a]) + radius(&districts[*b]) + GAP;
                let mut force = -rest * rest / (distance * distance) * 20.0;
                let w = weights.get(&(*a.min(b), *a.max(b))).copied().unwrap_or(0.0);
                if w > 0.0 {
                    force += (distance - rest) * 0.2 * (w / strongest).sqrt();
                }
                let f = delta / distance * force * cool;
                *push.entry(*a).or_default() += f;
                *push.entry(*b).or_default() -= f;
            }
            *push.entry(*a).or_default() -= at[a].to_vec2() * 0.002;
        }
        for (d, f) in push {
            let length = f.length();
            let f = if length > 30.0 { f / length * 30.0 } else { f };
            if let Some(p) = at.get_mut(&d) {
                *p += f;
            }
        }
    }
    separate(districts, &core, &mut at);
    move_to(districts, &at);
}

/// An orbit: the district most linked by `weights` in the middle, the others
/// around it, nearer the more they are linked to it.
pub(crate) fn orbit(districts: &mut [District], weights: &Weights) -> Option<usize> {
    let core = core(districts);
    let total = |d: usize| -> f32 {
        weights
            .iter()
            .filter(|((a, b), _)| *a == d || *b == d)
            .map(|(_, w)| w)
            .sum()
    };
    let hub = *core
        .iter()
        .max_by(|a, b| total(**a).total_cmp(&total(**b)).then(b.cmp(a)))?;
    let to_hub = |d: usize| {
        weights
            .get(&(d.min(hub), d.max(hub)))
            .copied()
            .unwrap_or(0.0)
    };
    let strongest = core.iter().map(|d| to_hub(*d)).fold(1e-6, f32::max);
    let mut others: Vec<usize> = core.iter().copied().filter(|d| *d != hub).collect();
    others.sort_by(|a, b| to_hub(*b).total_cmp(&to_hub(*a)).then(a.cmp(b)));
    let mut at: HashMap<usize, Pos2> = HashMap::from([(hub, Pos2::ZERO)]);
    for (k, d) in others.iter().enumerate() {
        let r = radius(&districts[*d]);
        let mut distance =
            radius(&districts[hub]) + r + GAP + (1.0 - to_hub(*d) / strongest) * 300.0;
        // The golden angle spreads them round the hub.
        let mut angle = k as f32 * 2.399;
        loop {
            let p = pos2(angle.cos() * distance, angle.sin() * distance);
            let free = at
                .iter()
                .all(|(o, c)| (*c - p).length() > radius(&districts[*o]) + r + GAP / 2.0);
            if free {
                at.insert(*d, p);
                break;
            }
            angle += 0.15;
            distance += 2.0;
        }
    }
    move_to(districts, &at);
    Some(hub)
}

/// Puts the tests below what the lens arranged and the outskirts to its
/// right.
pub(crate) fn aside(districts: &mut [District]) {
    let core = core(districts);
    let mut extent = core
        .iter()
        .fold(Rect::NOTHING, |e, d| e.union(districts[*d].area));
    if !extent.is_positive() {
        extent = Rect::from_center_size(Pos2::ZERO, Vec2::ZERO);
    }
    let mut at = HashMap::new();
    for (d, district) in districts.iter().enumerate() {
        let r = radius(district);
        let centre = match district.kind {
            DistrictKind::Tests => pos2(extent.center().x, extent.max.y + r + GAP * 2.0),
            DistrictKind::Outskirts => pos2(extent.max.x + r + GAP * 2.0, extent.center().y),
            _ => continue,
        };
        extent = extent.union(Rect::from_center_size(centre, Vec2::splat(2.0 * r)));
        at.insert(d, centre);
    }
    move_to(districts, &at);
}

/// Pushes the circles of `ids` apart until none overlap.
fn separate(districts: &[District], ids: &[usize], at: &mut HashMap<usize, Pos2>) {
    for _ in 0..200 {
        let mut moved = false;
        for (i, a) in ids.iter().enumerate() {
            for b in &ids[i + 1..] {
                let delta = at[b] - at[a];
                let distance = delta.length();
                let need = radius(&districts[*a]) + radius(&districts[*b]) + GAP / 2.0;
                if distance < need {
                    let dir = if distance < 0.01 {
                        Vec2::X
                    } else {
                        delta / distance
                    };
                    let shift = dir * (need - distance) / 2.0;
                    *at.get_mut(a).expect("placed") -= shift;
                    *at.get_mut(b).expect("placed") += shift;
                    moved = true;
                }
            }
        }
        if !moved {
            break;
        }
    }
}

fn move_to(districts: &mut [District], at: &HashMap<usize, Pos2>) {
    for (d, centre) in at {
        let offset = *centre - districts[*d].area.center();
        layout::translate(&mut districts[*d], offset);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::lens::cluster::link;

    fn district(n: usize, r: f32) -> District {
        District {
            name: format!("d{n}"),
            path: PathBuf::from(format!("/p/d{n}")),
            sub_districts: Vec::new(),
            nodes: Vec::new(),
            area: Rect::from_center_size(pos2(n as f32 * 10.0, 0.0), Vec2::splat(2.0 * r)),
            kind: DistrictKind::Cluster,
            elevation: 0.0,
        }
    }

    fn apart(districts: &[District]) -> bool {
        districts.iter().enumerate().all(|(i, a)| {
            districts[i + 1..]
                .iter()
                .all(|b| (a.area.center() - b.area.center()).length() >= radius(a) + radius(b))
        })
    }

    #[test]
    fn a_network_pulls_linked_districts_together_and_overlaps_none() {
        let mut districts: Vec<District> = (0..5).map(|n| district(n, 30.0)).collect();
        let mut weights = Weights::new();
        link(&mut weights, 0, 1, 10.0);

        network(&mut districts, &weights);

        assert!(apart(&districts));
        let distance =
            |a: usize, b: usize| (districts[a].area.center() - districts[b].area.center()).length();
        assert!(
            distance(0, 1) < distance(0, 4),
            "{} {}",
            distance(0, 1),
            distance(0, 4)
        );
    }

    #[test]
    fn an_orbit_centres_the_most_linked_district_and_brings_its_partners_near() {
        let mut districts: Vec<District> = (0..4).map(|n| district(n, 30.0)).collect();
        let mut weights = Weights::new();
        link(&mut weights, 2, 0, 5.0);
        link(&mut weights, 2, 1, 1.0);
        link(&mut weights, 2, 3, 0.1);

        let hub = orbit(&mut districts, &weights);

        assert_eq!(hub, Some(2));
        assert!(apart(&districts));
        let distance =
            |d: usize| (districts[d].area.center() - districts[2].area.center()).length();
        assert!(distance(0) < distance(3));
    }
}
