//! The shape of one track, read out of a Live for Speed path file.

use insim_pth::{
    Pth,
    pth::{lfspth, srpath},
};

/// Raw path coordinates are stored as fixed-point integers with this divisor.
const SCALE: f32 = 65536.0;

/// One point of a track outline, in the source file's world units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Point {
    pub(crate) x: f32,
    pub(crate) y: f32,
}

/// The centre line of one track, with the timing points along it.
///
/// Everything downstream works from this: nothing else in the tool needs to
/// know which of the two path formats a file was written in.
#[derive(Debug)]
pub(crate) struct Outline {
    /// The centre of every path node, in order of travel.
    pub(crate) points: Vec<Point>,
    /// Whether the last point joins back to the first.
    pub(crate) closed: bool,
    /// Index into `points` of the start/finish line, when the file names one.
    pub(crate) start_finish: Option<usize>,
    /// Indices into `points` of the intermediate splits, in order.
    pub(crate) splits: Vec<usize>,
}

impl Outline {
    /// Reads the centre line and timing points out of a parsed path file.
    ///
    /// The `y` axis is negated here, once, so that every later stage works in
    /// SVG's coordinate system: the path files put `+y` north, SVG puts it
    /// down the screen, and the in-game map is the former.
    pub(crate) fn read(pth: &Pth) -> Self {
        let points = pth
            .iter_nodes()
            .map(|node| {
                let centre = node.get_center(Some(SCALE));
                Point {
                    x: centre.x,
                    y: -centre.y,
                }
            })
            .collect::<Vec<_>>();
        let (closed, start_finish, splits) = match pth {
            Pth::LfsPth0(pth) => Self::lfspth_timing(pth, points.len()),
            Pth::SrPath0(pth) => Self::srpath_timing(pth, points.len()),
        };
        Self {
            points,
            closed,
            start_finish,
            splits,
        }
    }

    /// The older format names only the finish line, and only describes circuits.
    fn lfspth_timing(pth: &lfspth::v0::LfsPth, len: usize) -> (bool, Option<usize>, Vec<usize>) {
        let start_finish = usize::try_from(pth.finish_line_node)
            .ok()
            .filter(|node| *node < len);
        (true, start_finish, Vec::new())
    }

    /// The 0.8 format names all four timing points, and says whether it loops.
    ///
    /// `split0` is the finish line. The remaining three are optional, and a
    /// track with fewer than three splits writes `u32::MAX` in each field it
    /// does not use, which the bound on the node count already rejects. Zero
    /// is rejected as well: no revision is known to write it, but it cannot be
    /// told apart from a field that was never set, and drawing a marker that
    /// does not exist is the worse of the two mistakes.
    fn srpath_timing(pth: &srpath::v0::SrPth, len: usize) -> (bool, Option<usize>, Vec<usize>) {
        let start_finish = Some(pth.split0_node).filter(|node| *node < len);
        let splits = [pth.split1_node, pth.split2_node, pth.split3_node]
            .into_iter()
            .filter(|node| *node != 0 && *node < len && Some(*node) != start_finish)
            .collect();
        (
            pth.flags.contains(srpath::v0::SrPathFlags::LOOP),
            start_finish,
            splits,
        )
    }

    /// Every timing point, as indices into `points`, ordered and deduplicated.
    ///
    /// Simplification is anchored on these so that a marker always lands on a
    /// point the drawn curve actually passes through.
    pub(crate) fn timing_points(&self) -> Vec<usize> {
        let mut anchors: Vec<usize> = self
            .start_finish
            .into_iter()
            .chain(self.splits.iter().copied())
            .collect();
        anchors.sort_unstable();
        anchors.dedup();
        anchors
    }
}

#[cfg(test)]
mod tests {
    use insim_pth::node::{Node, NodeCoordinate};

    use super::*;

    fn node(x: i32, y: i32) -> Node {
        Node {
            center: NodeCoordinate { x, y, z: 0 },
            ..Node::default()
        }
    }

    #[test]
    fn flips_the_y_axis_into_svg_orientation() {
        let pth = Pth::LfsPth0(lfspth::v0::LfsPth {
            revision: 0,
            finish_line_node: 1,
            nodes: vec![node(0, 0), node(SCALE as i32, SCALE as i32 * 2)],
        });
        let outline = Outline::read(&pth);
        assert_eq!(outline.points[1], Point { x: 1.0, y: -2.0 });
        assert_eq!(outline.start_finish, Some(1));
        assert!(outline.closed, "the older format only describes circuits");
    }

    #[test]
    fn ignores_timing_points_the_file_does_not_have() {
        let pth = Pth::SrPath0(srpath::v0::SrPth {
            revision: 0,
            flags: srpath::v0::SrPathFlags::LOOP,
            mini_rev: 0,
            split0_node: 0,
            // One real split, one field left at zero, and one holding the
            // sentinel a real file writes for a split the track does not have.
            split1_node: 1,
            split2_node: 0,
            split3_node: u32::MAX as usize,
            pole_position: srpath::v0::SrPolePosition::default(),
            main_nodes: vec![
                srpath::v0::SrNode {
                    node: node(0, 0),
                    ..srpath::v0::SrNode::default()
                },
                srpath::v0::SrNode {
                    node: node(1, 1),
                    ..srpath::v0::SrNode::default()
                },
            ],
            pit0_nodes: Vec::new(),
            pit1_nodes: Vec::new(),
        });
        let outline = Outline::read(&pth);
        assert_eq!(outline.start_finish, Some(0));
        assert_eq!(outline.splits, vec![1]);
        assert_eq!(outline.timing_points(), vec![0, 1]);
    }

    #[test]
    fn reads_an_unlooped_path_as_open() {
        let pth = Pth::SrPath0(srpath::v0::SrPth {
            revision: 0,
            flags: srpath::v0::SrPathFlags::ROUTE,
            mini_rev: 0,
            split0_node: 0,
            split1_node: 0,
            split2_node: 0,
            split3_node: 0,
            pole_position: srpath::v0::SrPolePosition::default(),
            main_nodes: vec![srpath::v0::SrNode::default()],
            pit0_nodes: Vec::new(),
            pit1_nodes: Vec::new(),
        });
        assert!(!Outline::read(&pth).closed);
    }
}
