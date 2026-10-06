// pest. The Elegant Parser
// Copyright (c) 2018 Dragoș Tiselice
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

use super::pair::Pair;
use super::pairs::Pairs;
use crate::span::Span;
use crate::RuleType;

/// A read-only projection of a [`Pair`] subtree, filtered by a single rule.
///
/// A projection is a view over pairs that **already exist**: it never
/// re-parses the input and never fabricates tokens (e.g. for silent rules).
/// It keeps exactly the pairs matching the filter rule plus the ancestor
/// pairs needed to reach them, so the original parent path of every match
/// stays observable. Rules, spans and strings are always those of the
/// original pairs.
///
/// Nested pairs that share the filter rule are kept as distinct nodes,
/// because pairs are filtered by identity, not merged by rule name.
///
/// The regular iteration interface of [`Pair`] (`into_inner`, `tokens`, ...)
/// is left untouched; if nothing matches, the projection is simply empty.
///
/// # Examples
///
/// ```
/// # use std::rc::Rc;
/// # use pest;
/// # #[allow(non_camel_case_types)]
/// # #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// enum Rule {
///     a,
///     b
/// }
///
/// let input = "ab";
/// let pair = pest::state(input, |state| {
///     // generating Token pair a(b()) ...
/// #     state.rule(Rule::a, |s| s.match_string("a").and_then(|s| s.rule(Rule::b, |s| s.match_string("b"))))
/// }).unwrap().next().unwrap();
///
/// let view = pair.project(Rule::b);
/// // `a` does not match, but is kept as the parent path of `b`.
/// assert!(!view.is_match());
/// let b = view.children().next().unwrap();
/// assert!(b.is_match());
/// assert_eq!(b.as_span().as_str(), "b");
/// ```
#[derive(Clone)]
pub struct PairProjection<'i, R> {
    pair: Pair<'i, R>,
    rule: R,
}

impl<'i, R: RuleType> PairProjection<'i, R> {
    /// Creates a projection of `pair`, filtered by `rule`.
    ///
    /// This is a view over the existing pair; nothing is re-parsed.
    #[inline]
    pub fn new(pair: Pair<'i, R>, rule: R) -> Self {
        PairProjection { pair, rule }
    }

    /// Returns the rule this projection filters by.
    #[inline]
    pub fn filter_rule(&self) -> R {
        self.rule
    }

    /// Returns the `Rule` of the underlying pair.
    #[inline]
    pub fn as_rule(&self) -> R {
        self.pair.as_rule()
    }

    /// Returns `true` if the underlying pair itself matches the filter rule.
    #[inline]
    pub fn is_match(&self) -> bool {
        self.pair.as_rule() == self.rule
    }

    /// Returns a reference to the underlying [`Pair`], unchanged.
    ///
    /// This preserves the real pair identity, so nested pairs with the same
    /// rule remain distinguishable.
    #[inline]
    pub fn as_pair(&self) -> &Pair<'i, R> {
        &self.pair
    }

    /// Consumes the projection and returns the underlying [`Pair`].
    #[inline]
    pub fn into_pair(self) -> Pair<'i, R> {
        self.pair
    }

    /// Returns the `Span` of the underlying pair, i.e. the original span.
    #[inline]
    pub fn as_span(&self) -> Span<'i> {
        self.pair.as_span()
    }

    /// Captures the slice from the original input defined by the underlying pair.
    #[inline]
    pub fn as_str(&self) -> &'i str {
        self.pair.as_str()
    }

    /// Returns the input string the underlying pair was created from.
    #[inline]
    pub fn get_input(&self) -> &'i str {
        self.pair.get_input()
    }

    /// Get the node tag of the underlying pair, if any.
    #[inline]
    pub fn as_node_tag(&self) -> Option<&str> {
        self.pair.as_node_tag()
    }

    /// Returns the `line`, `col` of the underlying pair start.
    #[inline]
    pub fn line_col(&self) -> (usize, usize) {
        self.pair.line_col()
    }

    /// Returns an iterator over the children kept by this projection.
    ///
    /// A child is kept if it matches the filter rule or if any of its
    /// descendants does (the necessary parent path); all other children are
    /// pruned. Children are yielded in their original order.
    #[inline]
    pub fn children(&self) -> ProjectionChildren<'i, R> {
        ProjectionChildren {
            inner: self.pair.clone().into_inner(),
            rule: self.rule,
        }
    }

    /// Returns a pre-order iterator over all nodes of this projection that
    /// match the filter rule, including `self` and nested matches.
    ///
    /// Nested pairs with the same rule are yielded as separate items; they
    /// are never merged.
    #[inline]
    pub fn matching(&self) -> ProjectionMatches<'i, R> {
        ProjectionMatches {
            stack: vec![self.clone()],
        }
    }
}

/// Whether `pair` is kept by a projection filtering for `rule`: the pair
/// itself matches, or one of its descendants does.
fn keeps<R: RuleType>(pair: &Pair<'_, R>, rule: R) -> bool {
    pair.as_rule() == rule || pair.clone().into_inner().any(|child| keeps(&child, rule))
}

