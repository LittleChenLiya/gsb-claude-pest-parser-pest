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

use super::queueable_token::QueueableToken;
use crate::span::Span;
use crate::RuleType;

/// A start/end marker event generated while traversing the tokens of [`Pairs`].
///
/// Each matched rule produces exactly one [`TokenEvent::Start`] and one
/// [`TokenEvent::End`] event, even when the rule matched an empty string.
/// Both events carry the matched `rule` and the `span` it covers.
///
/// [`Pairs`]: struct.Pairs.html
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TokenEvent<'i, R> {
    /// The start of a matched rule, together with the span it covers.
    Start {
        /// matched rule
        rule: R,
        /// span covered by the matched rule
        span: Span<'i>,
    },
    /// The end of a matched rule, together with the span it covers.
    End {
        /// matched rule
        rule: R,
        /// span covered by the matched rule
        span: Span<'i>,
    },
}

impl<'i, R: Copy> TokenEvent<'i, R> {
    /// Returns the rule of this event.
    pub fn as_rule(&self) -> R {
        match *self {
            TokenEvent::Start { rule, .. } | TokenEvent::End { rule, .. } => rule,
        }
    }

    /// Returns the span of this event.
    pub fn as_span(&self) -> Span<'i> {
        match *self {
            TokenEvent::Start { span, .. } | TokenEvent::End { span, .. } => span,
        }
    }

    /// Returns `true` if this is a [`TokenEvent::Start`] event.
    pub fn is_start(&self) -> bool {
        matches!(self, TokenEvent::Start { .. })
    }

    /// Returns `true` if this is a [`TokenEvent::End`] event.
    pub fn is_end(&self) -> bool {
        matches!(self, TokenEvent::End { .. })
    }
}

/// An iterator over [`TokenEvent`]s. It is created by [`Pairs::events`].
///
/// Events are yielded in tree nesting (document) order: a `Start` event is
/// emitted when a rule is entered and the matching `End` event after all of
/// its inner rules have been closed. Rules that matched an empty string still
/// produce a `Start`/`End` pair with an empty span.
///
/// The iterator only holds an `Rc` clone of the shared token queue, so
/// stopping the iteration early never affects the [`Pairs`] it was created
/// from.
///
/// [`Pairs`]: struct.Pairs.html
/// [`Pairs::events`]: struct.Pairs.html#method.events
#[derive(Clone)]
pub struct Events<'i, R> {
    queue: Rc<Vec<QueueableToken<'i, R>>>,
    input: &'i str,
    start: usize,
    end: usize,
}

pub fn new<'i, R: RuleType>(
    queue: Rc<Vec<QueueableToken<'i, R>>>,
    input: &'i str,
    start: usize,
    end: usize,
) -> Events<'i, R> {
    if cfg!(debug_assertions) {
        for tok in queue.iter() {
            match *tok {
                QueueableToken::Start { input_pos, .. } | QueueableToken::End { input_pos, .. } => {
                    assert!(
                        input.get(input_pos..).is_some(),
                        "💥 INVALID `Events` CREATED 💥"
                    )
                }
            }
        }
    }

    Events {
        queue,
        input,
        start,
        end,
    }
}

impl<'i, R: RuleType> Events<'i, R> {
    fn create_event(&self, index: usize) -> TokenEvent<'i, R> {
        match self.queue[index] {
            QueueableToken::Start {
                end_token_index,
                input_pos,
            } => {
                let (rule, end_pos) = match self.queue[end_token_index] {
                    QueueableToken::End {
                        rule, input_pos, ..
                    } => (rule, input_pos),
                    _ => unreachable!(),
                };

                TokenEvent::Start {
                    rule,
                    span: Span::new_internal(self.input, input_pos, end_pos),
                }
            }
            QueueableToken::End {
                start_token_index,
                rule,
                input_pos,
                ..
            } => {
                let start_pos = match self.queue[start_token_index] {
                    QueueableToken::Start { input_pos, .. } => input_pos,
                    _ => unreachable!(),
                };

                TokenEvent::End {
                    rule,
                    span: Span::new_internal(self.input, start_pos, input_pos),
                }
            }
        }
    }
}

impl<R: RuleType> ExactSizeIterator for Events<'_, R> {
    fn len(&self) -> usize {
        self.end - self.start
    }
}

impl<'i, R: RuleType> Iterator for Events<'i, R> {
    type Item = TokenEvent<'i, R>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start >= self.end {
            return None;
        }

        let event = self.create_event(self.start);

        self.start += 1;

        Some(event)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = <Self as ExactSizeIterator>::len(self);
        (len, Some(len))
    }
}

impl<R: RuleType> DoubleEndedIterator for Events<'_, R> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.end <= self.start {
            return None;
        }

        let event = self.create_event(self.end - 1);

        self.end -= 1;

        Some(event)
    }
}

