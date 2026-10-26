/// Nombre de pages affichées autour de la page courante.
const RANGE: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageItem {
    Page(u32),
    Gap,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pagination {
    pub page_count: u32,
    pub current: u32,
    pub previous: Option<u32>,
    pub next: Option<u32>,
    pub start: u32,
    pub end: u32,
}

impl Pagination {
    /// `requested` peut être n'importe quelle valeur issue de l'URL : elle est ramenée dans `1..=page_count`.
    pub fn new(total: u64, per_page: u64, requested: i64) -> Self {
        let page_count = total.div_ceil(per_page.max(1)).min(u32::MAX as u64) as u32;
        let current = requested.clamp(1, page_count.max(1) as i64) as u32;

        let proximity = RANGE / 2;
        let mut start = current as i64 - proximity as i64;
        let mut end = (current + proximity) as i64;
        let count = page_count as i64;

        if start < 1 {
            end = (end + (1 - start)).min(count);
            start = 1;
        }
        if end > count {
            start = (start - (end - count)).max(1);
            end = count;
        }

        Self {
            page_count,
            current,
            previous: (current > 1).then(|| current - 1),
            next: (current < page_count).then(|| current + 1),
            start: start as u32,
            end: end.max(0) as u32,
        }
    }

    pub fn items(&self) -> Vec<PageItem> {
        let mut items = Vec::new();
        if self.page_count < 2 {
            return items;
        }

        if self.start > 1 {
            items.push(PageItem::Page(1));
            match self.start {
                2 => {}
                3 => items.push(PageItem::Page(2)),
                _ => items.push(PageItem::Gap),
            }
        }

        items.extend((self.start..=self.end).map(PageItem::Page));

        if self.page_count > self.end {
            if self.page_count > self.end + 1 {
                if self.page_count > self.end + 2 {
                    items.push(PageItem::Gap);
                } else {
                    items.push(PageItem::Page(self.page_count - 1));
                }
            }
            items.push(PageItem::Page(self.page_count));
        }

        items
    }
}

#[cfg(test)]
mod tests {
    use super::PageItem::{Gap, Page};
    use super::*;

    #[test]
    fn empty_and_single_page_have_no_items() {
        assert!(Pagination::new(0, 24, 1).items().is_empty());
        let p = Pagination::new(10, 24, 1);
        assert_eq!(
            (p.page_count, p.current, p.previous, p.next),
            (1, 1, None, None)
        );
        assert!(p.items().is_empty());
    }

    #[test]
    fn page_count_rounds_up() {
        assert_eq!(Pagination::new(24, 24, 1).page_count, 1);
        assert_eq!(Pagination::new(25, 24, 1).page_count, 2);
        assert_eq!(Pagination::new(103, 24, 1).page_count, 5);
    }

    #[test]
    fn requested_page_is_clamped() {
        assert_eq!(Pagination::new(103, 24, 0).current, 1);
        assert_eq!(Pagination::new(103, 24, -7).current, 1);
        assert_eq!(Pagination::new(103, 24, 99).current, 5);
    }

    #[test]
    fn first_page() {
        let p = Pagination::new(103, 24, 1);
        assert_eq!((p.previous, p.next), (None, Some(2)));
        assert_eq!(p.items(), vec![Page(1), Page(2), Page(3), Page(4), Page(5)]);
    }

    #[test]
    fn middle_page_of_many() {
        let p = Pagination::new(24 * 20, 24, 10);
        assert_eq!((p.previous, p.next), (Some(9), Some(11)));
        assert_eq!(
            p.items(),
            vec![Page(1), Gap, Page(9), Page(10), Page(11), Gap, Page(20)]
        );
    }

    #[test]
    fn single_hidden_page_is_shown() {
        let p = Pagination::new(24 * 8, 24, 4);
        assert_eq!(
            p.items(),
            vec![Page(1), Page(2), Page(3), Page(4), Page(5), Gap, Page(8)]
        );
        let p = Pagination::new(24 * 6, 24, 5);
        assert_eq!(p.items(), vec![Page(1), Gap, Page(4), Page(5), Page(6)]);
    }

    #[test]
    fn last_page() {
        let p = Pagination::new(24 * 20, 24, 20);
        assert_eq!((p.previous, p.next), (Some(19), None));
        assert_eq!(p.items(), vec![Page(1), Gap, Page(18), Page(19), Page(20)]);
    }
}
