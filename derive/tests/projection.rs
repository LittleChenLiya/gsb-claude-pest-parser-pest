// pest. The Elegant Parser
// Copyright (c) 2018 Dragoș Tiselice
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

extern crate pest;
#[macro_use]
extern crate pest_derive;

use pest::iterators::{Pair, PairProjection};
use pest::Parser;

#[derive(Parser)]
#[grammar = "tests/projection.pest"]
struct ProjectionParser;

fn parse(input: &str) -> Pair<'_, Rule> {
    ProjectionParser::parse(Rule::expr, input)
        .unwrap()
        .next()
        .unwrap()
}

/// Collects the spans of all pairs with `rule` by walking the original tree.
fn original_spans(pair: &Pair<'_, Rule>, rule: Rule, out: &mut Vec<(usize, usize)>) {
    if pair.as_rule() == rule {
        let span = pair.as_span();
        out.push((span.start(), span.end()));
    }
    for inner in pair.clone().into_inner() {
        original_spans(&inner, rule, out);
    }
}

#[test]
fn projection_keeps_parent_path_for_numbers() {
    let input = "(1+2) + 3";
    let root = parse(input);
    let view = root.project(Rule::number);

    // The root itself does not match, but stays as the start of the path.
    assert!(!view.is_match());
    assert_eq!(view.as_rule(), Rule::expr);
    assert_eq!(view.as_span().as_str(), input);

    // Three levels down to the first number: expr -> term -> expr -> term -> number.
    let term = view.children().next().unwrap();
    assert_eq!(term.as_rule(), Rule::term);
    assert_eq!(term.as_str(), "(1+2)");

    let inner_expr = term.children().next().unwrap();
    assert_eq!(inner_expr.as_rule(), Rule::expr);
    assert_eq!(inner_expr.as_str(), "1+2");

    let inner_term = inner_expr.children().next().unwrap();
    assert_eq!(inner_term.as_rule(), Rule::term);

    let number = inner_term.children().next().unwrap();
    assert!(number.is_match());
    assert_eq!(number.as_rule(), Rule::number);
    assert_eq!(number.as_str(), "1");
    assert_eq!((number.as_span().start(), number.as_span().end()), (1, 2));
    assert!(number.children().next().is_none());
}

#[test]
fn projection_spans_match_original_pairs() {
    let input = "(1+2) + 3";
    let root = parse(input);

    let mut expected = Vec::new();
    original_spans(&root, Rule::number, &mut expected);

    let actual: Vec<_> = root
        .project(Rule::number)
        .matching()
        .map(|n| (n.as_span().start(), n.as_span().end()))
        .collect();

    assert_eq!(actual, vec![(1, 2), (3, 4), (8, 9)]);
    assert_eq!(actual, expected);
}

#[test]
fn nested_same_rule_pairs_are_not_merged() {
    let root = parse("(1+2) + 3");

    let matches: Vec<_> = root.project(Rule::expr).matching().collect();
    // The outer `expr` and the parenthesized inner `expr` are both kept.
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].as_str(), "(1+2) + 3");
    assert_eq!(matches[1].as_str(), "1+2");
    // Distinct pairs: same rule, different identity and span.
    assert_ne!(matches[0].as_pair(), matches[1].as_pair());
    assert_ne!(matches[0].as_span(), matches[1].as_span());

    // The inner `expr` remains reachable through the kept parent path.
    let term = matches[0].children().next().unwrap();
    let inner = term.children().next().unwrap();
    assert_eq!(inner.as_pair(), matches[1].as_pair());
}

#[test]
fn silent_rules_are_not_fabricated() {
    let input = "(1 + 2) + 3";
    let root = parse(input);

    // Walking the whole projection only ever yields real pairs.
    let view = root.project(Rule::number);
    let mut stack: Vec<PairProjection<'_, Rule>> = vec![view];
    while let Some(node) = stack.pop() {
        assert_ne!(node.as_rule(), Rule::WHITESPACE);
        stack.extend(node.children());
    }

    // Filtering by the silent rule selects nothing; no nodes are invented.
    let silent = root.project(Rule::WHITESPACE);
    assert!(silent.children().next().is_none());
    assert!(silent.matching().next().is_none());
}

#[test]
fn projection_with_no_matches_is_empty() {
    let root = parse("1");

    let view = root.project(Rule::op);
    assert!(view.children().next().is_none());
    assert!(view.matching().next().is_none());
}

#[test]
fn projection_exposes_underlying_pair() {
    let input = "(1+2) + 3";
    let root = parse(input);

    let view = root.clone().project(Rule::number);
    assert_eq!(view.as_pair(), &root);
    assert_eq!(view.as_str(), input);
    assert_eq!(view.get_input(), input);
    assert_eq!(view.filter_rule(), Rule::number);
    assert_eq!(view.into_pair(), root);
}
