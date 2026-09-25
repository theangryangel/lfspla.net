//! Writing one fitted outline out as an SVG document.

use std::fmt::Write as _;

use crate::{
    geometry::{Frame, Tick},
    outline::Point,
};

/// How far each marker reaches past the edge of the track ribbon, in drawing
/// units. Held as an overhang rather than as a share of the line's width so
/// that a marker stays a couple of units wider than the line it crosses
/// whatever `--stroke-width` that line is given: enough to read as a line
/// across the track, not enough to read as a cross laid on top of it.
const START_FINISH_OVERHANG: f32 = 2.0;
const SPLIT_OVERHANG: f32 = 1.5;
/// A cut-out only has to break the line, so it reaches barely past it.
const CUT_OUT_OVERHANG: f32 = 1.0;

/// Shares of the centre line's width that the markers are drawn at, so their
/// weight against the line stays the same at any `--stroke-width`.
const START_FINISH_WIDTH: f32 = 0.44;
const SPLIT_WIDTH: f32 = 0.36;
/// How far a cut-out extends past the marker it sits behind, on each side.
/// Drawn to the full length of that marker it would instead leave stubs of
/// card colour sticking out into whatever is behind the drawing.
const CUT_OUT: f32 = 0.22;

/// The colour of each part, as the site's own theme token with a literal
/// fallback for a drawing that is not inside the site.
///
/// See [`Drawing::to_svg`] for why it is written twice over.
const TRACK_COLOUR: &str = "var(--muted-foreground, oklch(0.556 0 0))";
const CUT_OUT_COLOUR: &str = "var(--muted, oklch(0.97 0 0))";
const SPLIT_COLOUR: &str = "var(--foreground, oklch(0.145 0 0))";
const START_FINISH_COLOUR: &str = "var(--destructive, oklch(0.577 0.245 27.325))";
/// The same four tokens as the site's dark theme sets them.
const DARK_THEME: &str = concat!(
    "--muted-foreground:oklch(0.708 0 0);",
    "--muted:oklch(0.269 0 0);",
    "--foreground:oklch(0.985 0 0);",
    "--destructive:oklch(0.704 0.191 22.216)",
);

/// Everything one document draws.
pub(crate) struct Drawing<'a> {
    /// The track code, used as the accessible name.
    pub(crate) name: &'a str,
    pub(crate) frame: Frame,
    /// Width of the centre line; the markers are sized from it.
    pub(crate) stroke_width: f32,
    /// The centre line, already fitted to the frame and simplified.
    pub(crate) centre_line: &'a [Point],
    pub(crate) closed: bool,
    pub(crate) start_finish: Option<Tick>,
    pub(crate) splits: &'a [Tick],
}

impl Drawing<'_> {
    /// Renders a standalone SVG with theme fallbacks.
    pub(crate) fn to_svg(&self) -> String {
        let width = self.stroke_width;
        let mut svg = String::with_capacity(2048);
        let _ = write!(
            svg,
            concat!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {width} {height}\" ",
                "width=\"{width}\" height=\"{height}\" fill=\"none\" role=\"img\" aria-label=\"{name}\">\n",
                "<title>{name}</title>\n",
                "<style>@media(prefers-color-scheme:dark){{svg:root{{{dark}}}}}</style>\n",
                "<path class=\"track\" stroke=\"{track}\" stroke-width=\"{stroke}\" ",
                "stroke-linecap=\"round\" stroke-linejoin=\"round\" d=\"{data}\"/>\n",
            ),
            width = number(self.frame.width),
            height = number(self.frame.height),
            name = self.name,
            dark = DARK_THEME,
            track = TRACK_COLOUR,
            stroke = number(width),
            data = path_data(self.centre_line, self.closed),
        );

        // Every cut-out is drawn before every marker, so that two timing
        // points close together cannot punch holes in each other.
        for (tick, marker_width) in self.markers() {
            self.line(
                &mut svg,
                "cut-out",
                tick.resized(reach(width, CUT_OUT_OVERHANG)),
                CUT_OUT_COLOUR,
                marker_width + width * CUT_OUT * 2.0,
            );
        }
        for split in self.splits {
            self.line(&mut svg, "split", *split, SPLIT_COLOUR, width * SPLIT_WIDTH);
        }
        if let Some(start_finish) = self.start_finish {
            self.line(
                &mut svg,
                "start-finish",
                start_finish,
                START_FINISH_COLOUR,
                width * START_FINISH_WIDTH,
            );
        }
        svg.push_str("</svg>\n");
        svg
    }

    /// Every marker, with the width it is drawn at.
    fn markers(&self) -> impl Iterator<Item = (Tick, f32)> {
        let width = self.stroke_width;
        self.splits
            .iter()
            .map(move |split| (*split, width * SPLIT_WIDTH))
            .chain(
                self.start_finish
                    .map(|start_finish| (start_finish, width * START_FINISH_WIDTH)),
            )
    }

    /// Writes one marker line.
    fn line(&self, svg: &mut String, class: &str, tick: Tick, colour: &str, width: f32) {
        let _ = write!(
            svg,
            concat!(
                "<path class=\"{class}\" stroke=\"{colour}\" stroke-width=\"{width}\" ",
                "stroke-linecap=\"round\" d=\"M{x1} {y1}L{x2} {y2}\"/>\n"
            ),
            class = class,
            colour = colour,
            width = number(width),
            x1 = number(tick.from.x),
            y1 = number(tick.from.y),
            x2 = number(tick.to.x),
            y2 = number(tick.to.y),
        );
    }

    /// Half the length of the start/finish marker, in drawing units.
    pub(crate) fn start_finish_reach(stroke_width: f32) -> f32 {
        reach(stroke_width, START_FINISH_OVERHANG)
    }

    /// Half the length of a split marker, in drawing units.
    pub(crate) fn split_reach(stroke_width: f32) -> f32 {
        reach(stroke_width, SPLIT_OVERHANG)
    }
}

