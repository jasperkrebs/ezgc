//! Toolpaths: the nozzle's path in print order, as data.
//!
//! A project kind builds a toolpath and nothing else: it never writes G-code itself. The one
//! G-code writer ([`crate::gcode`]) turns any toolpath into text for any printer, and the 3D
//! preview draws the very same moves, so what you see is what prints. (In EasyGCode every
//! generator wrote its own G-code: the bead maths, E units, F changes and start/end blocks were
//! repeated per project.)
//!
//! Every move ends at a point in the part's own coordinates (the printer settings place the part
//! on the bed). An extruding move carries its bead (cross-section in mm²), how much of a full
//! bead it lays (ramps go from 0 to 1) and its feed; its volume is bead × length × flow, worked
//! out by the writer from the move's actual length. Events (comments, fan, dwells) sit between
//! moves.

use crate::geom::{Bounds, P3, dist3};
use serde::Serialize;

/// Above this a toolpath gets too heavy to compute and draw interactively.
pub const MAX_MOVES: usize = 2_000_000;

/// Moves shorter than this are skipped (as EasyGCode does): no G-code line, no time.
pub const MIN_MOVE: f64 = 1e-7;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Move {
    /// Where the move ends, in part coordinates (mm).
    pub to: P3,
    /// A travel moves without extruding, at the printer's travel feed.
    pub travel: bool,
    /// The bead's cross-section in mm² (see [`crate::bead::bead_area`]).
    pub area: f64,
    /// How much of a full bead the move lays: 0..1 on ramps, 1 normally.
    pub flow: f64,
    /// Feed in mm/min.
    pub feed: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A comment line in the G-code.
    Comment(String),
    /// The part fan comes on at the printer's fan setting (usually after the first layer).
    FanOn,
    /// The part fan at a level, 0–100 %.
    Fan(f64),
    /// Wait, in seconds.
    Dwell(f64),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Toolpath {
    pub moves: Vec<Move>,
    /// Events in order, each with the number of moves before it.
    pub events: Vec<(usize, Event)>,
    /// Layers (vase mode: revolutions; may end part way through one), for the stats.
    pub layers: f64,
}

impl Toolpath {
    pub fn new() -> Toolpath {
        Toolpath::default()
    }

    pub fn travel(&mut self, to: P3) {
        self.moves.push(Move {
            to,
            travel: true,
            area: 0.0,
            flow: 0.0,
            feed: 0.0,
        });
    }

    pub fn extrude(&mut self, to: P3, area: f64, flow: f64, feed: f64) {
        self.moves.push(Move {
            to,
            travel: false,
            area,
            flow,
            feed,
        });
    }

    pub fn event(&mut self, e: Event) {
        self.events.push((self.moves.len(), e));
    }

    pub fn comment(&mut self, text: impl Into<String>) {
        self.event(Event::Comment(text.into()));
    }

    /// Where the nozzle is now (after the last move).
    pub fn last(&self) -> Option<P3> {
        self.moves.last().map(|m| m.to)
    }

    /// Print statistics. `travel_feed` (mm/min) times the travels.
    pub fn stats(&self, travel_feed: f64) -> Stats {
        let mut s = Stats {
            layers: self.layers,
            ..Stats::default()
        };
        let mut bounds = Bounds::empty();
        let mut prev: Option<P3> = None;
        let (mut feed_min, mut feed_max) = (f64::INFINITY, 0.0f64);
        let (mut flow_min, mut flow_max) = (f64::INFINITY, 0.0f64);
        for m in &self.moves {
            bounds.add(m.to);
            let Some(from) = prev else {
                // The first move: always written, as a travel to the start.
                s.moves += 1;
                prev = Some(m.to);
                continue;
            };
            prev = Some(m.to);
            let d = dist3(from, m.to);
            if d < MIN_MOVE {
                continue;
            }
            s.moves += 1;
            if m.travel {
                if travel_feed > 0.0 {
                    s.time_min += d / travel_feed;
                }
                continue;
            }
            s.length_mm += d;
            s.volume_mm3 += m.area * d * m.flow;
            if m.feed > 0.0 {
                s.time_min += d / m.feed;
            }
            feed_min = feed_min.min(m.feed);
            feed_max = feed_max.max(m.feed);
            // The flow the printer has to melt, on moves that lay a full bead.
            if m.flow >= 0.999 {
                let q = m.area * m.flow * m.feed / 60.0;
                flow_min = flow_min.min(q);
                flow_max = flow_max.max(q);
            }
        }
        if !bounds.is_empty() {
            s.bounds = Some(bounds);
        }
        if feed_max > 0.0 {
            s.feed = Some([feed_min, feed_max]);
        }
        if flow_max > 0.0 {
            s.flow = Some([flow_min, flow_max]);
        }
        s
    }
}

/// What a toolpath prints, for the app.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    /// G-code moves (travels and extrusions).
    pub moves: usize,
    pub layers: f64,
    /// Length of the extruding moves, mm.
    pub length_mm: f64,
    pub volume_mm3: f64,
    pub time_min: f64,
    /// Feed range of the extruding moves, mm/min.
    pub feed: Option<[f64; 2]>,
    /// Volumetric flow range of full-bead moves, mm³/s.
    pub flow: Option<[f64; 2]>,
    /// In part coordinates.
    pub bounds: Option<Bounds>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_add_up() {
        let mut p = Toolpath::new();
        p.travel([0.0, 0.0, 0.2]);
        p.extrude([10.0, 0.0, 0.2], 0.5, 1.0, 600.0);
        p.extrude([10.0, 0.0, 0.2], 0.5, 1.0, 600.0); // zero length: skipped
        p.extrude([10.0, 10.0, 0.2], 0.5, 0.5, 1200.0);
        p.travel([0.0, 10.0, 0.2]);
        let s = p.stats(6000.0);
        assert_eq!(s.moves, 4); // as written: the first travel, two extrusions, a travel
        assert!((s.length_mm - 20.0).abs() < 1e-12);
        assert!((s.volume_mm3 - (5.0 + 2.5)).abs() < 1e-12);
        assert!((s.time_min - (10.0 / 600.0 + 10.0 / 1200.0 + 10.0 / 6000.0)).abs() < 1e-12);
        assert_eq!(s.feed, Some([600.0, 1200.0]));
        assert_eq!(s.flow, Some([5.0, 5.0]));
        let b = s.bounds.unwrap();
        assert_eq!(b.max, [10.0, 10.0, 0.2]);
    }
}
