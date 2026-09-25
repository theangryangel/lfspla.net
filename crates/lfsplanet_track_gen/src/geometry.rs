//! Fitting and simplifying a track outline for a thumbnail-sized frame.

use crate::outline::Point;

/// The box an outline is drawn into, in SVG user units.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Frame {
    pub(crate) width: f32,
    pub(crate) height: f32,
    /// Kept clear on every side, so a wide stroke is not clipped by the edge.
    pub(crate) padding: f32,
}

/// Scales and centres an outline to fill one frame.
///
/// Every track is fitted to the same box rather than drawn to a common world
/// scale. At thumbnail size a shared scale is the worse trade: it would leave
/// Aston Historic legible and South City Sprint 1 a smudge, and the picker is
/// there to tell one shape from another, not to compare their lengths.
///
/// The aspect ratio is preserved, so shapes are never stretched to fit.
pub(crate) fn fit(points: &[Point], frame: Frame) -> Vec<Point> {
    let Some((min, max)) = bounds(points) else {
        return Vec::new();
    };
    let extent = Point {
        x: max.x - min.x,
        y: max.y - min.y,
    };
    let room = Point {
        x: (frame.width - frame.padding * 2.0).max(0.0),
        y: (frame.height - frame.padding * 2.0).max(0.0),
    };
    // A path with no extent in one axis - a straight line - still has a scale
    // in the other, and one with no extent at all is drawn at its centre.
    let scale = match (extent.x > f32::EPSILON, extent.y > f32::EPSILON) {
        (true, true) => (room.x / extent.x).min(room.y / extent.y),
        (true, false) => room.x / extent.x,
        (false, true) => room.y / extent.y,
        (false, false) => 1.0,
    };
    let offset = Point {
        x: (frame.width - extent.x * scale) / 2.0 - min.x * scale,
        y: (frame.height - extent.y * scale) / 2.0 - min.y * scale,
    };
    points
        .iter()
        .map(|point| Point {
            x: point.x * scale + offset.x,
            y: point.y * scale + offset.y,
        })
        .collect()
}

/// A short line drawn across the track, marking one timing point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Tick {
    pub(crate) from: Point,
    pub(crate) to: Point,
}

impl Tick {
    /// The same crossing, re-cut to a different length about its centre.
    pub(crate) fn resized(self, half_length: f32) -> Self {
        let centre = Point {
            x: f32::midpoint(self.from.x, self.to.x),
            y: f32::midpoint(self.from.y, self.to.y),
        };
        let span = Point {
            x: self.to.x - centre.x,
            y: self.to.y - centre.y,
        };
        let length = span.x.hypot(span.y);
        if length <= f32::EPSILON {
            return self;
        }
        let reach = Point {
            x: span.x / length * half_length,
            y: span.y / length * half_length,
        };
        Self {
            from: Point {
                x: centre.x - reach.x,
                y: centre.y - reach.y,
            },
            to: Point {
                x: centre.x + reach.x,
                y: centre.y + reach.y,
            },
        }
    }
}

/// The line of length `half_length * 2` that crosses the path at `index`.
///
/// A timing point is drawn as a line across the track, the way the start line
/// is painted on one, rather than as a dot on the centre line: the crossing
/// tells the reader which way the lap runs through it, and reads at a smaller
/// size than a dot of the same area.
pub(crate) fn tick(points: &[Point], index: usize, closed: bool, half_length: f32) -> Option<Tick> {
    let at = *points.get(index)?;
    let heading = heading(points, index, closed);
    let across = Point {
        x: -heading.y,
        y: heading.x,
    };
    Some(Tick {
        from: Point {
            x: at.x - across.x * half_length,
            y: at.y - across.y * half_length,
        },
        to: Point {
            x: at.x + across.x * half_length,
            y: at.y + across.y * half_length,
        },
    })
}

