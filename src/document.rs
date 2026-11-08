//! Document/view state is independent of the desktop toolkit and the PDF engine.
use std::path::PathBuf;

pub type DocumentId = u64;

/// Reveal a selected document tab while retaining an already useful scroll position.
pub fn tab_scroll_offset(
    active: usize,
    count: usize,
    tab_width: f32,
    viewport: f32,
    offset: f32,
) -> f32 {
    if count == 0
        || !tab_width.is_finite()
        || tab_width <= 0.0
        || !viewport.is_finite()
        || viewport <= 0.0
    {
        return 0.0;
    }
    let maximum = (count as f32 * tab_width - viewport).max(0.0);
    let current = if offset.is_finite() {
        offset.clamp(0.0, maximum)
    } else {
        0.0
    };
    let left = active.min(count - 1) as f32 * tab_width;
    let right = left + tab_width.min(viewport);
    (if left < current {
        left
    } else if right > current + viewport {
        right - viewport
    } else {
        current
    })
    .clamp(0.0, maximum)
}

#[derive(Clone, Copy, Debug)]
pub struct PageSize {
    pub width: f32,
    pub height: f32,
}

/// Convert engine coordinate sizes to physical points without changing the source.
/// The UI must keep its raw sizes so repeated metadata delivery is idempotent.
pub fn physical_page_sizes(sizes: &[PageSize], units: &[f64]) -> Option<Vec<PageSize>> {
    if sizes.is_empty() || sizes.len() != units.len() {
        return None;
    }
    sizes
        .iter()
        .zip(units)
        .map(|(size, &unit)| {
            if !unit.is_finite() || unit <= 0.0 {
                return None;
            }
            let width = (f64::from(size.width) * unit) as f32;
            let height = (f64::from(size.height) * unit) as f32;
            (width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0)
                .then_some(PageSize { width, height })
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct Bookmark {
    pub title: String,
    pub page: usize,
}

#[derive(Clone, Debug)]
pub struct DocumentInfo {
    pub id: DocumentId,
    pub path: PathBuf,
    pub pages: Vec<PageSize>,
    /// Snapshot recovery validates physical geometry before restoring marks.
    pub physical_pages: Option<Vec<PageSize>>,
    pub bookmarks: Vec<Bookmark>,
    pub signatures: usize,
    pub password_protected: bool,
}

/// Bounds normalized to the unrotated rendered page, with a top-left origin.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl NormalizedRect {
    pub fn rotated(self, turns: u8) -> Self {
        match turns % 4 {
            1 => Self {
                x: 1.0 - self.y - self.height,
                y: self.x,
                width: self.height,
                height: self.width,
            },
            2 => Self {
                x: 1.0 - self.x - self.width,
                y: 1.0 - self.y - self.height,
                ..self
            },
            3 => Self {
                x: self.y,
                y: 1.0 - self.x - self.width,
                width: self.height,
                height: self.width,
            },
            _ => self,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SearchHit {
    pub page: usize,
    pub bounds: Vec<NormalizedRect>,
}

/// Keep a document point at the same viewport position, clamping at document edges.
pub fn anchored_scroll(
    old_scroll: f32,
    old_point: f32,
    new_point: f32,
    viewport: f32,
    canvas: f32,
) -> f32 {
    (old_scroll + old_point - new_point).clamp((viewport - canvas).min(0.0), 0.0)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ViewMode {
    #[default]
    Single,
    Continuous,
    Facing,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Zoom {
    #[default]
    FitWidth,
    FitPage,
    Scale(f32),
}

#[derive(Clone, Debug, Default)]
pub struct ViewState {
    pub page: usize,
    pub mode: ViewMode,
    pub zoom: Zoom,
    pub rotation: u8,
    pub scroll_y: f32,
    pub scroll_x: f32,
}

#[derive(Clone, Debug)]
pub struct PagePlacement {
    pub page: usize,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl PagePlacement {
    pub fn near_viewport(&self, scroll_y: f32, viewport_height: f32) -> bool {
        let top = -scroll_y;
        self.y + self.height >= top - 100.0 && self.y <= top + viewport_height + 100.0
    }
}

#[derive(Debug)]
pub struct Layout {
    pub pages: Vec<PagePlacement>,
    pub width: f32,
    pub height: f32,
    pub scale: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct ViewportAnchor {
    pub page: usize,
    pub page_point: (f32, f32),
    pub screen_fraction: (f32, f32),
}

impl Layout {
    pub fn viewport_anchor(
        &self,
        viewport: (f32, f32),
        scroll: (f32, f32),
    ) -> Option<ViewportAnchor> {
        let (width, height) = viewport;
        if ![width, height, scroll.0, scroll.1]
            .iter()
            .all(|v| v.is_finite())
            || width <= 0.0
            || height <= 0.0
        {
            return None;
        }
        let center = (width / 2.0 - scroll.0, height / 2.0 - scroll.1);
        let distance = |page: &PagePlacement| {
            let dx = (page.x - center.0)
                .max(0.0)
                .max(center.0 - page.x - page.width);
            let dy = (page.y - center.1)
                .max(0.0)
                .max(center.1 - page.y - page.height);
            dx * dx + dy * dy
        };
        let page = self
            .pages
            .iter()
            .min_by(|a, b| distance(a).total_cmp(&distance(b)))?;
        let x = ((center.0 - page.x) / page.width).clamp(0.0, 1.0);
        let y = ((center.1 - page.y) / page.height).clamp(0.0, 1.0);
        Some(ViewportAnchor {
            page: page.page,
            page_point: (x, y),
            screen_fraction: (
                ((page.x + x * page.width + scroll.0) / width).clamp(0.0, 1.0),
                ((page.y + y * page.height + scroll.1) / height).clamp(0.0, 1.0),
            ),
        })
    }

    pub fn restore_viewport_anchor(
        &self,
        anchor: ViewportAnchor,
        viewport: (f32, f32),
    ) -> Option<(f32, f32)> {
        let page = self.pages.iter().find(|p| p.page == anchor.page)?;
        let x = anchor.screen_fraction.0 * viewport.0 - page.x - anchor.page_point.0 * page.width;
        let y = anchor.screen_fraction.1 * viewport.1 - page.y - anchor.page_point.1 * page.height;
        Some((
            x.clamp((viewport.0 - self.width).min(0.0), 0.0),
            y.clamp((viewport.1 - self.height).min(0.0), 0.0),
        ))
    }
}

pub fn page_layout(sizes: &[PageSize], view: &ViewState, width: f32, height: f32) -> Layout {
    const GAP: f32 = 16.0;
    if sizes.is_empty() {
        return Layout {
            pages: vec![],
            width,
            height,
            scale: 1.0,
        };
    }
    let page = view.page.min(sizes.len() - 1);
    let rotated: Vec<_> = sizes
        .iter()
        .map(|p| {
            let (w, h) = if view.rotation % 2 == 1 {
                (p.height, p.width)
            } else {
                (p.width, p.height)
            };
            // PDF points -> logical pixels at 96 DPI. Print size is independent of screen size.
            (w.max(1.0) * 96.0 / 72.0, h.max(1.0) * 96.0 / 72.0)
        })
        .collect();
    let first = if view.mode == ViewMode::Facing {
        page / 2 * 2
    } else {
        page
    };
    let (mut base_w, mut base_h) = rotated[first];
    if view.mode == ViewMode::Facing && first + 1 < sizes.len() {
        base_w += rotated[first + 1].0;
        base_h = base_h.max(rotated[first + 1].1);
    }
    let facing_gap = if view.mode == ViewMode::Facing {
        GAP
    } else {
        0.0
    };
    let fit_w = ((width - GAP * 2.0 - facing_gap) / base_w).max(0.05);
    let scale = match view.zoom {
        Zoom::FitWidth => fit_w,
        Zoom::FitPage => fit_w.min(((height - GAP * 2.0) / base_h).max(0.05)),
        Zoom::Scale(s) => s.clamp(0.1, 6.0),
    };
    let indices: Vec<usize> = match view.mode {
        ViewMode::Single => vec![page],
        ViewMode::Continuous => (0..sizes.len()).collect(),
        ViewMode::Facing => (first..(first + 2).min(sizes.len())).collect(),
    };
    let widest = if view.mode == ViewMode::Facing {
        base_w * scale + facing_gap
    } else {
        indices
            .iter()
            .map(|i| rotated[*i].0 * scale)
            .fold(0.0, f32::max)
    };
    let canvas_w = width.max(widest + 2.0 * GAP);
    let mut x = (canvas_w - widest) / 2.0;
    let mut y = GAP;
    let mut pages = Vec::with_capacity(indices.len());
    for i in indices {
        let (w, h) = (rotated[i].0 * scale, rotated[i].1 * scale);
        pages.push(PagePlacement {
            page: i,
            x: if view.mode == ViewMode::Facing {
                x
            } else {
                (canvas_w - w) / 2.0
            },
            y,
            width: w,
            height: h,
        });
        if view.mode == ViewMode::Facing {
            x += w + GAP;
        } else {
            y += h + GAP;
        }
    }
    let canvas_h = if view.mode == ViewMode::Facing {
        base_h * scale + 2.0 * GAP
    } else {
        y
    };
    Layout {
        pages,
        width: canvas_w,
        height: height.max(canvas_h),
        scale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn viewport_anchor_survives_reflow_and_clamps_at_document_edges() {
        for mode in [ViewMode::Single, ViewMode::Continuous, ViewMode::Facing] {
            for rotation in 0..4 {
                for zoom in [Zoom::FitWidth, Zoom::FitPage, Zoom::Scale(2.0)] {
                    let view = ViewState {
                        page: 1,
                        mode,
                        zoom,
                        rotation,
                        ..Default::default()
                    };
                    let old = page_layout(&sizes(), &view, 1000.0, 800.0);
                    let selected = old.pages.iter().find(|p| p.page == 1).unwrap();
                    let scroll = (
                        (500.0 - selected.x - selected.width * 0.6)
                            .clamp((1000.0 - old.width).min(0.0), 0.0),
                        (400.0 - selected.y - selected.height * 0.7)
                            .clamp((800.0 - old.height).min(0.0), 0.0),
                    );
                    let anchor = old.viewport_anchor((1000.0, 800.0), scroll).unwrap();
                    let new = page_layout(&sizes(), &view, 700.0, 600.0);
                    let restored = new.restore_viewport_anchor(anchor, (700.0, 600.0)).unwrap();
                    let p = new.pages.iter().find(|p| p.page == anchor.page).unwrap();
                    for (actual, desired, minimum) in [
                        (
                            restored.0,
                            anchor.screen_fraction.0 * 700.0 - p.x - anchor.page_point.0 * p.width,
                            (700.0 - new.width).min(0.0),
                        ),
                        (
                            restored.1,
                            anchor.screen_fraction.1 * 600.0 - p.y - anchor.page_point.1 * p.height,
                            (600.0 - new.height).min(0.0),
                        ),
                    ] {
                        assert!(actual >= minimum && actual <= 0.0);
                        if desired >= minimum && desired <= 0.0 {
                            assert!((actual - desired).abs() < 0.01);
                        }
                    }
                    let same = old
                        .restore_viewport_anchor(anchor, (1000.0, 800.0))
                        .unwrap();
                    assert!((same.0 - scroll.0).abs() < 0.01 && (same.1 - scroll.1).abs() < 0.01);
                }
            }
        }
        let empty = page_layout(&[], &ViewState::default(), 1000.0, 800.0);
        assert!(empty.viewport_anchor((1000.0, 800.0), (0.0, 0.0)).is_none());
        assert!(
            empty
                .viewport_anchor((f32::NAN, 800.0), (0.0, 0.0))
                .is_none()
        );
    }
    #[test]
    fn tab_reveal_handles_wrap_resize_removal_and_keeps_visible_positions() {
        let width = 212.0;
        let last = tab_scroll_offset(11, 12, width, 1000.0, 0.0);
        assert_eq!(last, 1544.0);
        assert_eq!(tab_scroll_offset(0, 12, width, 1000.0, last), 0.0);
        assert_eq!(tab_scroll_offset(7, 12, width, 1000.0, 1200.0), 1200.0);
        assert_eq!(tab_scroll_offset(11, 12, width, 1400.0, last), 1144.0);
        assert_eq!(tab_scroll_offset(11, 12, width, 800.0, last), 1744.0);
        assert_eq!(tab_scroll_offset(2, 3, width, 1000.0, last), 0.0);
        assert_eq!(tab_scroll_offset(11, 12, width, 100.0, 0.0), 2332.0);
        assert_eq!(tab_scroll_offset(0, 0, width, 1000.0, last), 0.0);
        assert_eq!(tab_scroll_offset(0, 1, width, f32::NAN, last), 0.0);
    }
    #[test]
    fn physical_geometry_uses_each_page_multiplier_and_rejects_invalid_facts() {
        let sizes = [
            PageSize {
                width: 306.0,
                height: 396.0,
            },
            PageSize {
                width: 1224.0,
                height: 1584.0,
            },
        ];
        let physical = physical_page_sizes(&sizes, &[2.0, 0.5]).unwrap();
        assert!(
            physical
                .iter()
                .all(|page| page.width == 612.0 && page.height == 792.0)
        );
        assert_eq!((sizes[0].width, sizes[1].height), (306.0, 1584.0));
        assert!(physical_page_sizes(&sizes, &[1.0]).is_none());
        assert!(physical_page_sizes(&[], &[]).is_none());
        for unit in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::MAX] {
            assert!(physical_page_sizes(&sizes, &[unit, 1.0]).is_none());
        }
    }

    fn sizes() -> Vec<PageSize> {
        vec![
            PageSize {
                width: 612.0,
                height: 792.0
            };
            3
        ]
    }

    #[test]
    fn search_bounds_rotate_with_page_and_return_after_four_turns() {
        let rect = NormalizedRect {
            x: 0.1,
            y: 0.2,
            width: 0.3,
            height: 0.15,
        };
        let rotated = rect.rotated(1);
        assert!((rotated.x - 0.65).abs() < 0.0001);
        assert!((rotated.y - 0.1).abs() < 0.0001);
        let restored = rotated.rotated(3);
        assert!((restored.x - rect.x).abs() < 0.0001);
        assert!((restored.y - rect.y).abs() < 0.0001);
        assert_eq!(rect.rotated(4), rect);
    }

    #[test]
    fn zoom_anchor_preserves_screen_position_except_at_document_edges() {
        let scroll = anchored_scroll(-400.0, 600.0, 900.0, 800.0, 2400.0);
        assert_eq!(scroll, -700.0);
        assert_eq!(-400.0 + 600.0, scroll + 900.0);
        assert_eq!(anchored_scroll(0.0, 100.0, 50.0, 800.0, 600.0), 0.0);
        assert_eq!(
            anchored_scroll(-1500.0, 2000.0, 3000.0, 800.0, 2400.0),
            -1600.0
        );
    }

    #[test]
    fn continuous_layout_preserves_page_order_and_limits_render_window() {
        let view = ViewState {
            mode: ViewMode::Continuous,
            zoom: Zoom::Scale(1.0),
            ..Default::default()
        };
        let layout = page_layout(&sizes(), &view, 1000.0, 800.0);
        assert_eq!(
            layout.pages.iter().map(|p| p.page).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert!(layout.pages[1].y > layout.pages[0].y + layout.pages[0].height);
        assert!(!layout.pages[2].near_viewport(0.0, 800.0));
        assert!(layout.pages[2].near_viewport(-layout.pages[2].y, 800.0));
    }

    #[test]
    fn facing_last_odd_page_is_valid_and_rotation_swaps_dimensions() {
        let view = ViewState {
            mode: ViewMode::Facing,
            page: 2,
            rotation: 1,
            zoom: Zoom::Scale(1.0),
            ..Default::default()
        };
        let layout = page_layout(&sizes(), &view, 1000.0, 800.0);
        assert_eq!(layout.pages.len(), 1);
        assert_eq!(layout.pages[0].page, 2);
        assert!(layout.pages[0].width > layout.pages[0].height);
    }

    #[test]
    fn fit_page_keeps_full_spread_inside_viewport() {
        let view = ViewState {
            mode: ViewMode::Facing,
            zoom: Zoom::FitPage,
            ..Default::default()
        };
        let layout = page_layout(&sizes(), &view, 1000.0, 600.0);
        assert!(layout.pages[1].x + layout.pages[1].width <= 1000.0);
        assert!(layout.pages.iter().all(|p| p.y + p.height <= 600.0));
    }
}
