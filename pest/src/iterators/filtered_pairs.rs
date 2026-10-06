// pest. The Elegant Parser
// Copyright (c) 2018 Dragoș Tiselice
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::fmt;

use super::line_index::LineIndex;
use super::pair::{self, Pair};
use super::queueable_token::QueueableToken;
use crate::RuleType;

/// A read-only projection over [`Pairs`], selecting a single [`Rule`].
///
/// This iterator is created by [`Pairs::select`]. It yields only the existing
/// [`Pair`]s whose subtree contains at least one pair of the selected rule:
/// matching pairs are yielded together with the ancestors needed to reach
/// them, so the original parent path and [`Span`] of every yielded pair are
/// preserved. Pairs are shared with the originating [`Pairs`] by identity, so
/// nested pairs of the same rule stay distinct and are never merged.
///
/// The projection never re-parses the input and never fabricates tokens;
/// rules that produced no tokens (such as silent rules) cannot appear in it.
/// Descend with [`Pair::into_inner`] and [`Pairs::select`] to walk from the
/// kept ancestors down to the matching pairs.
///
/// [`Pairs`]: struct.Pairs.html
/// [`Pairs::select`]: struct.Pairs.html#method.select
/// [`Pair`]: struct.Pair.html
/// [`Pair::into_inner`]: struct.Pair.html#method.into_inner
/// [`Rule`]: ../trait.RuleType.html
/// [`Span`]: ../struct.Span.html
#[derive(Clone)]
pub struct FilteredPairs<'i, R> {
    queue: Rc<Vec<QueueableToken<'i, R>>>,
    input: &'i str,
    start: usize,
    end: usize,
    rule: R,
    line_index: Rc<LineIndex>,
}

pub fn new<'i, R: RuleType>(
    queue: Rc<Vec<QueueableToken<'i, R>>>,
    input: &'i str,
    line_index: Rc<LineIndex>,
    start: usize,
    end: usize,
    rule: R,
) -> FilteredPairs<'i, R> {
    FilteredPairs {
        queue,
        input,
        start,
        end,
        rule,
        line_index,
    }
}

impl<'i, R: RuleType> FilteredPairs<'i, R> {
    /// Returns the `Rule` this projection selects.
    #[inline]
    pub fn selected_rule(&self) -> R {
        self.rule
    }

    /// Returns `true` if the pair spanning the token range `start..=end`
    /// contains at least one token of the selected rule, itself included.
    fn contains_selected(&self, start: usize, end: usize) -> bool {
        self.queue[start..=end]
            .iter()
            .any(|token| matches!(token, QueueableToken::End { rule, .. } if *rule == self.rule))
    }
}

impl<'i, R: RuleType> Iterator for FilteredPairs<'i, R> {
    type Item = Pair<'i, R>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.start < self.end {
            let end = match self.queue[self.start] {
                QueueableToken::Start {
                    end_token_index, ..
                } => end_token_index,
                _ => unreachable!(),
            };
            let start = self.start;
            self.start = end + 1;

            if self.contains_selected(start, end) {
                return Some(pair::new(
                    Rc::clone(&self.queue),
                    self.input,
                    Rc::clone(&self.line_index),
                    start,
                ));
            }
        }

        None
    }
}

