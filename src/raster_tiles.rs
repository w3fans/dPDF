//! Device-pixel regions for bounded high-zoom detail. No engine or UI handles.
use crate::document::PagePlacement;
use std::{
    cmp::Ordering,
    collections::BinaryHeap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering as AtomicOrdering},
    },
};

pub const TILE_EDGE: u32 = 510;
pub const MAX_TILE_EDGE: u32 = 512;
pub const DETAIL_BUDGET: usize = 32 * 1024 * 1024;
pub const MAX_REGIONS: usize = 128;
pub const MAX_PENDING_RENDERS: usize = 8;
const MAX_CANVAS: u32 = 131_072;

/// Covers queued requests, active native rendering and replies awaiting the UI.
/// Generation changes cannot forget occupied slots or admit an unbounded tail.
#[derive(Clone, Default)]
pub struct RenderSlots(Arc<AtomicUsize>);
pub struct RenderPermit(Arc<AtomicUsize>);

impl RenderSlots {
    pub fn acquire(&self) -> Option<RenderPermit> {
        self.0
            .fetch_update(AtomicOrdering::AcqRel, AtomicOrdering::Acquire, |n| {
                (n < MAX_PENDING_RENDERS).then_some(n + 1)
            })
            .ok()?;
        Some(RenderPermit(self.0.clone()))
    }
    pub fn in_flight(&self) -> usize {
        self.0.load(AtomicOrdering::Acquire)
    }
}
impl Drop for RenderPermit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, AtomicOrdering::AcqRel);
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct Region {
    pub canvas_height: u32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Region {
    pub fn valid_for(self, canvas_width: u32) -> bool {
        canvas_width > 0
            && canvas_width <= MAX_CANVAS
            && self.canvas_height > 0
            && self.canvas_height <= MAX_CANVAS
            && self.width > 0
            && self.height > 0
            && self.width <= MAX_TILE_EDGE
            && self.height <= MAX_TILE_EDGE
            && self
                .x
                .checked_add(self.width)
                .is_some_and(|v| v <= canvas_width)
            && self
                .y
                .checked_add(self.height)
                .is_some_and(|v| v <= self.canvas_height)
    }

    pub fn bytes(self) -> usize {
        self.width as usize * self.height as usize * 4
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    pub page: usize,
    pub canvas_width: u32,
    pub region: Region,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

struct Candidate {
    priority: (bool, u64, usize, u32, u32),
    placement: Placement,
}
impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}
impl Eq for Candidate {}
impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority.cmp(&other.priority)
    }
}

