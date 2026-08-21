#![allow(clippy::unwrap_used)]

//! The clamp that `range_bounds` now applies to every aggregation.
//!
//! Until this refactor only `SUM` shortened a whole-column reference to the
//! rows a sheet actually uses; `MIN`, `MAX`, `AVERAGE`, `COUNT` and the
//! deviation family each walked all 1,048,576. Sharing the clamp makes them
//! fast, and the risk that buys is that a clamp which cut one row too few or
//! too many would change an answer rather than merely a runtime.
//!
//! So these compare a whole-column reference against the explicit range it
//! stands for. They are equal, and they must stay equal.

use crate::test::util::new_empty_model;

/// A sheet whose only content is A1:A5 — so its dimension ends where the data
/// does, and a clamp that cuts one row too many loses a value.
///
/// The formulas go on a second sheet for exactly that reason. An earlier
/// version of this test put them in column C of the same sheet, which pushed
/// the dimension past the data and made the assertion vacuous: deliberately
/// breaking the clamp to `max_row - 1` still passed, because row 17 of an empty
/// column holds nothing either way. Keeping the referenced sheet bare is what
/// gives the comparison teeth.
fn two_sheet_model<'a>() -> crate::model::Model<'a> {
    let mut model = new_empty_model();
    for row in 1..=5 {
        model._set(&format!("A{row}"), &(row * 10).to_string());
    }
    model.new_sheet();
    model
}

const AGGREGATIONS: [&str; 18] = [
    "SUM", "MIN", "MAX", "AVERAGE", "COUNT", "COUNTA", "PRODUCT", "AVERAGEA", "MAXA", "MINA",
    "STDEV.P", "STDEV.S", "VAR.P", "VAR.S", "STDEVA", "STDEVPA", "VARA", "VARPA",
];

#[test]
fn whole_column_matches_the_explicit_range() {
    let mut model = two_sheet_model();
    for (index, function) in AGGREGATIONS.iter().enumerate() {
        let row = index as i32 + 1;
        model._set(
            &format!("Sheet2!A{row}"),
            &format!("={function}(Sheet1!A:A)"),
        );
        model._set(
            &format!("Sheet2!B{row}"),
            &format!("={function}(Sheet1!A1:A5)"),
        );
    }
    model.evaluate();

    for (index, function) in AGGREGATIONS.iter().enumerate() {
        let row = index as i32 + 1;
        let clamped = model._get_text(&format!("Sheet2!A{row}"));
        let explicit = model._get_text(&format!("Sheet2!B{row}"));
        assert_eq!(
            clamped, explicit,
            "{function}: a whole-column reference disagreed with the range it stands for"
        );
    }
}

#[test]
fn counting_blanks_still_sees_the_whole_column() {
    // COUNTBLANK is the aggregation the clamp must not reach: the cells it
    // skips are exactly the ones this counts. It keeps its own full walk, and
    // this is what says so.
    let mut model = two_sheet_model();
    model._set("C1", "=COUNTBLANK(A1:A10)");
    model.evaluate();
    assert_eq!(model._get_text("C1"), "5");
}

#[test]
fn an_empty_column_still_aggregates() {
    // The clamp reads a sheet's dimension, and an empty sheet has none worth
    // speaking of. The bounds it produces must still be walkable rather than
    // inverted.
    let mut model = new_empty_model();
    model._set("C1", "=SUM(A:A)");
    model._set("C2", "=COUNT(A:A)");
    model._set("C3", "=MIN(A:A)");
    model.evaluate();
    assert_eq!(model._get_text("C1"), "0");
    assert_eq!(model._get_text("C2"), "0");
    assert_eq!(model._get_text("C3"), "0");
}
