//! Which display attribute each part of the composition gets (display_attributes.rs applies them).  Pure, so it is
//! tested on any system.
//!
//! Every word is one run, except that the parts of a word converted through an automatic correction get their own
//! runs, with a dotted underline.  The word the candidate list replaces and the word ↓ would change keep their own
//! look over the whole word instead: TSF gives a range one attribute, a dotted thick line would no longer read as the
//! cursor's word, and the candidate window open on a corrected word already says what was typed (`你打的是 …`).

use crate::protocol::Reply;

/// A display attribute, as an index into `display_attributes::STYLES` and the atoms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mark {
    Word = 0,
    /// The same look as `Word`: neighbouring runs alternate, so applications that leave a gap between attribute runs
    /// show where each one ends.
    WordNext = 1,
    Focus = 2,
    Target = 3,
    Corrected = 4,
    CorrectedNext = 5,
}

/// The runs of `reply.preedit` (UTF-16 ranges, in order, covering the words) and their marks; the whole text as one
/// word when the host sends no words.
pub(crate) fn runs(reply: &Reply) -> Vec<(usize, usize, Mark)> {
    let whole = [(0, reply.preedit.len())];
    let clauses = if reply.clauses.is_empty() { &whole[..] } else { &reply.clauses[..] };
    let mut runs = Vec::new();
    for (index, &(start, end)) in clauses.iter().enumerate() {
        if reply.focus == Some(index) {
            runs.push((start, end, Mark::Focus));
            continue;
        }
        if reply.target == Some(index) {
            runs.push((start, end, Mark::Target));
            continue;
        }
        // The parity of the word, so a corrected part never meets a corrected part of the next word with the same
        // attribute; inside a word, plain and corrected parts alternate anyway.
        let (plain, corrected) =
            if index % 2 == 0 { (Mark::Word, Mark::Corrected) } else { (Mark::WordNext, Mark::CorrectedNext) };
        let mut at = start;
        for &(from, to) in &reply.corrections {
            let (from, to) = (from.max(start), to.min(end));
            if from >= to {
                continue;
            }
            if at < from {
                runs.push((at, from, plain));
            }
            runs.push((from, to, corrected));
            at = to;
        }
        if at < end {
            runs.push((at, end, plain));
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;
    use Mark::*;

    fn reply(text: &str, clauses: &[(usize, usize)], corrections: &[(usize, usize)]) -> Reply {
        Reply {
            preedit: text.encode_utf16().collect(),
            clauses: clauses.to_vec(),
            corrections: corrections.to_vec(),
            ..Reply::default()
        }
    }

    #[test]
    fn words_alternate() {
        assert_eq!(runs(&reply("ㄓ", &[], &[])), vec![(0, 1, Word)]);
        assert_eq!(runs(&reply("你好嗎", &[(0, 2), (2, 3)], &[])), vec![(0, 2, Word), (2, 3, WordNext)]);
    }

    #[test]
    fn corrected_parts_are_split_out() {
        let marked = reply("你好嗎呢", &[(0, 2), (2, 4)], &[(1, 3)]);
        assert_eq!(runs(&marked), vec![(0, 1, Word), (1, 2, Corrected), (2, 3, CorrectedNext), (3, 4, WordNext)]);
        let whole = reply("你好嗎", &[(0, 2), (2, 3)], &[(0, 2), (2, 3)]);
        assert_eq!(runs(&whole), vec![(0, 2, Corrected), (2, 3, CorrectedNext)]);
        let inside = reply("你好嗎", &[(0, 3)], &[(0, 1), (2, 3)]);
        assert_eq!(runs(&inside), vec![(0, 1, Corrected), (1, 2, Word), (2, 3, Corrected)]);
        // No words: the whole text is one.
        assert_eq!(runs(&reply("你好", &[], &[(1, 2)])), vec![(0, 1, Word), (1, 2, Corrected)]);
    }

    #[test]
    fn focus_and_target_keep_the_whole_word() {
        let mut marked = reply("你好嗎", &[(0, 2), (2, 3)], &[(0, 3)]);
        marked.focus = Some(0);
        assert_eq!(runs(&marked), vec![(0, 2, Focus), (2, 3, CorrectedNext)]);
        marked.focus = None;
        marked.target = Some(1);
        assert_eq!(runs(&marked), vec![(0, 2, Corrected), (2, 3, Target)]);
    }
}