impl<R: RuleType> fmt::Debug for Events<'_, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{Events, TokenEvent};
    use crate::{state, Span};
    use alloc::format;
    use alloc::vec;
    use alloc::vec::Vec;

    // A small grammar with two nested rules (`outer` wraps `inner`) and one
    // empty rule (`empty` matches the empty string).
    #[allow(non_camel_case_types)]
    #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    enum Rule {
        outer,
        inner,
        empty,
    }

    fn parse(input: &str) -> crate::iterators::Pairs<'_, Rule> {
        state(input, |state| {
            state.rule(Rule::outer, |s| {
                s.rule(Rule::inner, |s| s.match_string("a"))
                    .and_then(|s| s.rule(Rule::empty, |s| Ok(s)))
                    .and_then(|s| s.match_string("b"))
            })
        })
        .unwrap()
    }

    fn start(rule: Rule, input: &str, start: usize, end: usize) -> TokenEvent<'_, Rule> {
        TokenEvent::Start {
            rule,
            span: Span::new(input, start, end).unwrap(),
        }
    }

    fn end(rule: Rule, input: &str, start: usize, end: usize) -> TokenEvent<'_, Rule> {
        TokenEvent::End {
            rule,
            span: Span::new(input, start, end).unwrap(),
        }
    }

    #[test]
    fn events_follow_nesting_order_with_rule_and_span() {
        let input = "ab";
        let pairs = parse(input);
        let events: Vec<TokenEvent<'_, Rule>> = pairs.events().collect();

        assert_eq!(
            events,
            vec![
                start(Rule::outer, input, 0, 2),
                start(Rule::inner, input, 0, 1),
                end(Rule::inner, input, 0, 1),
                start(Rule::empty, input, 1, 1),
                end(Rule::empty, input, 1, 1),
                end(Rule::outer, input, 0, 2),
            ]
        );
    }

    #[test]
    fn events_are_paired_even_for_empty_rules() {
        let pairs = parse("ab");
        let events: Vec<TokenEvent<'_, Rule>> = pairs.events().collect();

        // Every rule, including the empty one, produces a Start/End pair
        // carrying the same rule and span.
        let mut stack: Vec<TokenEvent<'_, Rule>> = Vec::new();
        for event in &events {
            match *event {
                TokenEvent::Start { .. } => stack.push(*event),
                TokenEvent::End { rule, span } => {
                    let start = stack.pop().expect("unbalanced events");
                    assert!(start.is_start());
                    assert_eq!(start.as_rule(), rule);
                    assert_eq!(start.as_span(), span);
                }
            }
        }
        assert!(stack.is_empty());

        // The empty rule produced an empty span at the position between
        // `inner` and the trailing "b".
        let empty_events: Vec<_> = events
            .iter()
            .filter(|e| e.as_rule() == Rule::empty)
            .collect();
        assert_eq!(empty_events.len(), 2);
        assert!(empty_events[0].is_start());
        assert!(empty_events[1].is_end());
        assert_eq!(empty_events[0].as_span().as_str(), "");
        assert_eq!(
            (
                empty_events[0].as_span().start(),
                empty_events[0].as_span().end()
            ),
            (1, 1)
        );
    }

    #[test]
    fn stopping_events_early_keeps_original_pairs_usable() {
        let pairs = parse("ab");

        // Consume only the first event, then drop the iterator.
        let mut events = pairs.events();
        let first = events.next().unwrap();
        assert!(first.is_start());
        assert_eq!(first.as_rule(), Rule::outer);
        drop(events);

        // The original `Pairs` is untouched: it still yields its pairs and
        // `flatten` keeps working as before.
        assert_eq!(pairs.len(), 1);
        let outer = pairs.clone().next().unwrap();
        assert_eq!(outer.as_rule(), Rule::outer);
        assert_eq!(outer.as_span().as_str(), "ab");

        let flattened: Vec<Rule> = pairs.flatten().map(|p| p.as_rule()).collect();
        assert_eq!(flattened, vec![Rule::outer, Rule::inner, Rule::empty]);
    }

    #[test]
    fn double_ended_iter_for_events() {
        let pairs = parse("ab");
        let mut events = pairs.events().collect::<Vec<TokenEvent<'_, Rule>>>();
        events.reverse();
        let reverse_events = pairs.events().rev().collect::<Vec<TokenEvent<'_, Rule>>>();
        assert_eq!(events, reverse_events);
    }

    #[test]
    fn exact_size_iter_for_events() {
        let events = parse("ab").events();
        assert_eq!(events.len(), events.count());

        let mut events = parse("ab").events();
        let events_len = events.len();
        let _ = events.next().unwrap();
        assert_eq!(events.count() + 1, events_len);
    }

    #[test]
    fn events_debug() {
        let pairs = parse("ab");
        let events: Events<'_, Rule> = pairs.events();
        let debug = format!("{events:?}");
        assert!(debug.starts_with("[Start { rule: outer"));
        assert!(debug.ends_with("End { rule: outer, span: Span { str: \"ab\", range: 0..2 } }]"));
    }
}
