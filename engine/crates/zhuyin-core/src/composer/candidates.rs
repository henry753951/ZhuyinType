use crate::conversion::FixedWord;

// An open window always has at least one choice and a selection within it.
#[derive(Default)]
pub(super) struct CandidateWindow(Option<Open>);

struct Open {
    choices: Vec<FixedWord>,
    selection: usize,
    target: usize,
    single_character: bool,
    tolerant: bool,
    literal: bool,
}

impl CandidateWindow {
    pub(super) fn open(
        &mut self,
        choices: Vec<FixedWord>,
        target: usize,
        single_character: bool,
        tolerant: bool,
        literal: bool,
    ) {
        self.0 = (!choices.is_empty()).then_some(Open {
            choices,
            selection: 0,
            target,
            single_character,
            tolerant,
            literal,
        });
    }
    pub(super) fn close(&mut self) {
        self.0 = None;
    }
    pub(super) fn is_closed(&self) -> bool {
        self.0.is_none()
    }
    pub(super) fn is_open(&self) -> bool {
        self.0.is_some()
    }
    pub(super) fn choices(&self) -> &[FixedWord] {
        self.0.as_ref().map_or(&[], |w| &w.choices)
    }
    pub(super) fn get(&self, index: usize) -> Option<&FixedWord> {
        self.choices().get(index)
    }
    pub(super) fn selected(&self) -> Option<&FixedWord> {
        let w = self.0.as_ref()?;
        w.choices.get(w.selection)
    }
    pub(super) fn selection(&self) -> usize {
        self.0.as_ref().map_or(0, |w| w.selection)
    }
    pub(super) fn single_character(&self) -> bool {
        self.0.as_ref().is_some_and(|w| w.single_character)
    }
    pub(super) fn tolerant(&self) -> bool {
        self.0.as_ref().is_some_and(|w| w.tolerant)
    }
    pub(super) fn literal(&self) -> bool {
        self.0.as_ref().is_some_and(|w| w.literal)
    }
    pub(super) fn valid_for(&self, count: usize) -> bool {
        self.0.as_ref().is_none_or(|w| {
            w.selection < w.choices.len()
                && w.target < count
                && w.choices.iter().all(|c| c.range.start < c.range.end && c.range.end <= count)
        })
    }
    pub(super) fn move_page(&mut self, size: usize, forward: bool, wrap: bool) {
        let Some(w) = &mut self.0 else { return };
        let pages = w.choices.len().div_ceil(size);
        let page = w.selection / size;
        let next = if wrap {
            (page + 1) % pages
        } else if forward {
            page.saturating_add(1).min(pages - 1)
        } else {
            page.saturating_sub(1)
        };
        w.selection = next * size;
    }
    pub(super) fn move_selection(&mut self, size: usize, forward: bool, page_at_edge: bool) {
        let Some(w) = &mut self.0 else { return };
        if forward {
            if page_at_edge || w.selection % size != size - 1 {
                w.selection = w.selection.saturating_add(1).min(w.choices.len() - 1);
            }
        } else if page_at_edge || !w.selection.is_multiple_of(size) {
            w.selection = w.selection.saturating_sub(1);
        }
    }
    pub(super) fn target(&self, count: usize, forward: bool) -> Option<usize> {
        let w = self.0.as_ref()?;
        let last = count.checked_sub(1)?;
        let next = if forward { w.target.saturating_add(1).min(last) } else { w.target.saturating_sub(1) };
        (next != w.target).then_some(next)
    }
}