/// The unit direction of travel at one point, taken from its neighbours.
fn heading(points: &[Point], index: usize, closed: bool) -> Point {
    let last = points.len() - 1;
    let (before, after) = if closed && points.len() > 2 {
        (
            points[(index + last) % points.len()],
            points[(index + 1) % points.len()],
        )
    } else {
        (
            points[index.saturating_sub(1)],
            points[(index + 1).min(last)],
        )
    };
    let run = Point {
        x: after.x - before.x,
        y: after.y - before.y,
    };
    let length = run.x.hypot(run.y);
    if length <= f32::EPSILON {
        // Two coincident neighbours leave nothing to be perpendicular to.
        return Point { x: 1.0, y: 0.0 };
    }
    Point {
        x: run.x / length,
        y: run.y / length,
    }
}

/// Whether an outline has enough extent to read as a track.
pub(crate) fn has_shape(points: &[Point], minimum: f32) -> bool {
    let Some((min, max)) = bounds(points) else {
        return false;
    };
    (max.x - min.x).min(max.y - min.y) >= minimum
}

/// Drops the points that carry no shape, keeping every anchor.
///
/// Path files hold a node roughly every 0.2 seconds of driving, which is far
/// more detail than a 160 unit wide drawing can show: Blackwood arrives as 400
/// nodes and leaves as about 60, with no visible difference. `anchors` are
/// indices that must survive, because a timing marker is drawn at that
/// coordinate and has to sit on the curve.
pub(crate) fn simplify(
    points: &[Point],
    anchors: &[usize],
    closed: bool,
    epsilon: f32,
) -> Vec<Point> {
    if points.len() < 3 {
        return points.to_vec();
    }
    if closed {
        simplify_ring(points, anchors, epsilon)
    } else {
        simplify_run(points, anchors, epsilon)
    }
}

/// Simplifies a loop, cut at its anchors and rejoined.
fn simplify_ring(points: &[Point], anchors: &[usize], epsilon: f32) -> Vec<Point> {
    // Rotating the loop to begin at an anchor turns the wrap-around into an
    // ordinary run, so the seam gets simplified like everything else instead
    // of being pinned to whichever node the file happened to start at.
    let start = anchors.first().copied().unwrap_or(0);
    let mut ring: Vec<Point> = points[start..]
        .iter()
        .chain(&points[..start])
        .copied()
        .collect();
    ring.push(ring[0]);
    let mut cuts: Vec<usize> = anchors
        .iter()
        .map(|anchor| (anchor + points.len() - start) % points.len())
        .collect();
    cuts.sort_unstable();
    cuts.dedup();
    if cuts.first() != Some(&0) {
        cuts.insert(0, 0);
    }
    cuts.push(points.len());
    let mut simplified = join(&ring, &cuts, epsilon);
    // The closing point is a repeat of the first, and the path closes itself.
    simplified.pop();
    simplified
}

/// Simplifies an open path, cut at its anchors and rejoined.
fn simplify_run(points: &[Point], anchors: &[usize], epsilon: f32) -> Vec<Point> {
    let last = points.len() - 1;
    let mut cuts: Vec<usize> = std::iter::once(0)
        .chain(
            anchors
                .iter()
                .copied()
                .filter(|cut| *cut > 0 && *cut < last),
        )
        .chain(std::iter::once(last))
        .collect();
    cuts.sort_unstable();
    cuts.dedup();
    join(points, &cuts, epsilon)
}

/// Simplifies each run between consecutive cuts and concatenates them.
fn join(points: &[Point], cuts: &[usize], epsilon: f32) -> Vec<Point> {
    let mut simplified: Vec<Point> = Vec::new();
    for pair in cuts.windows(2) {
        let run = douglas_peucker(&points[pair[0]..=pair[1]], epsilon);
        // Consecutive runs share the cut point between them.
        simplified.extend(if simplified.is_empty() {
            &run[..]
        } else {
            &run[1..]
        });
    }
    simplified
}

