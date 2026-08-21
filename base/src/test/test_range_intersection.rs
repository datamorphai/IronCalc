#![allow(clippy::unwrap_used)]

//! Excel's intersection operator, which is written as a space.
//!
//! `=SUM(A1:A5 A3:A5)` sums what the two references have in common. The space
//! is the only place whitespace means anything in a formula, which is what made
//! this awkward: the lexer threw whitespace away, so `A1:A5 A3:A5` reached the
//! parser as two operands with nothing between them and answered `#ERROR!`.

use crate::test::util::new_empty_model;

/// A1:C5 filled with the row number, so a sum says how many rows were covered.
fn grid<'a>() -> crate::model::Model<'a> {
    let mut model = new_empty_model();
    for row in 1..=5 {
        for column in ["A", "B", "C"] {
            model._set(&format!("{column}{row}"), &row.to_string());
        }
    }
    model
}

#[test]
fn intersects_two_overlapping_ranges() {
    let mut model = grid();
    model._set("E1", "=SUM(A1:A5 A3:A5)");
    model._set("E2", "=SUM(A3:A5)");
    model.evaluate();
    // 3 + 4 + 5
    assert_eq!(model._get_text("E1"), "12");
    assert_eq!(model._get_text("E2"), "12");
}

#[test]
fn intersects_across_columns() {
    let mut model = grid();
    // A1:B5 and B1:C5 share column B.
    model._set("E1", "=SUM(A1:B5 B1:C5)");
    model._set("E2", "=SUM(B1:B5)");
    model.evaluate();
    assert_eq!(model._get_text("E1"), "15");
    assert_eq!(model._get_text("E2"), "15");
}

#[test]
fn a_bare_intersection_resolves_to_its_one_cell() {
    let mut model = grid();
    // A1:B5 and B2:C2 overlap in B2 alone.
    model._set("E1", "=A1:B5 B2:C2");
    model.evaluate();
    assert_eq!(model._get_text("E1"), "2");
}

#[test]
fn ranges_that_do_not_meet_are_null() {
    /*
     * `#NULL!` exists in Excel for this and almost nothing else. It is the
     * right answer rather than a complaint about the formula: `=A1:A5 C1:C5`
     * is written correctly and describes nothing.
     */
    let mut model = grid();
    model._set("E1", "=SUM(A1:A2 A4:A5)");
    model._set("E2", "=SUM(A1:A5 E7:F9)");
    model.evaluate();
    assert_eq!(model._get_text("E1"), "#NULL!");
    assert_eq!(model._get_text("E2"), "#NULL!");
}

#[test]
fn chains_left_to_right() {
    let mut model = grid();
    // Three ranges whose common part is C3:C5.
    model._set("E1", "=SUM(A1:C5 B2:C5 C3:C5)");
    model._set("E2", "=SUM(C3:C5)");
    model.evaluate();
    assert_eq!(model._get_text("E1"), "12");
    assert_eq!(model._get_text("E2"), "12");
}

#[test]
fn binds_looser_than_the_range_operator() {
    // `A1:A5 A3:A7` must intersect two ranges, not intersect `A5` with `A3` and
    // then span the result. The two readings differ, which is what makes this
    // worth asserting.
    let mut model = grid();
    model._set("E1", "=SUM(A1:A5 A3:A7)");
    model.evaluate();
    assert_eq!(model._get_text("E1"), "12");
}

#[test]
fn binds_tighter_than_arithmetic() {
    let mut model = grid();
    // The intersection is A3:A5 (12), then + 1.
    model._set("E1", "=SUM(A1:A5 A3:A5)+1");
    model.evaluate();
    assert_eq!(model._get_text("E1"), "13");
}

#[test]
fn the_formula_reads_back_with_its_space() {
    let mut model = grid();
    model._set("E1", "=SUM(A1:A5 A3:A5)");
    model.evaluate();
    assert_eq!(model._get_formula("E1"), "=SUM(A1:A5 A3:A5)");
}

#[test]
fn whitespace_everywhere_else_still_means_nothing() {
    /*
     * The risk this feature carries. Whitespace is insignificant everywhere in
     * a formula except between two references, so a rule that reads it too
     * eagerly turns working formulas into intersections. Each of these has a
     * space where a user would put one.
     */
    let mut model = grid();
    model._set("E1", "=SUM( A1 , A2 )");
    model._set("E2", "= A1 + A2");
    model._set("E3", "=IF( A1 > 0 , A2 , A3 )");
    model._set("E4", "=SUM(A1:A5 )");
    model._set("E5", "= 1 + 2 * 3");
    model.evaluate();
    assert_eq!(model._get_text("E1"), "3");
    assert_eq!(model._get_text("E2"), "3");
    assert_eq!(model._get_text("E3"), "2");
    assert_eq!(model._get_text("E4"), "15");
    assert_eq!(model._get_text("E5"), "7");
}
