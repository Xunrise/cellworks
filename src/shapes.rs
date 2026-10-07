// Machine outlines by file type. Corner features are capped in screen pixels so they stay subtle when zoomed in.
use eframe::egui::{Pos2, Rect, pos2};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FileShape {
    Code,
    Test,
    Doc,
    Config,
    Script,
    Style,
}

pub const ALL: [(FileShape, &str); 6] = [
    (FileShape::Code, "code: slightly rounded"),
    (FileShape::Test, "tests: rounded"),
    (FileShape::Doc, "docs: folded page"),
    (FileShape::Config, "config and data: chamfered"),
    (FileShape::Script, "scripts: arrow"),
    (FileShape::Style, "styles and markup: pill"),
];

pub fn classify(rel: &str) -> FileShape {
    let name = rel.rsplit('/').next().unwrap_or(rel).to_lowercase();
    let ext = name.rsplit_once('.').map_or("", |x| x.1);
    if name.contains(".spec.") || name.contains(".test.") || name.starts_with("test_") || name.contains("_test.") || rel.split('/').any(|d| d == "tests" || d == "__tests__") {
        return FileShape::Test;
    }
    match ext {
        "md" | "txt" | "rst" | "adoc" | "org" => FileShape::Doc,
        "json" | "yaml" | "yml" | "toml" | "ini" | "env" | "conf" | "cfg" | "xml" | "csv" | "tsv" | "plist" | "neon" | "dist" => FileShape::Config,
        "sh" | "bash" | "zsh" | "fish" => FileShape::Script,
        "css" | "scss" | "less" | "html" | "svg" => FileShape::Style,
        _ if name.starts_with('.') || name == "dockerfile" || name == "makefile" || name == "brewfile" => FileShape::Config,
        _ => FileShape::Code,
    }
}

/// Convex outline of `r` (screen space) for a shape, clockwise from the top-left.
pub fn outline(shape: FileShape, r: Rect) -> Vec<Pos2> {
    let m = r.width().min(r.height());
    let arc = |c: Pos2, rad: f32, from: f32, out: &mut Vec<Pos2>| {
        for k in 0..=6 {
            let a = from + k as f32 / 6.0 * std::f32::consts::FRAC_PI_2;
            out.push(pos2(c.x + a.cos() * rad, c.y + a.sin() * rad));
        }
    };
    let rounded = |rad: f32| {
        let mut p = vec![];
        use std::f32::consts::PI;
        arc(pos2(r.min.x + rad, r.min.y + rad), rad, PI, &mut p);
        arc(pos2(r.max.x - rad, r.min.y + rad), rad, -PI / 2.0, &mut p);
        arc(pos2(r.max.x - rad, r.max.y - rad), rad, 0.0, &mut p);
        arc(pos2(r.min.x + rad, r.max.y - rad), rad, PI / 2.0, &mut p);
        p
    };
    match shape {
        FileShape::Code => rounded((m * 0.08).min(7.0)),
        FileShape::Test => rounded((m * 0.25).min(18.0)),
        FileShape::Style => rounded((m * 0.5).min(22.0)),
        FileShape::Doc => {
            let c = (m * 0.18).min(18.0);
            vec![r.min, pos2(r.max.x - c, r.min.y), pos2(r.max.x, r.min.y + c), r.max, pos2(r.min.x, r.max.y)]
        }
        FileShape::Config => {
            let c = (m * 0.12).min(12.0);
            vec![
                pos2(r.min.x + c, r.min.y), pos2(r.max.x - c, r.min.y), pos2(r.max.x, r.min.y + c), pos2(r.max.x, r.max.y - c),
                pos2(r.max.x - c, r.max.y), pos2(r.min.x + c, r.max.y), pos2(r.min.x, r.max.y - c), pos2(r.min.x, r.min.y + c),
            ]
        }
        FileShape::Script => {
            let c = (r.width() * 0.08).min(16.0);
            vec![r.min, pos2(r.max.x - c, r.min.y), pos2(r.max.x, r.center().y), pos2(r.max.x - c, r.max.y), pos2(r.min.x, r.max.y), pos2(r.min.x + c, r.center().y)]
        }
    }
}

/// The part of a convex polygon above `y` (Sutherland-Hodgman against one horizontal edge).
pub fn clip_above(poly: &[Pos2], y: f32) -> Vec<Pos2> {
    let mut out = vec![];
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let (ina, inb) = (a.y <= y, b.y <= y);
        if ina {
            out.push(a);
        }
        if ina != inb {
            let t = (y - a.y) / (b.y - a.y);
            out.push(pos2(a.x + (b.x - a.x) * t, y));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_and_clips() {
        assert_eq!(classify("app/src/sca.spec.ts"), FileShape::Test);
        assert_eq!(classify("docs/README.md"), FileShape::Doc);
        assert_eq!(classify("package.json"), FileShape::Config);
        assert_eq!(classify(".gitignore"), FileShape::Config);
        assert_eq!(classify("spirit/pray.sh"), FileShape::Script);
        assert_eq!(classify("src/main.rs"), FileShape::Code);
        let r = Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 80.0));
        let head = clip_above(&outline(FileShape::Doc, r), 20.0);
        assert!(head.iter().all(|p| p.y <= 20.0) && head.len() == 5);
        assert!(outline(FileShape::Test, r).iter().all(|p| r.expand(0.01).contains(*p)));
    }
}