impl<R: RuleType> fmt::Debug for FilteredPairs<'_, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;
    use alloc::vec::Vec;

    use crate::{state, ParseResult, ParserState};

    #[allow(non_camel_case_types)]
    #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    enum Rule {
        sum,
        product,
        number,
        list,
        unused,
    }

    fn number(state: Box<ParserState<'_, Rule>>) -> ParseResult<Box<ParserState<'_, Rule>>> {
        state.rule(Rule::number, |s| s.match_char_by(|c| c.is_ascii_digit()))
    }

    // product = number ~ ("*" ~ number)*
    fn product(state: Box<ParserState<'_, Rule>>) -> ParseResult<Box<ParserState<'_, Rule>>> {
        state.rule(Rule::product, |s| {
            number(s)
                .and_then(|s| s.repeat(|s| s.sequence(|s| s.match_string("*").and_then(number))))
        })
    }

    // sum = product ~ ("+" ~ product)*, a three-level grammar over number
    fn sum(state: Box<ParserState<'_, Rule>>) -> ParseResult<Box<ParserState<'_, Rule>>> {
        state.rule(Rule::sum, |s| {
            product(s)
                .and_then(|s| s.repeat(|s| s.sequence(|s| s.match_string("+").and_then(product))))
        })
    }

    // list = number | "(" ~ list ~ ")", nesting the same rule
    fn list(state: Box<ParserState<'_, Rule>>) -> ParseResult<Box<ParserState<'_, Rule>>> {
        state.rule(Rule::list, |s| {
            number(s).or_else(|s| {
                s.sequence(|s| {
                    s.match_string("(")
                        .and_then(list)
                        .and_then(|s| s.match_string(")"))
                })
            })
        })
    }

    #[test]
    fn select_keeps_parent_path_for_leaf_rule() {
        // sum -> product -> number: three levels, only numbers selected.
        let pairs = state("1+2*3", sum).unwrap();

        let mut projection = pairs.select(Rule::number);
        let sum_pair = projection.next().unwrap();
        assert_eq!(sum_pair.as_rule(), Rule::sum);
        assert_eq!(sum_pair.as_str(), "1+2*3");
        assert_eq!(
            (sum_pair.as_span().start(), sum_pair.as_span().end()),
            (0, 5)
        );
        assert!(projection.next().is_none());

        let products: Vec<_> = sum_pair.into_inner().select(Rule::number).collect();
        assert_eq!(products.len(), 2);
        assert!(products.iter().all(|p| p.as_rule() == Rule::product));
        assert_eq!(products[0].as_str(), "1");
        assert_eq!(products[1].as_str(), "2*3");

        let numbers: Vec<_> = products
            .into_iter()
            .flat_map(|p| p.into_inner().select(Rule::number))
            .collect();
        let spans: Vec<_> = numbers
            .iter()
            .map(|p| (p.as_span().start(), p.as_span().end()))
            .collect();
        assert_eq!(spans, [(0, 1), (2, 3), (4, 5)]);
        assert!(numbers.iter().all(|p| p.as_rule() == Rule::number));
    }

    #[test]
    fn select_preserves_nested_same_rule_pairs() {
        let pairs = state("(7)", list).unwrap();

        let mut projection = pairs.select(Rule::list);
        let outer = projection.next().unwrap();
        assert_eq!(outer.as_rule(), Rule::list);
        assert_eq!((outer.as_span().start(), outer.as_span().end()), (0, 3));
        assert!(projection.next().is_none());

        let mut inner = outer.clone().into_inner().select(Rule::list);
        let nested = inner.next().unwrap();
        assert_eq!(nested.as_rule(), Rule::list);
        assert_eq!((nested.as_span().start(), nested.as_span().end()), (1, 2));
        assert!(inner.next().is_none());

        // Same rule, different real pairs: identity is preserved, not merged.
        assert_ne!(outer, nested);
        assert_eq!(nested.into_inner().select(Rule::list).next(), None);
    }

    #[test]
    fn select_only_yields_existing_pairs() {
        let pairs = state("1+2*3", sum).unwrap();
        let original: Vec<_> = pairs
            .clone()
            .flatten()
            .map(|p| (p.as_rule(), p.as_span().start(), p.as_span().end()))
            .collect();

        fn walk<'i>(pairs: crate::iterators::Pairs<'i, Rule>, acc: &mut Vec<(Rule, usize, usize)>) {
            for pair in pairs.select(Rule::number) {
                acc.push((pair.as_rule(), pair.as_span().start(), pair.as_span().end()));
                walk(pair.into_inner(), acc);
            }
        }

        let mut projected = Vec::new();
        walk(pairs, &mut projected);
        assert!(!projected.is_empty());
        // Every pair reachable through the projection already exists in the
        // original parse; nothing is re-parsed or fabricated.
        for node in &projected {
            assert!(original.contains(node));
        }
    }

    #[test]
    fn select_without_match_is_empty() {
        let pairs = state("1+2", sum).unwrap();
        assert!(pairs.select(Rule::unused).next().is_none());
    }
}
