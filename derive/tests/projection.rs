// pest. The Elegant Parser
// Copyright (c) 2018 Dragoș Tiselice
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.
#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;
use alloc::vec::Vec;

use pest::iterators::{Pair, Pairs};
use pest::Parser;

#[derive(pest_derive::Parser)]
#[grammar = "tests/projection.pest"]
struct ProjectionParser;

fn span(pair: &Pair<'_, Rule>) -> (usize, usize) {
    (pair.as_span().start(), pair.as_span().end())
}

/// Collects every pair of `rule` reachable through the projection, descending
/// through the kept ancestors and through matches themselves, so nested pairs
/// of the same rule are all found.
fn collect_selected<'i>(pairs: Pairs<'i, Rule>, rule: Rule, out: &mut Vec<Pair<'i, Rule>>) {
    for pair in pairs.select(rule) {
        if pair.as_rule() == rule {
            out.push(pair.clone());
        }
        collect_selected(pair.into_inner(), rule, out);
    }
}

#[test]
fn select_keeps_parent_path_for_numbers() {
    // expr -> term -> atom -> number: selecting only `number` still locates
    // the original parent path, one real pair per level.
    let pairs = ProjectionParser::parse(Rule::expr, "1+2").unwrap();

    let mut projection = pairs.select(Rule::number);
    let expr = projection.next().unwrap();
    assert_eq!(expr.as_rule(), Rule::expr);
    assert_eq!(span(&expr), (0, 3));
    assert!(projection.next().is_none());

    let mut projection = expr.into_inner().select(Rule::number);
    let term = projection.next().unwrap();
    assert_eq!(term.as_rule(), Rule::term);
    assert_eq!(span(&term), (0, 1));

    let mut projection = term.into_inner().select(Rule::number);
    let atom = projection.next().unwrap();
    assert_eq!(atom.as_rule(), Rule::atom);
    assert_eq!(span(&atom), (0, 1));

    let mut projection = atom.into_inner().select(Rule::number);
    let number = projection.next().unwrap();
    assert_eq!(number.as_rule(), Rule::number);
    assert_eq!(number.as_str(), "1");
    assert_eq!(span(&number), (0, 1));
    assert!(projection.next().is_none());
}

#[test]
fn select_number_spans_match_original_parse() {
    let input = "1 + (2*3)";
    let pairs = ProjectionParser::parse(Rule::expr, input).unwrap();

    let expected: Vec<(usize, usize)> = pairs
        .clone()
        .flatten()
        .filter(|pair| pair.as_rule() == Rule::number)
        .map(|pair| span(&pair))
        .collect();
    assert_eq!(expected, [(0, 1), (5, 6), (7, 8)]);

    let mut numbers = Vec::new();
    collect_selected(pairs, Rule::number, &mut numbers);
    let spans: Vec<(usize, usize)> = numbers.iter().map(span).collect();
    assert_eq!(spans, expected);
    let strs: Vec<&str> = numbers.iter().map(|pair| pair.as_str()).collect();
    assert_eq!(strs, ["1", "2", "3"]);
}

#[test]
fn select_preserves_nested_same_rule_pairs() {
    let pairs = ProjectionParser::parse(Rule::expr, "1+(2*3)").unwrap();

    let mut exprs = Vec::new();
    collect_selected(pairs, Rule::expr, &mut exprs);

    // Both `expr` pairs survive as distinct real pairs; they are not merged.
    assert_eq!(exprs.len(), 2);
    assert!(exprs.iter().all(|pair| pair.as_rule() == Rule::expr));
    assert_ne!(exprs[0], exprs[1]);
    assert_eq!(span(&exprs[0]), (0, 7));
    assert_eq!(exprs[0].as_str(), "1+(2*3)");
    assert_eq!(span(&exprs[1]), (3, 6));
    assert_eq!(exprs[1].as_str(), "2*3");
}

#[test]
fn select_introduces_no_extra_nodes() {
    let pairs = ProjectionParser::parse(Rule::expr, "1 + (2*3)").unwrap();
    let original: Vec<(Rule, usize, usize)> = pairs
        .clone()
        .flatten()
        .map(|pair| (pair.as_rule(), pair.as_span().start(), pair.as_span().end()))
        .collect();

    fn walk(pairs: Pairs<'_, Rule>, acc: &mut Vec<(Rule, usize, usize)>) {
        for pair in pairs.select(Rule::number) {
            acc.push((pair.as_rule(), pair.as_span().start(), pair.as_span().end()));
            walk(pair.into_inner(), acc);
        }
    }

    let mut projected = Vec::new();
    walk(pairs, &mut projected);
    assert!(!projected.is_empty());
    // Every pair reachable through the projection already exists in the
    // original parse; nothing is re-parsed or fabricated, and the silent
    // WHITESPACE rule never materializes as a pair.
    for node in &projected {
        assert!(original.contains(node));
    }
    assert!(projected.iter().all(|(rule, ..)| *rule != Rule::WHITESPACE));
}

#[test]
fn select_without_match_is_empty() {
    let pairs = ProjectionParser::parse(Rule::expr, "1+2").unwrap();
    assert!(pairs.select(Rule::name).next().is_none());
}

#[test]
fn original_iteration_is_unaffected() {
    let pairs = ProjectionParser::parse(Rule::expr, "1+2").unwrap();
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs.clone().next().unwrap().as_rule(), Rule::expr);
    // expr, term, atom, number, term, atom, number
    assert_eq!(pairs.flatten().count(), 7);
}
