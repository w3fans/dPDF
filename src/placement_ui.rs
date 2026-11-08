//! Per-tab draft marks. Physical coordinates stay independent of zoom and view rotation.
use crate::{
    document::{NormalizedRect, PageSize},
    overlay::{Geometry, Placement},
};
use anyhow::{Result, bail};
use std::collections::{HashSet, VecDeque};

#[derive(Clone)]
pub struct Mark {
    pub placement: Placement,
    pub previews: [slint::Image; 4],
    pub default_size: (f32, f32),
    pub aspect_locked: bool,
}

impl Mark {
    fn same_content(&self, other: &Self) -> bool {
        let (a, b) = (self.placement.geometry, other.placement.geometry);
        self.placement.page == other.placement.page
            && a.quarter_turns == b.quarter_turns
            && [
                (a.x_mm, b.x_mm),
                (a.y_mm, b.y_mm),
                (a.width_mm, b.width_mm),
                (a.height_mm, b.height_mm),
            ]
            .iter()
            .all(|(a, b)| (a - b).abs() < 0.0001)
            && (self.placement.opacity - other.placement.opacity).abs() < 0.000001
            && std::sync::Arc::ptr_eq(&self.placement.image, &other.placement.image)
            && self.aspect_locked == other.aspect_locked
            && self.default_size == other.default_size
    }
}

#[derive(Clone)]
struct Snapshot {
    items: Vec<Mark>,
    selected: Option<usize>,
}

pub struct Drafts {
    pub items: Vec<Mark>,
    pub selected: Option<usize>,
    pub generation: u64,
    drag: Option<Drag>,
    undo: VecDeque<Snapshot>,
    redo: Vec<Snapshot>,
    max_steps: usize,
    max_image_bytes: u64,
}
impl Default for Drafts {
    fn default() -> Self {
        Self {
            items: vec![],
            selected: None,
            generation: 0,
            drag: None,
            undo: VecDeque::new(),
            redo: vec![],
            max_steps: 64,
            max_image_bytes: 64 * 1024 * 1024,
        }
    }
}
struct Drag {
    index: usize,
    point: (f32, f32),
    rect: NormalizedRect,
    resize: bool,
    before: Snapshot,
}

fn from_view(
    rect: NormalizedRect,
    size: PageSize,
    view_rotation: u8,
    object_rotation: u8,
) -> Geometry {
    let rect = rect.rotated((4 - view_rotation % 4) % 4);
    let width = rect.width * size.width * 25.4 / 72.0;
    let height = rect.height * size.height * 25.4 / 72.0;
    let (width_mm, height_mm) = if object_rotation.is_multiple_of(2) {
        (width, height)
    } else {
        (height, width)
    };
    Geometry {
        x_mm: rect.x * size.width * 25.4 / 72.0,
        y_mm: rect.y * size.height * 25.4 / 72.0,
        width_mm,
        height_mm,
        quarter_turns: object_rotation,
    }
}

pub fn at_point(
    size: PageSize,
    rotation: u8,
    point: (f32, f32),
    width: f32,
    height: f32,
) -> Result<Geometry> {
    if ![point.0, point.1, width, height, size.width, size.height]
        .iter()
        .all(|v| v.is_finite())
        || width < 0.1
        || height < 0.1
        || size.width <= 0.0
        || size.height <= 0.0
    {
        bail!("Invalid placement size or coordinates");
    }
    let (page_w, page_h) = if rotation.is_multiple_of(2) {
        (size.width, size.height)
    } else {
        (size.height, size.width)
    };
    let (w, h) = (width * 72.0 / 25.4 / page_w, height * 72.0 / 25.4 / page_h);
    if w > 1.0 || h > 1.0 {
        bail!("Mark is larger than this page; reduce its preferred size in the studio");
    }
    let rect = NormalizedRect {
        x: (point.0 - w / 2.0).clamp(0.0, 1.0 - w),
        y: (point.1 - h / 2.0).clamp(0.0, 1.0 - h),
        width: w,
        height: h,
    };
    let geometry = from_view(rect, size, rotation, (4 - rotation % 4) % 4);
    geometry.normalized(size.width, size.height)?;
    Ok(geometry)
}

