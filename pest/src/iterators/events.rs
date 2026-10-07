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

/// An event generated while traversing [`Pairs`] with start/end markers.
///
/// Each matched rule produces exactly one [`Event::Start`] and one [`Event::End`], even when the
/// rule matched an empty slice of the input. Events are emitted in tree nesting (document) order:
/// the `Start` events of inner rules appear between the `Start` and `End` events of their
/// enclosing rule.
///
/// [`Pairs`]: struct.Pairs.html
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Event<'i, R> {
    /// The start of a matched rule, together with the `Span` the whole rule covers.
    Start {
        /// matched rule
        rule: R,
        /// span covered by the whole matched rule
        span: Span<'i>,
    },
    /// The end of a matched rule, together with the `Span` the whole rule covers.
    End {
        /// matched rule
        rule: R,
        /// span covered by the whole matched rule
        span: Span<'i>,
    },
}

impl<'i, R> Event<'i, R> {
    /// Returns the `Rule` of this event.
    pub fn as_rule(&self) -> R
    where
        R: RuleType,
    {
        match *self {
            Event::Start { rule, .. } | Event::End { rule, .. } => rule,
        }
    }

    /// Returns the `Span` covered by the matched rule this event belongs to.
    pub fn as_span(&self) -> Span<'i> {
        match *self {
            Event::Start { span, .. } | Event::End { span, .. } => span,
        }
    }
}

/// An iterator over [`Event`]s. It is created by [`Pairs::events`].
///
/// The iterator shares the token queue of the [`Pairs`] it was created from through a reference
/// counted pointer, so stopping the iteration early never alters the original `Pairs`.
///
/// [`Event`]: enum.Event.html
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
    Events {
        queue,
        input,
        start,
        end,
    }
}

impl<'i, R: RuleType> Events<'i, R> {
    fn create_event(&self, index: usize) -> Event<'i, R> {
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

                Event::Start {
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

                Event::End {
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
    type Item = Event<'i, R>;

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
    use super::Event;
    use crate::{state, ParseResult, ParserState};
    use alloc::boxed::Box;
    use alloc::vec;
    use alloc::vec::Vec;

    // A small in-memory grammar fixture: two nested rules (`outer` wrapping
    // `inner`) followed by one empty rule (`empty`).
    #[allow(non_camel_case_types)]
    #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    enum Rule {
        outer,
        inner,
        empty,
    }

    fn parse(state: Box<ParserState<'_, Rule>>) -> ParseResult<Box<ParserState<'_, Rule>>> {
        state
            .rule(Rule::outer, |s| {
                s.rule(Rule::inner, |s| s.match_string("ab"))
            })
            .and_then(|s| s.rule(Rule::empty, |s| Ok(s)))
    }

    fn kind<R>(event: &Event<'_, R>) -> &'static str {
        match event {
            Event::Start { .. } => "Start",
            Event::End { .. } => "End",
        }
    }

    #[test]
    fn events_pair_up_in_nesting_order_with_spans() {
        let pairs = state("ab", parse).unwrap();
        let events: Vec<Event<'_, Rule>> = pairs.events().collect();

        // Paired start/end markers, in tree nesting order; the empty rule
        // also produces a pair of events.
        assert_eq!(
            events
                .iter()
                .map(|e| (kind(e), e.as_rule()))
                .collect::<Vec<_>>(),
            vec![
                ("Start", Rule::outer),
                ("Start", Rule::inner),
                ("End", Rule::inner),
                ("End", Rule::outer),
                ("Start", Rule::empty),
                ("End", Rule::empty),
            ]
        );

        // Spans cover the whole matched rule on both markers.
        let spans: Vec<_> = events
            .iter()
            .map(|e| (e.as_span().start(), e.as_span().end(), e.as_span().as_str()))
            .collect();
        assert_eq!(
            spans,
            vec![
                (0, 2, "ab"),
                (0, 2, "ab"),
                (0, 2, "ab"),
                (0, 2, "ab"),
                (2, 2, ""),
                (2, 2, ""),
            ]
        );
    }

    #[test]
    fn empty_rule_produces_paired_events() {
        let pairs = state("", |s| s.rule(Rule::empty, |s| Ok(s))).unwrap();
        let events: Vec<Event<'_, Rule>> = pairs.events().collect();

        assert_eq!(events.len(), 2);
        assert!(matches!(
            events[0],
            Event::Start {
                rule: Rule::empty,
                ..
            }
        ));
        assert!(matches!(
            events[1],
            Event::End {
                rule: Rule::empty,
                ..
            }
        ));
        assert_eq!(events[0].as_span().as_str(), "");
        assert_eq!(events[0].as_span(), events[1].as_span());
    }

    #[test]
    fn early_stop_keeps_original_pairs_readable() {
        let pairs = state("ab", parse).unwrap();

        // Stop the event traversal after the first event.
        let mut events = pairs.events();
        assert!(matches!(
            events.next(),
            Some(Event::Start {
                rule: Rule::outer,
                ..
            })
        ));
        drop(events);

        // The original `Pairs` is unchanged and still yields all of its pairs.
        assert_eq!(
            pairs.clone().map(|p| p.as_rule()).collect::<Vec<_>>(),
            vec![Rule::outer, Rule::empty]
        );
        assert_eq!(pairs.as_str(), "ab");

        // The original flatten iteration is also unaffected.
        assert_eq!(
            pairs.flatten().map(|p| p.as_rule()).collect::<Vec<_>>(),
            vec![Rule::outer, Rule::inner, Rule::empty]
        );
    }

    #[test]
    fn double_ended_iter_for_events() {
        let pairs = state("ab", parse).unwrap();
        let mut events = pairs.events().collect::<Vec<Event<'_, Rule>>>();
        events.reverse();
        let reverse_events = pairs.events().rev().collect::<Vec<Event<'_, Rule>>>();
        assert_eq!(events, reverse_events);
    }

    #[test]
    fn exact_size_iter_for_events() {
        let events = state("ab", parse).unwrap().events();
        assert_eq!(events.len(), events.count());

        let mut events = state("ab", parse).unwrap().events();
        let events_len = events.len();
        let _ = events.next().unwrap();
        assert_eq!(events.count() + 1, events_len);
    }
}
