use crate::extract::{PageParams, Query};
use crate::math::interval::Interval;
use crate::web;
use serde::Serialize;

pub enum Page {
    Prev { offset: u64, disabled: bool },
    Next { offset: u64, disabled: bool },
    Number { offset: u64, number: u64, active: bool },
    Ellipsis,
}

impl Page {
    fn prev(page_size: u64, current_page: u64) -> Self {
        Page::Prev {
            offset: current_page.saturating_sub(2) * page_size,
            disabled: current_page <= 1,
        }
    }

    fn next(page_size: u64, current_page: u64, total_pages: u64) -> Self {
        Page::Next {
            offset: current_page * page_size,
            disabled: current_page >= total_pages,
        }
    }

    fn numbered(page_size: u64, number: u64, active: bool) -> Self {
        Page::Number {
            offset: number.saturating_sub(1) * page_size,
            number,
            active,
        }
    }

    fn inactive(page_size: u64, number: u64) -> Self {
        Self::numbered(page_size, number, false)
    }
}

pub struct Pager<'a, T> {
    pub pages: Vec<Page>,
    params: &'a T,
    route: &'static str,
}

impl<'a, T: Serialize> Pager<'a, T> {
    pub fn build(route: &'static str, params: &'a T, Query(page_params): Query<PageParams>, total: u64) -> Self {
        let page_size = page_params.limit();
        let current_page = page_params.current_page();
        let total_pages = total / page_size + 1;
        let pages = build_pages(page_size, current_page, total_pages);

        Self { pages, params, route }
    }

    pub fn url(&self, page: &Page) -> Result<String, serde_urlencoded::ser::Error> {
        let offset = match page {
            Page::Next { offset, .. } | Page::Prev { offset, .. } | Page::Number { offset, .. } => *offset,
            Page::Ellipsis => 0,
        };
        let params = PagerParams {
            params: self.params,
            offset,
        };

        web::url(self.route, &params)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct PagerParams<'a, T> {
    #[serde(flatten)]
    params: &'a T,
    offset: u64,
}

fn build_pages(page_size: u64, current_page: u64, total_pages: u64) -> Vec<Page> {
    const NUM_OPTIONS: u64 = 2;

    let all_pages = Interval::new(1, total_pages);
    let first_pages = Interval::new(1, NUM_OPTIONS).intersect(all_pages);
    let nearby_pages =
        Interval::new(current_page.saturating_sub(NUM_OPTIONS), current_page + NUM_OPTIONS).intersect(all_pages);
    let last_pages = Interval::new(total_pages.saturating_sub(NUM_OPTIONS - 1), total_pages).intersect(all_pages);

    let mut pages = Vec::new();

    // Create prev page
    pages.push(Page::prev(page_size, current_page));

    // Create first pages
    for page_number in first_pages.as_range().filter(|&page| !nearby_pages.contains(page)) {
        pages.push(Page::inactive(page_size, page_number));
    }

    // Create transition between first pages and nearby pages
    let transition_page = NUM_OPTIONS + 1;
    if all_pages.contains(transition_page) && !nearby_pages.contains(transition_page) {
        pages.push(Page::Ellipsis);
    }

    // Create nearby pages
    for page_number in nearby_pages.as_range() {
        pages.push(Page::numbered(page_size, page_number, page_number == current_page));
    }

    // Create transition between nearby pages and last pages
    let transition_page = total_pages.saturating_sub(NUM_OPTIONS);
    if all_pages.contains(transition_page) && !nearby_pages.contains(transition_page) {
        pages.push(Page::Ellipsis);
    }

    // Create last pages
    for page_number in last_pages.as_range().filter(|&page| !nearby_pages.contains(page)) {
        pages.push(Page::inactive(page_size, page_number));
    }

    // Create next page
    pages.push(Page::next(page_size, current_page, total_pages));

    pages
}
