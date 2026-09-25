//! Renders Live for Speed path files as track outline SVGs.
//!
//! One SVG per path file, named after it, sized for the thumbnails in the
//! chart picker rather than for a wall map: the centre line only, with the
//! start/finish line and the splits marked on it.
//!
//! ```text
//! lfsplanet_track_gen --output assets/tracks/ ~/LFS/data/pth/*.pth
//! ```
//!
//! Based on the `pth2svg` example from insim.rs.

mod geometry;
mod outline;
mod render;

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use clap::Parser;
use insim_pth::Pth;

use crate::{geometry::Frame, outline::Outline, render::Drawing};

/// How much extent, in drawing units, a path needs in both axes to be drawn.
///
/// Small enough that no circuit comes near it, large enough to recognise a
/// path file that holds a straight line instead of a track.
const MINIMUM_EXTENT: f32 = 10.0;

/// Converts Live for Speed path files into track outline SVGs.
#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// Directory to write the SVG files into. Created when it does not exist.
    #[arg(short, long, value_name = "DIR")]
    output: PathBuf,

    /// Width of the drawing, in SVG user units.
    #[arg(long, default_value_t = 160.0)]
    width: f32,

    /// Height of the drawing, in SVG user units.
    #[arg(long, default_value_t = 100.0)]
    height: f32,

    /// Space kept clear on every side of the outline.
    #[arg(long, default_value_t = 8.0)]
    padding: f32,

    /// Width of the centre line. The markers are sized from it.
    #[arg(long, default_value_t = 5.0)]
    stroke_width: f32,

    /// How far a node may sit from a straight line before it is kept.
    ///
    /// In drawing units, so it does not depend on the size of the track. Zero
    /// keeps every node in the file.
    #[arg(long, default_value_t = 0.4)]
    simplify: f32,

    /// The path files to convert.
    #[arg(required = true, value_name = "PTH")]
    paths: Vec<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let frame = Frame {
        width: args.width,
        height: args.height,
        padding: args.padding,
    };
    ensure!(
        frame.width > 0.0 && frame.height > 0.0,
        "The drawing must have a positive width and height"
    );
    ensure!(
        frame.padding >= 0.0 && frame.padding * 2.0 < frame.width.min(frame.height),
        "The padding must leave room to draw in"
    );
    ensure!(
        args.stroke_width > 0.0,
        "The centre line must have a positive width"
    );
    ensure!(
        args.simplify >= 0.0,
        "The simplification tolerance cannot be negative"
    );
    fs::create_dir_all(&args.output)
        .with_context(|| format!("Could not create the output directory {:?}", args.output))?;

    // A run over a whole `data/pth` directory should not be abandoned because
    // one file in it cannot be read, so failures are collected and reported
    // together at the end.
    let mut failures = Vec::new();
    let mut written = 0usize;
    let mut skipped = 0usize;
    for path in &args.paths {
        match convert(path, &args.output, frame, args.stroke_width, args.simplify) {
            Ok(outcome) => {
                println!("{}", outcome.report());
                match outcome {
                    Outcome::Wrote(_) => written += 1,
                    Outcome::Skipped(_) => skipped += 1,
                }
            }
            Err(error) => failures.push(format!("{}: {error:#}", path.display())),
        }
    }

    println!(
        "Wrote {written} of {} into {}{}",
        args.paths.len(),
        args.output.display(),
        if skipped > 0 {
            format!(", skipping {skipped} with no track in them")
        } else {
            String::new()
        }
    );
    if !failures.is_empty() {
        bail!("Could not convert:\n  {}", failures.join("\n  "));
    }
    Ok(())
}

/// What became of one path file.
enum Outcome {
    Wrote(String),
    Skipped(String),
}

impl Outcome {
    /// The one line report of what was drawn, or of why nothing was.
    fn report(&self) -> &str {
        match self {
            Self::Wrote(report) | Self::Skipped(report) => report,
        }
    }
}

/// Converts one path file, unless it holds no track to draw.
fn convert(
    path: &Path,
    output: &Path,
    frame: Frame,
    stroke_width: f32,
    simplify: f32,
) -> Result<Outcome> {
    let name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .context("The file name is not usable as a track code")?;
    let pth = Pth::from_path(path).context("Could not read the path file")?;
    let outline = Outline::read(&pth);
    ensure!(
        outline.points.len() >= 2,
        "The path file describes {} nodes, which is not a track",
        outline.points.len()
    );

    // Markers are measured on the fitted points before simplification, both
    // for the heading they cross and because the nodes they sit on are the
    // anchors simplification is forbidden to drop. So each one lands square on
    // the curve rather than beside it.
    let fitted = geometry::fit(&outline.points, frame);
    if !geometry::has_shape(&fitted, MINIMUM_EXTENT) {
        return Ok(Outcome::Skipped(format!(
            "{name}: skipped, the path is a straight line rather than a track"
        )));
    }
    let start_finish = outline.start_finish.and_then(|node| {
        geometry::tick(
            &fitted,
            node,
            outline.closed,
            Drawing::start_finish_reach(stroke_width),
        )
    });
    let splits: Vec<_> = outline
        .splits
        .iter()
        .filter_map(|node| {
            geometry::tick(
                &fitted,
                *node,
                outline.closed,
                Drawing::split_reach(stroke_width),
            )
        })
        .collect();
    let centre_line =
        geometry::simplify(&fitted, &outline.timing_points(), outline.closed, simplify);

    let svg = Drawing {
        name,
        frame,
        stroke_width,
        centre_line: &centre_line,
        closed: outline.closed,
        start_finish,
        splits: &splits,
    }
    .to_svg();
    let destination = output.join(format!("{name}.svg"));
    fs::write(&destination, svg)
        .with_context(|| format!("Could not write {}", destination.display()))?;

    Ok(Outcome::Wrote(format!(
        "{name}.svg: {} of {} nodes, {}{}",
        centre_line.len(),
        outline.points.len(),
        if outline.closed { "loop" } else { "open" },
        match (start_finish.is_some(), splits.len()) {
            (false, _) => String::new(),
            (true, 0) => ", start/finish".to_owned(),
            (true, count) => format!(", start/finish and {count} split(s)"),
        }
    )))
}
