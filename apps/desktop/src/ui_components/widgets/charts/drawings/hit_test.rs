//! Shared screen-space hit-testing backed by `kurbo`. Every drawing tool
//! reduces its geometry to a `kurbo::Shape` (a single `Line`, a `BezPath` of
//! segments, etc.) and calls `hit_shape`; `ParamCurveNearest::nearest` handles
//! the actual point-to-curve distance for lines, Beziers, and arcs
//! identically.

use egui::Pos2;
use kurbo::{ParamCurveNearest, PathSeg, Point, Shape};

/// Sub-pixel accuracy used both for path flattening and for the nearest-point
/// search. Tight enough that drifting off a line by a fraction of a pixel
/// still reads as a miss; loose enough to stay cheap on curves.
const ACCURACY: f64 = 0.25;

pub fn to_kurbo(p: Pos2) -> Point {
    Point::new(p.x as f64, p.y as f64)
}

/// True iff `px` is within `tol` pixels of any segment of `shape`. Works for
/// every `kurbo::Shape` — straight segments, Beziers, arcs, or composite
/// paths — without per-tool hit-test math.
pub fn hit_shape<S: Shape>(shape: &S, px: Pos2, tol: f32) -> bool {
    let target = to_kurbo(px);
    let tol_sq = (tol as f64) * (tol as f64);
    for seg in shape.path_segments(ACCURACY) {
        if nearest_sq(seg, target) <= tol_sq {
            return true;
        }
    }
    false
}

fn nearest_sq(seg: PathSeg, target: Point) -> f64 {
    match seg {
        PathSeg::Line(l) => l.nearest(target, ACCURACY).distance_sq,
        PathSeg::Quad(q) => q.nearest(target, ACCURACY).distance_sq,
        PathSeg::Cubic(c) => c.nearest(target, ACCURACY).distance_sq,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Line;

    fn line(x0: f64, y0: f64, x1: f64, y1: f64) -> Line {
        Line::new(Point::new(x0, y0), Point::new(x1, y1))
    }

    #[test]
    fn hit_shape_on_segment() {
        let l = line(0.0, 0.0, 100.0, 0.0);
        assert!(hit_shape(&l, Pos2::new(50.0, 0.0), 1.0));
    }

    #[test]
    fn hit_shape_within_tolerance() {
        let l = line(0.0, 0.0, 100.0, 0.0);
        assert!(hit_shape(&l, Pos2::new(50.0, 3.5), 4.0));
    }

    #[test]
    fn hit_shape_outside_tolerance() {
        let l = line(0.0, 0.0, 100.0, 0.0);
        assert!(!hit_shape(&l, Pos2::new(50.0, 10.0), 4.0));
    }

    #[test]
    fn hit_shape_past_endpoint_misses() {
        let l = line(0.0, 0.0, 100.0, 0.0);
        assert!(!hit_shape(&l, Pos2::new(150.0, 0.0), 4.0));
    }
}