/// Half the length of a line that crosses a ribbon `stroke_width` wide and
/// overhangs it by `overhang` on each side.
fn reach(stroke_width: f32, overhang: f32) -> f32 {
    stroke_width / 2.0 + overhang
}

/// A closed or open curve through every point.
///
/// The points are joined by Catmull-Rom splines written as cubic Beziers,
/// which pass exactly through each point rather than near it. Straight
/// segments would show the simplification as facets on long corners; the
/// spline reads as a circuit at any size, and a marker still lands square on
/// the line because the curve interpolates its points.
fn path_data(points: &[Point], closed: bool) -> String {
    let mut data = String::with_capacity(points.len() * 24);
    let Some(first) = points.first() else {
        return data;
    };
    let _ = write!(data, "M{} {}", number(first.x), number(first.y));
    if points.len() == 1 {
        return data;
    }
    if points.len() == 2 {
        let _ = write!(data, "L{} {}", number(points[1].x), number(points[1].y));
        return data;
    }
    let last = points.len() - 1;
    let segments = if closed { points.len() } else { last };
    for index in 0..segments {
        let (before, start, end, after) = if closed {
            (
                points[(index + last) % points.len()],
                points[index],
                points[(index + 1) % points.len()],
                points[(index + 2) % points.len()],
            )
        } else {
            (
                points[index.saturating_sub(1)],
                points[index],
                points[index + 1],
                points[(index + 2).min(last)],
            )
        };
        let control_start = Point {
            x: start.x + (end.x - before.x) / 6.0,
            y: start.y + (end.y - before.y) / 6.0,
        };
        let control_end = Point {
            x: end.x - (after.x - start.x) / 6.0,
            y: end.y - (after.y - start.y) / 6.0,
        };
        let _ = write!(
            data,
            "C{} {} {} {} {} {}",
            number(control_start.x),
            number(control_start.y),
            number(control_end.x),
            number(control_end.y),
            number(end.x),
            number(end.y),
        );
    }
    if closed {
        data.push('Z');
    }
    data
}

