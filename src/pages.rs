//! Immutable source-page references: assembly changes page order, never rasterizes source pages.
use anyhow::{Result, bail};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourcePage {
    pub source: usize,
    pub page: usize,
    pub turns: u8,
    /// Original primary page whose navigation should follow a replacement.
    pub primary_origin: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PagePlan {
    counts: Vec<usize>,
    pages: Vec<SourcePage>,
}

impl PagePlan {
    pub fn new(counts: Vec<usize>) -> Result<Self> {
        if counts.is_empty() || counts.contains(&0) {
            bail!("Each source PDF must contain at least one page");
        }
        let pages = (0..counts[0])
            .map(|page| SourcePage {
                source: 0,
                page,
                turns: 0,
                primary_origin: Some(page),
            })
            .collect();
        Ok(Self { counts, pages })
    }

    fn source_selection(&self, source: usize, selected: &[usize]) -> Result<Vec<SourcePage>> {
        let Some(count) = self.counts.get(source) else {
            bail!("Unknown source PDF");
        };
        if selected.is_empty() || selected.iter().any(|page| page >= count) {
            bail!("Source page selection is empty or out of bounds");
        }
        Ok(selected
            .iter()
            .map(|page| SourcePage {
                source,
                page: *page,
                turns: 0,
                primary_origin: (source == 0).then_some(*page),
            })
            .collect())
    }

    fn selection(&self, selected: &[usize]) -> Result<()> {
        if selected.is_empty() || selected.iter().any(|page| *page >= self.pages.len()) {
            bail!("Page selection is empty or out of bounds");
        }
        Ok(())
    }

    pub fn replace(&mut self, target: usize, source: usize, page: usize) -> Result<()> {
        self.selection(&[target])?;
        let mut replacement = self.source_selection(source, &[page])?[0];
        replacement.primary_origin = self.pages[target].primary_origin;
        self.pages[target] = replacement;
        Ok(())
    }

    pub fn insert(&mut self, before: usize, source: usize, selected: &[usize]) -> Result<()> {
        if before > self.pages.len() {
            bail!("Insertion position is out of bounds");
        }
        let pages = self.source_selection(source, selected)?;
        self.pages.splice(before..before, pages);
        Ok(())
    }

    pub fn remove(&mut self, selected: &[usize]) -> Result<()> {
        self.selection(selected)?;
        let selected: HashSet<_> = selected.iter().copied().collect();
        if selected.len() == self.pages.len() {
            bail!("Removing every page would create an empty PDF");
        }
        self.pages = self
            .pages
            .iter()
            .enumerate()
            .filter(|(index, _)| !selected.contains(index))
            .map(|(_, page)| *page)
            .collect();
        Ok(())
    }

    pub fn extract(&mut self, selected: &[usize]) -> Result<()> {
        self.selection(selected)?;
        self.pages = selected.iter().map(|page| self.pages[*page]).collect();
        Ok(())
    }

    pub fn reorder(&mut self, order: &[usize]) -> Result<()> {
        self.selection(order)?;
        let unique: HashSet<_> = order.iter().copied().collect();
        if order.len() != self.pages.len() || unique.len() != self.pages.len() {
            bail!("Reordering must include each page exactly once");
        }
        self.pages = order.iter().map(|page| self.pages[*page]).collect();
        Ok(())
    }

    pub fn rotate(&mut self, selected: &[usize], turns: i32) -> Result<()> {
        self.selection(selected)?;
        for index in selected.iter().copied().collect::<HashSet<_>>() {
            self.pages[index].turns =
                (self.pages[index].turns as i32 + turns.rem_euclid(4)).rem_euclid(4) as u8;
        }
        Ok(())
    }

    /// Move one output slot before another slot (or len() to append).
    pub fn move_page(&mut self, from: usize, before: usize) -> Result<()> {
        self.selection(&[from])?;
        if before > self.pages.len() {
            bail!("Drop position is out of bounds");
        }
        let page = self.pages.remove(from);
        self.pages.insert(before - usize::from(from < before), page);
        Ok(())
    }

    pub fn pages(&self) -> &[SourcePage] {
        &self.pages
    }

    pub fn source_count(&self) -> usize {
        self.counts.len()
    }
}

/// One-based user ranges to zero-based references, preserving explicit order and duplicates.
pub fn parse_selection(value: &str, count: usize) -> Result<Vec<usize>> {
    if count == 0 {
        bail!("The PDF contains no pages");
    }
    if value.trim().is_empty() {
        return Ok((0..count).collect());
    }
    let mut pages = Vec::new();
    for part in value.split(',') {
        let bounds: Vec<_> = part.trim().split('-').collect();
        if bounds.len() > 2 {
            bail!("Use page ranges such as 1-3,7");
        }
        let start: usize = bounds[0]
            .trim()
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid page number"))?;
        let end: usize = if bounds.len() == 2 {
            bounds[1]
                .trim()
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid page range"))?
        } else {
            start
        };
        if start == 0 || start > end || end > count {
            bail!("Pages must be between 1 and {count}");
        }
        if pages.len().saturating_add(end - start + 1) > 100_000 {
            bail!("Select at most 100,000 output pages");
        }
        pages.extend(start - 1..end);
    }
    Ok(pages)
}

pub fn ranges(pages: &[usize]) -> String {
    let mut parts = Vec::new();
    let mut index = 0;
    while index < pages.len() {
        let start = pages[index];
        let mut end = start;
        while index + 1 < pages.len() && pages[index + 1] == end + 1 {
            index += 1;
            end += 1;
        }
        parts.push(if start == end {
            (start + 1).to_string()
        } else {
            format!("{}-{}", start + 1, end + 1)
        });
        index += 1;
    }
    parts.join(",")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contract_replacement_preserves_first_nine_source_references() {
        let mut plan = PagePlan::new(vec![10, 1]).unwrap();
        let untouched = plan.pages[..9].to_vec();
        plan.replace(9, 1, 0).unwrap();
        assert_eq!(&plan.pages[..9], untouched);
        assert_eq!(
            plan.pages[9],
            SourcePage {
                source: 1,
                page: 0,
                turns: 0,
                primary_origin: Some(9)
            }
        );
        assert_eq!(plan.pages.len(), 10);
        plan.move_page(9, 0).unwrap();
        assert_eq!(plan.pages[0].primary_origin, Some(9));
        plan.replace(0, 1, 0).unwrap();
        assert_eq!(plan.pages[0].primary_origin, Some(9));
    }
    #[test]
    fn invalid_mutations_leave_a_valid_plan_unchanged() {
        let mut plan = PagePlan::new(vec![3, 1]).unwrap();
        let original = plan.clone();
        assert!(plan.remove(&[0, 1, 2]).is_err());
        assert!(plan.reorder(&[0, 0, 2]).is_err());
        assert!(plan.insert(4, 1, &[0]).is_err());
        assert!(plan.replace(0, 1, 1).is_err());
        assert!(plan.rotate(&[0, 4], 1).is_err());
        assert!(plan.move_page(0, 4).is_err());
        assert!(plan.move_page(3, 0).is_err());
        assert_eq!(plan, original);
    }
    #[test]
    fn drag_moves_preserve_source_identity_and_handle_adjacent_and_end_slots() {
        let mut plan = PagePlan::new(vec![4]).unwrap();
        plan.rotate(&[0], 1).unwrap();
        let rotated = plan.pages[0];
        plan.move_page(0, 4).unwrap();
        assert_eq!(
            plan.pages.iter().map(|p| p.page).collect::<Vec<_>>(),
            vec![1, 2, 3, 0]
        );
        assert_eq!(plan.pages[3], rotated);
        plan.move_page(3, 0).unwrap();
        let original = plan.clone();
        plan.move_page(0, 1).unwrap();
        plan.move_page(2, 2).unwrap();
        assert_eq!(plan, original);
    }
    #[test]
    fn selection_order_insert_extract_and_rotation_compose() {
        let mut plan = PagePlan::new(vec![3, 2]).unwrap();
        plan.insert(1, 1, &[1, 0]).unwrap();
        plan.rotate(&[1, 1, 4], -1).unwrap();
        assert_eq!(plan.pages[1].turns, 3);
        plan.remove(&[2]).unwrap();
        plan.reorder(&[3, 2, 1, 0]).unwrap();
        plan.extract(&[0, 2, 2]).unwrap();
        assert_eq!(plan.pages.len(), 3);
        assert_eq!(
            plan.pages[0],
            SourcePage {
                source: 0,
                page: 2,
                turns: 3,
                primary_origin: Some(2)
            }
        );
        assert_eq!(plan.pages[1], plan.pages[2]);
    }
    #[test]
    fn user_ranges_preserve_order_and_reject_malformed_or_excessive_requests() {
        assert_eq!(parse_selection("3,1-2,1", 3).unwrap(), vec![2, 0, 1, 0]);
        assert_eq!(ranges(&[2, 0, 1, 0]), "3,1-2,1");
        for bad in ["0", "4", "3-1", "1-", "1,,2", "1-2-3", "@file"] {
            assert!(parse_selection(bad, 3).is_err());
        }
        assert!(parse_selection("1-100000,1", 100000).is_err());
    }
}