/// Ramer-Douglas-Peucker: keeps the points that stray from a straight line.
fn douglas_peucker(points: &[Point], epsilon: f32) -> Vec<Point> {
    let (Some(first), Some(last)) = (points.first(), points.last()) else {
        return points.to_vec();
    };
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut worst = (0usize, 0.0f32);
    for (index, point) in points.iter().enumerate().take(points.len() - 1).skip(1) {
        let distance = distance_from_line(*point, *first, *last);
        if distance > worst.1 {
            worst = (index, distance);
        }
    }
    if worst.1 <= epsilon {
        return vec![*first, *last];
    }
    let mut kept = douglas_peucker(&points[..=worst.0], epsilon);
    kept.pop();
    kept.extend(douglas_peucker(&points[worst.0..], epsilon));
    kept
}

/// How far `point` lies from the line through `from` and `to`.
fn distance_from_line(point: Point, from: Point, to: Point) -> f32 {
    let run = Point {
        x: to.x - from.x,
        y: to.y - from.y,
    };
    let length = run.x.mul_add(run.x, run.y * run.y);
    // A run whose ends coincide - a whole loop cut nowhere - has no line to
    // measure against, so measure from the end itself.
    if length <= f32::EPSILON {
        return (point.x - from.x).hypot(point.y - from.y);
    }
    let along = ((point.x - from.x) * run.x + (point.y - from.y) * run.y) / length;
    let nearest = Point {
        x: from.x + run.x * along,
        y: from.y + run.y * along,
    };
    (point.x - nearest.x).hypot(point.y - nearest.y)
}