/// An iterator over the children kept by a [`PairProjection`].
///
/// Created by [`PairProjection::children`].
#[derive(Clone)]
pub struct ProjectionChildren<'i, R> {
    inner: Pairs<'i, R>,
    rule: R,
}

impl<'i, R: RuleType> Iterator for ProjectionChildren<'i, R> {
    type Item = PairProjection<'i, R>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let child = self.inner.next()?;
            if keeps(&child, self.rule) {
                return Some(PairProjection::new(child, self.rule));
            }
        }
    }
}

impl<R: RuleType> fmt::Debug for ProjectionChildren<'_, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProjectionChildren").finish()
    }
}

/// A pre-order iterator over the matching nodes of a [`PairProjection`].
///
/// Created by [`PairProjection::matching`].
#[derive(Clone)]
pub struct ProjectionMatches<'i, R> {
    stack: Vec<PairProjection<'i, R>>,
}

impl<'i, R: RuleType> Iterator for ProjectionMatches<'i, R> {
    type Item = PairProjection<'i, R>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(node) = self.stack.pop() {
            // Only kept children can contain matches; push them reversed so
            // that iteration stays in the original (pre-order) order.
            self.stack
                .extend(node.children().collect::<Vec<_>>().into_iter().rev());
            if node.is_match() {
                return Some(node);
            }
        }
        None
    }
}

impl<R: RuleType> fmt::Debug for ProjectionMatches<'_, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProjectionMatches").finish()
    }
}

impl<R: RuleType> fmt::Debug for PairProjection<'_, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PairProjection")
            .field("filter", &self.rule)
            .field("rule", &self.as_rule())
            .field("span", &self.as_span())
            .field("children", &self.children().collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use crate::macros::tests::*;
    use crate::parser::Parser;
    use crate::state;

    #[test]
    fn projection_keeps_parent_path() {
        // the tokens a(b()), c()
        let pair = AbcParser::parse(Rule::a, "abcde").unwrap().next().unwrap();

        let view = pair.project(Rule::b);
        assert!(!view.is_match());
        assert_eq!(view.as_rule(), Rule::a);
        assert_eq!(view.as_span().as_str(), "abc");

        let children: Vec<_> = view.children().collect();
        assert_eq!(children.len(), 1);
        assert!(children[0].is_match());
        assert_eq!(children[0].as_rule(), Rule::b);
        assert_eq!(children[0].as_span().as_str(), "b");
        assert_eq!(children[0].as_span().start(), 1);
        assert_eq!(children[0].as_span().end(), 2);
    }

    #[test]
    fn projection_matching_collects_matches_in_order() {
        let pair = AbcParser::parse(Rule::a, "abcde").unwrap().next().unwrap();

        let matches: Vec<_> = pair.project(Rule::b).matching().collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].as_span().as_str(), "b");
    }

    #[test]
    fn projection_of_missing_rule_is_empty() {
        let pair = AbcParser::parse(Rule::a, "abcde").unwrap().next().unwrap();

        // `c` is a sibling of `a`, not inside it; `d` never produced tokens.
        for rule in [Rule::c, Rule::d] {
            let view = pair.project(rule);
            assert!(view.children().next().is_none());
            assert!(view.matching().next().is_none());
        }
    }

    #[test]
    fn projection_root_match_is_kept() {
        let pair = AbcParser::parse(Rule::a, "abcde").unwrap().next().unwrap();

        let view = pair.project(Rule::a);
        assert!(view.is_match());
        // `b` does not match and has no matching descendant, so it is pruned.
        assert!(view.children().next().is_none());
        let matches: Vec<_> = view.matching().collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].as_span().as_str(), "abc");
    }

    #[test]
    fn nested_same_rule_pairs_are_not_merged() {
        // the tokens a(a())
        let pair = state("xy", |state| {
            state.rule(Rule::a, |s| {
                s.match_string("x")
                    .and_then(|s| s.rule(Rule::a, |s| s.match_string("y")))
            })
        })
        .unwrap()
        .next()
        .unwrap();

        let view = pair.project(Rule::a);
        let matches: Vec<_> = view.matching().collect();
        assert_eq!(matches.len(), 2);
        // Both pairs have rule `a` but keep their own identity and span.
        assert_eq!(matches[0].as_span().as_str(), "xy");
        assert_eq!(matches[1].as_span().as_str(), "y");
        assert_ne!(matches[0].as_pair(), matches[1].as_pair());

        // The inner `a` is reachable as a kept child of the outer one.
        let children: Vec<_> = view.children().collect();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].as_span().as_str(), "y");
        assert_eq!(children[0].as_pair(), matches[1].as_pair());
    }

    #[test]
    fn projection_delegates_to_underlying_pair() {
        let pair = AbcParser::parse(Rule::a, "abcde").unwrap().next().unwrap();

        let view = pair.clone().project(Rule::b);
        assert_eq!(view.as_pair(), &pair);
        assert_eq!(view.as_str(), pair.as_str());
        assert_eq!(view.get_input(), pair.get_input());
        assert_eq!(view.line_col(), pair.line_col());
        assert_eq!(view.as_node_tag(), pair.as_node_tag());
        assert_eq!(view.filter_rule(), Rule::b);
        assert_eq!(view.into_pair(), pair);
    }
}
