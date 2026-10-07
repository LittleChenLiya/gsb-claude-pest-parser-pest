//! Integration tests for `Error::with_tab_width`, demonstrating the
//! rendered output through a small temporary-directory fixture.

#![allow(non_camel_case_types)]

use std::fs;
use std::path::PathBuf;

use pest::error::{Error, ErrorVariant, InputLocation, LineColLocation};
use pest::Position;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum Rule {
    open_paren,
    closed_paren,
}

fn fixture_dir() -> PathBuf {
    // Cargo provides a writable temporary directory for integration tests
    // (located under `target/` in the workspace).
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("tab_width");
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Two-line short input with tabs around Chinese characters.
const INPUT: &str = "\t我\t\n\t中\t";

fn sample_error() -> Error<Rule> {
    // Byte position of `中` on the second line.
    let pos = Position::new(INPUT, 7).unwrap();
    Error::new_from_pos(
        ErrorVariant::ParsingError {
            positives: vec![Rule::open_paren],
            negatives: vec![Rule::closed_paren],
        },
        pos,
    )
}

#[test]
fn tab_width_fixture() {
    let width_4 = sample_error().with_tab_width(4).unwrap();
    let width_2 = sample_error().with_tab_width(2).unwrap();

    // Internal byte positions are not affected by the display tab width.
    for error in [&width_4, &width_2] {
        assert_eq!(error.location, InputLocation::Pos(7));
        assert_eq!(error.line_col, LineColLocation::Pos((2, 2)));
    }

    let rendered_4 = format!("{width_4}");
    let rendered_2 = format!("{width_2}");

    assert_eq!(
        rendered_4,
        [
            " --> 2:2",
            "  |",
            "2 |     中   ",
            "  |     ^---",
            "  |",
            "  = unexpected closed_paren; expected open_paren"
        ]
        .join("\n")
    );
    assert_eq!(
        rendered_2,
        [
            " --> 2:2",
            "  |",
            "2 |   中 ",
            "  |   ^---",
            "  |",
            "  = unexpected closed_paren; expected open_paren"
        ]
        .join("\n")
    );

    // Zero tab width is rejected.
    assert!(sample_error().with_tab_width(0).is_err());

    // Persist the rendered output into the temporary fixture directory so
    // the result can be inspected after the test run.
    let out = fixture_dir().join("rendered.txt");
    fs::write(
        &out,
        format!("tab width 4:\n{rendered_4}\n\ntab width 2:\n{rendered_2}\n"),
    )
    .unwrap();
    let written = fs::read_to_string(&out).unwrap();
    assert!(written.contains(&rendered_4));
    assert!(written.contains(&rendered_2));

    println!("wrote fixture to {}", out.display());
    println!("{rendered_4}");
    println!();
    println!("{rendered_2}");
}