/// One coordinate, at the finest precision the drawing can show.
///
/// A tenth of a unit in a 160 unit frame is well under a pixel at any size
/// these are used at, and rounding there keeps the files small and their
/// diffs readable when a track is regenerated.
fn number(value: f32) -> String {
    let rounded = (value * 10.0).round() / 10.0;
    // Avoid emitting "-0", which is what a rounded negative zero formats as.
    let rounded = if rounded == 0.0 { 0.0 } else { rounded };
    let mut text = format!("{rounded:.1}");
    if let Some(trimmed) = text.strip_suffix(".0") {
        text = trimmed.to_owned();
    }
    text
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

    fn tick(x: f32, y: f32) -> Tick {
        Tick {
            from: point(x, y - 5.0),
            to: point(x, y + 5.0),
        }
    }

    fn drawing() -> String {
        Drawing {
            name: "BL1",
            frame: FRAME,
            stroke_width: 7.0,
            centre_line: &[point(8.0, 8.0), point(152.0, 8.0), point(80.0, 92.0)],
            closed: true,
            start_finish: Some(tick(8.0, 8.0)),
            splits: &[tick(152.0, 8.0), tick(80.0, 92.0)],
        }
        .to_svg()
    }

    #[test]
    fn formats_numbers_compactly() {
        assert_eq!(number(12.0), "12");
        assert_eq!(number(12.04), "12");
        assert_eq!(number(12.06), "12.1");
        assert_eq!(number(-0.01), "0");
        assert_eq!(number(-3.25), "-3.3");
    }

    #[test]
    fn draws_a_curve_through_every_point() {
        let data = path_data(
            &[point(0.0, 0.0), point(10.0, 10.0), point(20.0, 0.0)],
            false,
        );
        assert!(data.starts_with("M0 0"), "{data}");
        // Two segments for three points, and every one ends on a real point.
        assert_eq!(data.matches('C').count(), 2);
        assert!(data.ends_with("20 0"), "{data}");
        assert!(!data.contains('Z'), "an open path must not close");
    }

    #[test]
    fn closes_a_loop_back_to_its_first_point() {
        let square = [
            point(0.0, 0.0),
            point(10.0, 0.0),
            point(10.0, 10.0),
            point(0.0, 10.0),
        ];
        let data = path_data(&square, true);
        assert_eq!(data.matches('C').count(), 4, "one segment per side: {data}");
        assert!(data.ends_with("C-1.7 8.3 -1.7 1.7 0 0Z"), "{data}");
    }

    #[test]
    fn names_every_colour_as_a_token_and_a_literal() {
        let svg = drawing();
        assert!(svg.contains("stroke=\"var(--muted-foreground, oklch(0.556 0 0))\""));
        assert!(svg.contains("stroke=\"var(--destructive, oklch(0.577 0.245 27.325))\""));
        assert!(
            svg.contains("@media(prefers-color-scheme:dark){svg:root{--muted-foreground:"),
            "a standalone file resolves its own dark palette"
        );
        assert!(
            !svg.contains(" :root{") && !svg.contains(">:root{"),
            "the override must not match a host page's root: {svg}"
        );
    }

    #[test]
    fn cuts_every_marker_out_of_the_line_before_drawing_any() {
        let svg = drawing();
        let cut_outs = svg.match_indices("class=\"cut-out\"").count();
        assert_eq!(cut_outs, 3, "one per marker");
        let last_cut_out = svg.rfind("class=\"cut-out\"").expect("a cut-out was drawn");
        let first_marker = svg.find("class=\"split\"").expect("a split was drawn");
        assert!(
            last_cut_out < first_marker,
            "a cut-out must not land on top of another marker"
        );
        assert!(
            svg.find("class=\"split\"") < svg.find("class=\"start-finish\""),
            "the start line is the topmost marker"
        );
    }

    /// The `stroke-width` of the first path carrying `class`.
    fn stroke_width_of(svg: &str, class: &str) -> f32 {
        let at = svg
            .find(&format!("class=\"{class}\""))
            .expect("the class was drawn");
        let tail = &svg[at..];
        let opens = tail.find("stroke-width=\"").expect("a width was set") + 14;
        let closes = tail[opens..].find('"').expect("the width is quoted");
        tail[opens..opens + closes].parse().expect("a number")
    }

    #[test]
    fn a_marker_is_thinner_than_the_line_and_cut_out_of_it() {
        let svg = drawing();
        let track = stroke_width_of(&svg, "track");
        let start_finish = stroke_width_of(&svg, "start-finish");
        let split = stroke_width_of(&svg, "split");
        assert!(
            split < start_finish && start_finish < track,
            "a marker reads against the line by being thinner than it: \
             track {track}, start/finish {start_finish}, split {split}"
        );
        assert!(
            stroke_width_of(&svg, "cut-out") > start_finish,
            "and by having the line cut away around it"
        );
    }

    #[test]
    fn a_marker_crosses_the_line_by_a_couple_of_units() {
        // Wide enough to read as a line across the track, and no wider: the
        // overhang does not grow with the width of the line it crosses.
        for width in [4.0, 7.0, 12.0] {
            let over = Drawing::start_finish_reach(width) * 2.0 - width;
            assert!((2.0..=4.0).contains(&over), "{width} wide: {over} over");
            let over = Drawing::split_reach(width) * 2.0 - width;
            assert!((2.0..=4.0).contains(&over), "{width} wide: {over} over");
        }
    }
}