/// The lowest and highest coordinate in each axis.
fn bounds(points: &[Point]) -> Option<(Point, Point)> {
    let first = points.first()?;
    Some(points.iter().fold((*first, *first), |(min, max), point| {
        (
            Point {
                x: min.x.min(point.x),
                y: min.y.min(point.y),
            },
            Point {
                x: max.x.max(point.x),
                y: max.y.max(point.y),
            },
        )
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: Frame = Frame {
        width: 160.0,
        height: 100.0,
        padding: 8.0,
    };

    fn point(x: f32, y: f32) -> Point {
        Point { x, y }
    }

    #[test]
    fn crosses_the_track_at_a_right_angle_to_travel() {
        let straight = [point(0.0, 5.0), point(10.0, 5.0), point(20.0, 5.0)];
        let tick = tick(&straight, 1, false, 4.0).expect("the index is in range");
        assert_eq!(tick.from, point(10.0, 1.0));
        assert_eq!(tick.to, point(10.0, 9.0));
    }

    #[test]
    fn crosses_a_loop_at_its_seam() {
        // The first point of a loop takes its heading from the last point, not
        // from a clamped neighbour, so a start line that sits on the seam is
        // square to the track like any other marker.
        let square = [
            point(0.0, 0.0),
            point(10.0, 0.0),
            point(10.0, 10.0),
            point(0.0, 10.0),
        ];
        let tick = tick(&square, 0, true, 5.0).expect("the index is in range");
        let along = point(square[1].x - square[3].x, square[1].y - square[3].y);
        let across = point(tick.to.x - tick.from.x, tick.to.y - tick.from.y);
        assert!(
            across.x.mul_add(along.x, across.y * along.y).abs() < 1e-3,
            "the marker crosses the direction of travel: {tick:?}"
        );
        assert!(
            (across.x.hypot(across.y) - 10.0).abs() < 1e-3,
            "and is twice the requested reach long: {tick:?}"
        );
        let centre = point(
            (tick.from.x + tick.to.x) / 2.0,
            (tick.from.y + tick.to.y) / 2.0,
        );
        assert!(centre.x.abs() < 1e-3 && centre.y.abs() < 1e-3, "{centre:?}");
        assert_ne!(
            tick,
            super::tick(&square, 0, false, 5.0).expect("the index is in range"),
            "an open path clamps to its first point instead"
        );
    }

    #[test]
    fn refuses_a_marker_beyond_the_end_of_the_path() {
        assert!(tick(&[point(0.0, 0.0)], 4, false, 4.0).is_none());
    }

    #[test]
    fn fits_a_square_centred_without_stretching_it() {
        let fitted = fit(
            &[
                point(0.0, 0.0),
                point(10.0, 0.0),
                point(10.0, 10.0),
                point(0.0, 10.0),
            ],
            FRAME,
        );
        // The square is limited by the shorter axis, so it is 84 units tall
        // and 84 wide, centred in a 160 by 100 frame.
        assert_eq!(fitted[0], point(38.0, 8.0));
        assert_eq!(fitted[2], point(122.0, 92.0));
    }

    #[test]
    fn tells_a_circuit_from_a_straight_line() {
        let circuit: Vec<Point> = (0..12u8)
            .map(|step| {
                let angle = f32::from(step) * std::f32::consts::TAU / 12.0;
                point(80.0 + angle.cos() * 60.0, 50.0 + angle.sin() * 40.0)
            })
            .collect();
        assert!(has_shape(&circuit, 10.0));
        assert!(!has_shape(
            &fit(&[point(0.0, 5.0), point(10.0, 5.0)], FRAME),
            10.0
        ));
        assert!(!has_shape(&[], 10.0));
    }

    #[test]
    fn fits_a_straight_line_without_dividing_by_zero() {
        let fitted = fit(&[point(0.0, 5.0), point(10.0, 5.0)], FRAME);
        assert_eq!(fitted, vec![point(8.0, 50.0), point(152.0, 50.0)]);
    }

    #[test]
    fn drops_points_that_sit_on_a_straight_line() {
        let straight = [
            point(0.0, 0.0),
            point(1.0, 0.0),
            point(2.0, 0.0),
            point(3.0, 0.1),
            point(4.0, 0.0),
        ];
        assert_eq!(
            simplify(&straight, &[], false, 0.5),
            vec![point(0.0, 0.0), point(4.0, 0.0)]
        );
    }

    #[test]
    fn keeps_a_point_that_carries_shape() {
        let corner = [point(0.0, 0.0), point(2.0, 4.0), point(4.0, 0.0)];
        assert_eq!(simplify(&corner, &[], false, 0.5), corner.to_vec());
    }

    #[test]
    fn keeps_every_anchor_even_on_a_straight_line() {
        let straight = [
            point(0.0, 0.0),
            point(1.0, 0.0),
            point(2.0, 0.0),
            point(3.0, 0.0),
            point(4.0, 0.0),
        ];
        assert_eq!(
            simplify(&straight, &[2], false, 0.5),
            vec![point(0.0, 0.0), point(2.0, 0.0), point(4.0, 0.0)]
        );
    }

    #[test]
    fn simplifies_a_loop_without_repeating_the_closing_point() {
        // A many-sided polygon around a circle, which is entirely shape and so
        // survives simplification, but must not gain a duplicate first point.
        let circle: Vec<Point> = (0..24u8)
            .map(|step| {
                let angle = f32::from(step) * std::f32::consts::TAU / 24.0;
                point(angle.cos() * 50.0, angle.sin() * 50.0)
            })
            .collect();
        let simplified = simplify(&circle, &[0], true, 0.5);
        assert_eq!(simplified.len(), circle.len());
        assert_ne!(simplified.first(), simplified.last());
    }

    #[test]
    fn a_loop_keeps_its_anchors_and_starts_at_one() {
        let circle: Vec<Point> = (0..40u8)
            .map(|step| {
                let angle = f32::from(step) * std::f32::consts::TAU / 40.0;
                point(angle.cos() * 50.0, angle.sin() * 50.0)
            })
            .collect();
        let simplified = simplify(&circle, &[7, 20], true, 4.0);
        assert_eq!(simplified.first(), Some(&circle[7]));
        assert!(simplified.contains(&circle[20]));
        assert!(
            simplified.len() < circle.len(),
            "a coarse tolerance should still drop points"
        );
    }
}