/// The viewport is in document logical coordinates. Detail images include a
/// one-pixel gutter, rendered from the same full-page device transform, so
/// neighbouring images do not expose independent bitmap filtering edges.
pub fn plan(pages: &[PagePlacement], viewport: (f32, f32, f32, f32), scale: f32) -> Vec<Placement> {
    let (vx, vy, vw, vh) = viewport;
    if ![vx, vy, vw, vh, scale].iter().all(|v| v.is_finite())
        || vw <= 0.0
        || vh <= 0.0
        || scale <= 0.0
    {
        return Vec::new();
    }
    // Budget physical density for unusually large/high-DPI viewports before
    // enumerating cells. This does not change logical page/overlay geometry.
    let density = scale
        .min(4096.0 / vw)
        .min(4096.0 / vh)
        .min((4_000_000.0 / (vw as f64 * vh as f64)).sqrt() as f32);
    let mut candidates: BinaryHeap<Candidate> = BinaryHeap::new();
    for page in pages {
        if ![page.x, page.y, page.width, page.height]
            .iter()
            .all(|v| v.is_finite())
            || page.width <= 0.0
            || page.height <= 0.0
            || page.x >= vx + vw
            || page.y >= vy + vh
            || page.x + page.width <= vx
            || page.y + page.height <= vy
        {
            continue;
        }
        let requested_width = page.width * scale;
        let requested_height = page.height * scale;
        if requested_width <= 3072.0 && requested_height <= 4096.0 {
            continue;
        }
        let width = ((page.width * density / 64.0).ceil() * 64.0).max(1.0);
        let height = (page.height * width / page.width).round().max(1.0);
        if width > MAX_CANVAS as f32 || height > MAX_CANVAS as f32 {
            continue;
        }
        let (width, height) = (width as u32, height as u32);
        let sx = width as f32 / page.width;
        let sy = height as f32 / page.height;
        let left = ((vx - page.x).max(0.0) * sx).floor() as u32;
        let top = ((vy - page.y).max(0.0) * sy).floor() as u32;
        let right = (((vx + vw - page.x).min(page.width) * sx).ceil() as u32).min(width);
        let bottom = (((vy + vh - page.y).min(page.height) * sy).ceil() as u32).min(height);
        let first_x = (left / TILE_EDGE).saturating_sub(1);
        let first_y = (top / TILE_EDGE).saturating_sub(1);
        let last_x = (right.saturating_sub(1) / TILE_EDGE + 1).min((width - 1) / TILE_EDGE);
        let last_y = (bottom.saturating_sub(1) / TILE_EDGE + 1).min((height - 1) / TILE_EDGE);
        for row in first_y..=last_y {
            for col in first_x..=last_x {
                let (cx, cy) = (col * TILE_EDGE, row * TILE_EDGE);
                let (x, y) = (cx.saturating_sub(1), cy.saturating_sub(1));
                let end_x = (cx + TILE_EDGE + 1).min(width);
                let end_y = (cy + TILE_EDGE + 1).min(height);
                let region = Region {
                    canvas_height: height,
                    x,
                    y,
                    width: end_x - x,
                    height: end_y - y,
                };
                let visible =
                    cx < right && cy < bottom && cx + TILE_EDGE > left && cy + TILE_EDGE > top;
                let tile_x = page.x + x as f32 / sx;
                let tile_y = page.y + y as f32 / sy;
                let tile_width = region.width as f32 / sx;
                let tile_height = region.height as f32 / sy;
                let distance = ((tile_x as f64 + tile_width as f64 / 2.0
                    - (vx as f64 + vw as f64 / 2.0))
                    * density as f64)
                    .powi(2)
                    + ((tile_y as f64 + tile_height as f64 / 2.0 - (vy as f64 + vh as f64 / 2.0))
                        * density as f64)
                        .powi(2);
                let candidate = Candidate {
                    priority: (!visible, distance as u64, page.page, row, col),
                    placement: Placement {
                        page: page.page,
                        canvas_width: width,
                        region,
                        x: tile_x,
                        y: tile_y,
                        width: tile_width,
                        height: tile_height,
                    },
                };
                if candidates.len() < MAX_REGIONS {
                    candidates.push(candidate);
                } else if candidates.peek().is_some_and(|worst| candidate < *worst) {
                    candidates.pop();
                    candidates.push(candidate);
                }
            }
        }
    }
    let mut bytes = 0;
    candidates
        .into_sorted_vec()
        .into_iter()
        .filter_map(|candidate| {
            let p = candidate.placement;
            if bytes + p.region.bytes() > DETAIL_BUDGET {
                return None;
            }
            bytes += p.region.bytes();
            Some(p)
        })
        .take(MAX_REGIONS)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn page(width: f32, height: f32) -> PagePlacement {
        PagePlacement {
            page: 7,
            x: 16.0,
            y: 16.0,
            width,
            height,
        }
    }
    #[test]
    fn ordinary_pages_keep_existing_path() {
        assert!(plan(&[page(1000.0, 1400.0)], (0.0, 0.0, 1180.0, 800.0), 1.0).is_empty());
    }
    #[test]
    fn visible_fractional_region_has_complete_coverage_and_bounded_gutters() {
        let pages = [page(8000.0, 12000.0)];
        let tiles = plan(&pages, (3521.25, 4173.75, 1179.5, 799.25), 2.0);
        assert!(!tiles.is_empty());
        assert!(tiles.len() <= MAX_REGIONS);
        assert!(tiles.iter().map(|p| p.region.bytes()).sum::<usize>() <= DETAIL_BUDGET);
        for tile in &tiles {
            assert!(tile.region.valid_for(tile.canvas_width));
        }
        for y in 0..80 {
            for x in 0..118 {
                let (px, py) = (3521.25 + x as f32 * 10.0, 4173.75 + y as f32 * 10.0);
                assert!(
                    tiles.iter().any(|t| px >= t.x
                        && px <= t.x + t.width
                        && py >= t.y
                        && py <= t.y + t.height),
                    "uncovered {px},{py}"
                );
            }
        }
    }
    #[test]
    fn invalid_geometry_never_allocates_or_enumerates() {
        assert!(plan(&[page(f32::NAN, 8000.0)], (0.0, 0.0, 1000.0, 800.0), 1.0).is_empty());
        assert!(
            plan(
                &[page(8000.0, 8000.0)],
                (0.0, 0.0, f32::INFINITY, 800.0),
                1.0
            )
            .is_empty()
        );
        assert!(plan(&[page(1e30, 1e30)], (0.0, 0.0, 1000.0, 800.0), 1.0).is_empty());
        assert!(
            !Region {
                canvas_height: 100,
                x: u32::MAX,
                y: 0,
                width: 1,
                height: 1
            }
            .valid_for(100)
        );
    }
    #[test]
    fn oversized_display_density_and_overscan_remain_bounded() {
        let tiles = plan(
            &[page(60000.0, 60000.0)],
            (17000.0, 21000.0, 10000.0, 8000.0),
            4.0,
        );
        assert!(!tiles.is_empty());
        assert!(tiles.iter().map(|p| p.region.bytes()).sum::<usize>() <= DETAIL_BUDGET);
        assert!(
            tiles
                .iter()
                .all(|p| p.region.width <= 512 && p.region.height <= 512)
        );
    }
    #[test]
    fn rotated_aspect_ratios_and_page_origins_keep_logical_mapping() {
        for (w, h) in [(8000.0, 12000.0), (12000.0, 8000.0)] {
            for unit in [0.5, 1.0, 2.0] {
                let mut page = page(w * unit, h * unit);
                page.x = -50.25;
                page.y = 17.75;
                let tiles = plan(&[page.clone()], (100.0, 200.0, 900.0, 600.0), 1.5);
                assert!(!tiles.is_empty());
                for tile in tiles {
                    assert!(
                        (tile.x
                            - page.x
                            - tile.region.x as f32 / tile.canvas_width as f32 * page.width)
                            .abs()
                            < 0.01
                    );
                    assert!(
                        (tile.y
                            - page.y
                            - tile.region.y as f32 / tile.region.canvas_height as f32
                                * page.height)
                            .abs()
                            < 0.01
                    );
                }
            }
        }
    }
    #[test]
    fn generation_changes_cannot_forget_outstanding_render_slots() {
        let slots = RenderSlots::default();
        let old_generation: Vec<_> = (0..MAX_PENDING_RENDERS)
            .map(|_| slots.acquire().unwrap())
            .collect();
        assert_eq!(slots.in_flight(), MAX_PENDING_RENDERS);
        assert!(slots.acquire().is_none());
        drop(old_generation);
        assert_eq!(slots.in_flight(), 0);
        let fresh = slots.acquire().unwrap();
        assert_eq!(slots.in_flight(), 1);
        drop(fresh);
        assert_eq!(slots.in_flight(), 0);
    }
    #[test]
    fn many_thin_visible_pages_keep_candidate_metadata_and_pixels_bounded() {
        let pages: Vec<_> = (0..1000)
            .map(|n| PagePlacement {
                page: n,
                x: 16.0,
                y: n as f32,
                width: 8000.0,
                height: 0.5,
            })
            .collect();
        let tiles = plan(&pages, (4000.0, 300.0, 1000.0, 500.0), 2.0);
        assert!(!tiles.is_empty());
        assert!(tiles.len() <= MAX_REGIONS);
        assert!(tiles.iter().map(|t| t.region.bytes()).sum::<usize>() <= DETAIL_BUDGET);
        assert!(tiles.iter().any(|t| (450..650).contains(&t.page)));
    }
}