impl Drafts {
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            items: self.items.clone(),
            selected: self.selected,
        }
    }
    fn same_content(&self, snapshot: &Snapshot) -> bool {
        self.items.len() == snapshot.items.len()
            && self
                .items
                .iter()
                .zip(&snapshot.items)
                .all(|(a, b)| a.same_content(b))
    }
    fn resource_cost(&self) -> (u64, u64) {
        let mut seen = HashSet::new();
        let (mut images, mut previews) = (0, 0);
        let history = self
            .undo
            .iter()
            .chain(self.redo.iter())
            .flat_map(|s| s.items.iter());
        for mark in self.items.iter().chain(history) {
            if seen.insert(std::sync::Arc::as_ptr(&mark.placement.image) as usize) {
                images += mark.placement.image.len() as u64;
                previews += mark
                    .previews
                    .iter()
                    .map(|image| u64::from(image.size().width) * u64::from(image.size().height) * 4)
                    .sum::<u64>();
            }
        }
        (images, previews)
    }
    fn commit(&mut self, before: Snapshot) {
        if self.same_content(&before) {
            self.items = before.items;
            return;
        }
        self.generation += 1;
        self.redo.clear();
        self.undo.push_back(before);
        loop {
            let (images, previews) = self.resource_cost();
            if self.undo.len() <= self.max_steps
                && images <= self.max_image_bytes
                && previews <= self.max_image_bytes
            {
                break;
            }
            if self.undo.pop_front().is_none() {
                break;
            }
        }
    }
    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }
    pub fn restore(&mut self, items: Vec<Mark>, selected: Option<usize>) {
        self.clear();
        self.items = items;
        self.selected = selected.filter(|i| *i < self.items.len());
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
            || self
                .drag
                .as_ref()
                .is_some_and(|d| !self.same_content(&d.before))
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
            && self
                .drag
                .as_ref()
                .is_none_or(|d| self.same_content(&d.before))
    }
    pub fn insert(&mut self, mark: Mark) {
        self.finish_drag();
        let before = self.snapshot();
        self.items.push(mark);
        self.selected = Some(self.items.len() - 1);
        self.commit(before);
    }
    pub fn edit(&mut self, index: usize, geometry: Geometry, opacity: f32, aspect_locked: bool) {
        self.finish_drag();
        if index >= self.items.len() {
            return;
        }
        let before = self.snapshot();
        let mark = &mut self.items[index];
        mark.placement.geometry = geometry;
        mark.placement.opacity = opacity;
        mark.aspect_locked = aspect_locked;
        self.commit(before);
    }
    pub fn remove_selected(&mut self) {
        self.finish_drag();
        let Some(index) = self.selected.filter(|i| *i < self.items.len()) else {
            return;
        };
        let before = self.snapshot();
        self.items.remove(index);
        self.selected = None;
        self.commit(before);
    }
    pub fn finish_drag(&mut self) {
        if let Some(drag) = self.drag.take() {
            self.commit(drag.before);
        }
    }
    pub fn cancel_drag(&mut self) -> bool {
        let Some(drag) = self.drag.take() else {
            return false;
        };
        if !self.same_content(&drag.before) {
            self.generation += 1;
        }
        self.items = drag.before.items;
        self.selected = drag.before.selected;
        true
    }
    pub fn undo(&mut self) -> bool {
        self.finish_drag();
        let Some(before) = self.undo.pop_back() else {
            return false;
        };
        self.redo.push(self.snapshot());
        self.items = before.items;
        self.selected = before.selected;
        self.generation += 1;
        true
    }
    pub fn redo(&mut self) -> bool {
        self.finish_drag();
        let Some(after) = self.redo.pop() else {
            return false;
        };
        self.undo.push_back(self.snapshot());
        self.items = after.items;
        self.selected = after.selected;
        self.generation += 1;
        true
    }
    pub fn clear_after_save(&mut self, generation: u64) -> bool {
        if self.generation != generation {
            return false;
        }
        self.clear();
        true
    }
    pub fn clear(&mut self) {
        self.items.clear();
        self.selected = None;
        self.drag = None;
        self.undo.clear();
        self.redo.clear();
        self.generation += 1;
    }
    pub fn begin(
        &mut self,
        index: usize,
        point: (f32, f32),
        resize: bool,
        size: PageSize,
        rotation: u8,
        aspect_locked: Option<bool>,
    ) {
        self.finish_drag();
        let Some(mark) = self.items.get(index) else {
            return;
        };
        let Ok(rect) = mark.placement.geometry.normalized(size.width, size.height) else {
            return;
        };
        self.selected = Some(index);
        let before = self.snapshot();
        if let Some(lock) = aspect_locked {
            self.items[index].aspect_locked = lock;
        }
        self.drag = Some(Drag {
            index,
            point,
            rect: rect.rotated(rotation),
            resize,
            before,
        });
    }
    pub fn drag(
        &mut self,
        point: (f32, f32),
        page_pixels: (f32, f32),
        size: PageSize,
        rotation: u8,
        finish: bool,
    ) -> Result<()> {
        if ![point.0, point.1, page_pixels.0, page_pixels.1]
            .iter()
            .all(|v| v.is_finite())
            || page_pixels.0 <= 0.0
            || page_pixels.1 <= 0.0
        {
            bail!("Invalid drag coordinates");
        }
        let Some(drag) = &self.drag else {
            return Ok(());
        };
        let mark = &mut self.items[drag.index];
        let (dx, dy) = (
            (point.0 - drag.point.0) / page_pixels.0,
            (point.1 - drag.point.1) / page_pixels.1,
        );
        let mut rect = drag.rect;
        if drag.resize {
            if mark.aspect_locked {
                let factor = ((rect.width + dx) / rect.width)
                    .max((rect.height + dy) / rect.height)
                    .clamp(
                        0.01,
                        ((1.0 - rect.x) / rect.width).min((1.0 - rect.y) / rect.height),
                    );
                rect.width *= factor;
                rect.height *= factor;
            } else {
                rect.width = (rect.width + dx).clamp(0.00000001, (1.0 - rect.x).max(0.00000001));
                rect.height = (rect.height + dy).clamp(0.00000001, (1.0 - rect.y).max(0.00000001));
            }
        } else {
            rect.x = (rect.x + dx).clamp(0.0, (1.0 - rect.width).max(0.0));
            rect.y = (rect.y + dy).clamp(0.0, (1.0 - rect.height).max(0.0));
        }
        let geometry = from_view(rect, size, rotation, mark.placement.geometry.quarter_turns);
        let result = geometry.normalized(size.width, size.height);
        if result.is_ok() && geometry != mark.placement.geometry {
            mark.placement.geometry = geometry;
            self.generation += 1;
        }
        if finish {
            self.finish_drag();
        }
        result.map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mark(seed: u8) -> Mark {
        Mark {
            placement: Placement {
                page: 0,
                geometry: Geometry {
                    x_mm: 10.0,
                    y_mm: 20.0,
                    width_mm: 40.0,
                    height_mm: 20.0,
                    quarter_turns: 0,
                },
                opacity: 1.0,
                image: std::sync::Arc::new(vec![seed; 8]),
            },
            previews: std::array::from_fn(|_| slint::Image::default()),
            default_size: (40.0, 20.0),
            aspect_locked: true,
        }
    }
    #[test]
    fn history_shares_images_restores_edits_and_discards_branches() {
        let mut drafts = Drafts::default();
        let original = mark(1);
        let master = original.placement.image.clone();
        drafts.insert(original);
        let mut geometry = drafts.items[0].placement.geometry;
        geometry.width_mm = 60.0;
        drafts.edit(0, geometry, 0.5, false);
        drafts.remove_selected();
        assert!(drafts.items.is_empty());
        assert!(drafts.undo());
        assert_eq!(drafts.items[0].placement.opacity, 0.5);
        assert_eq!(drafts.selected, Some(0));
        assert!(drafts.undo());
        assert_eq!(drafts.items[0].placement.geometry.width_mm, 40.0);
        assert!(std::sync::Arc::ptr_eq(
            &master,
            &drafts.items[0].placement.image
        ));
        let generation = drafts.generation;
        drafts.edit(0, drafts.items[0].placement.geometry, 1.0, true);
        assert_eq!(drafts.generation, generation);
        assert!(drafts.can_redo());
        assert!(drafts.redo());
        assert_eq!(drafts.items[0].placement.geometry.width_mm, 60.0);
        assert!(drafts.undo());
        drafts.edit(0, geometry, 0.8, true);
        assert!(!drafts.can_redo());
        assert!(drafts.undo());
        assert!(drafts.undo());
        assert!(drafts.items.is_empty());
        assert!(!drafts.can_undo());
        assert!(drafts.can_redo());
    }
    #[test]
    fn gesture_is_one_action_and_cancel_restores_without_history() {
        let size = PageSize {
            width: 612.0,
            height: 792.0,
        };
        for rotation in 0..4 {
            let mut drafts = Drafts::default();
            drafts.insert(mark(1));
            let original = drafts.items[0].placement.geometry;
            drafts.begin(0, (100.0, 100.0), false, size, rotation, None);
            for step in 1..=10 {
                drafts
                    .drag(
                        (100.0 + step as f32, 105.0),
                        (600.0, 800.0),
                        size,
                        rotation,
                        step == 10,
                    )
                    .unwrap();
            }
            assert_ne!(drafts.items[0].placement.geometry, original);
            assert_eq!(drafts.undo.len(), 2);
            drafts.undo();
            assert_eq!(drafts.items[0].placement.geometry, original);
            drafts.begin(0, (100.0, 100.0), true, size, rotation, None);
            drafts
                .drag((120.0, 110.0), (600.0, 800.0), size, rotation, false)
                .unwrap();
            assert!(!drafts.can_redo());
            assert!(drafts.cancel_drag());
            assert_eq!(drafts.items[0].placement.geometry, original);
            assert!(drafts.can_redo());
            drafts.begin(0, (100.0, 100.0), false, size, rotation, None);
            drafts
                .drag((100.0, 100.0), (600.0, 800.0), size, rotation, true)
                .unwrap();
            assert_eq!(drafts.undo.len(), 1);
            assert!(drafts.can_redo());
        }
    }
    #[test]
    fn history_limits_release_removed_assets() {
        let mut drafts = Drafts {
            max_steps: 2,
            ..Drafts::default()
        };
        drafts.insert(mark(1));
        for step in 1..=8 {
            drafts.edit(
                0,
                drafts.items[0].placement.geometry,
                step as f32 / 10.0,
                true,
            );
        }
        assert_eq!(drafts.undo.len(), 2);
        assert!(drafts.undo());
        assert!(drafts.undo());
        assert!(!drafts.undo());
        let mut drafts = Drafts {
            max_image_bytes: 8,
            ..Drafts::default()
        };
        let first = mark(1);
        let weak = std::sync::Arc::downgrade(&first.placement.image);
        drafts.insert(first);
        drafts.remove_selected();
        assert!(weak.upgrade().is_some());
        drafts.insert(mark(2));
        assert!(weak.upgrade().is_none());
        assert_eq!(drafts.resource_cost().0, 8);
        drafts.clear();
        assert!(!drafts.can_undo());
        assert!(!drafts.can_redo());
    }
    #[test]
    fn stale_save_cannot_clear_newer_edits_even_after_undo() {
        let mut drafts = Drafts::default();
        drafts.insert(mark(1));
        let saved_generation = drafts.generation;
        drafts.edit(0, drafts.items[0].placement.geometry, 0.5, true);
        drafts.undo();
        assert_eq!(drafts.items[0].placement.opacity, 1.0);
        assert!(drafts.generation > saved_generation);
        assert!(!drafts.clear_after_save(saved_generation));
        assert_eq!(drafts.items.len(), 1);
        assert!(drafts.can_redo());
        assert!(drafts.clear_after_save(drafts.generation));
        assert!(drafts.items.is_empty());
        assert!(!drafts.can_undo());
        assert!(!drafts.can_redo());
    }
    #[test]
    fn click_places_upright_physical_size_in_each_view_rotation() {
        let size = PageSize {
            width: 612.0,
            height: 792.0,
        };
        for rotation in 0..4 {
            let geometry = at_point(size, rotation, (0.7, 0.8), 45.0, 20.0).unwrap();
            assert!(
                (geometry.width_mm - 45.0).abs() < 0.001
                    && (geometry.height_mm - 20.0).abs() < 0.001
            );
            let rect = geometry
                .normalized(size.width, size.height)
                .unwrap()
                .rotated(rotation);
            assert!((rect.x + rect.width / 2.0 - 0.7).abs() < 0.001);
            assert!((rect.y + rect.height / 2.0 - 0.8).abs() < 0.001);
            assert_eq!((geometry.quarter_turns + rotation) % 4, 0);
        }
        assert!(at_point(size, 0, (0.5, 0.5), 300.0, 20.0).is_err());
    }
}
