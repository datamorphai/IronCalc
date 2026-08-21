#![allow(clippy::unwrap_used)]

//! `AGGREGATE`, and mostly its options argument.
//!
//! The individual aggregations are already tested through their own functions.
//! What is worth testing here is the part that makes `AGGREGATE` worth having:
//! that it can be told to skip error values, which is how a spreadsheet totals a
//! column with one `#DIV/0!` in it, and that it refuses when told to skip
//! nothing.

use crate::test::util::new_empty_model;

/// 1, 2, 3, 4 in A1:A4, with an error in A5.
fn model_with_error() -> crate::model::Model<'static> {
    let mut model = new_empty_model();
    for (index, value) in ["1", "2", "3", "4"].iter().enumerate() {
        model._set(&format!("A{}", index + 1), value);
    }
    model._set("A5", "=1/0");
    model
}

#[test]
fn sums_ignoring_errors() {
    // The reason the function exists. Option 6 is "ignore error values", and
    // without it a single #DIV/0! makes the whole total an error.
    let mut model = model_with_error();
    model._set("C1", "=AGGREGATE(9,6,A1:A5)");
    model._set("C2", "=AGGREGATE(9,4,A1:A5)");
    model._set("C3", "=SUM(A1:A5)");
    model.evaluate();

    assert_eq!(model._get_text("C1"), "10");
    // Option 4 is "ignore nothing", so the error propagates — as it must, or the
    // options argument would not mean anything.
    assert_eq!(model._get_text("C2"), "#DIV/0!");
    assert_eq!(model._get_text("C3"), "#DIV/0!");
}

#[test]
fn every_option_that_ignores_errors_does() {
    let mut model = model_with_error();
    for (index, options) in [2, 3, 6, 7].iter().enumerate() {
        model._set(
            &format!("C{}", index + 1),
            &format!("=AGGREGATE(9,{options},A1:A5)"),
        );
    }
    // And every option that does not, does not.
    for (index, options) in [0, 1, 4, 5].iter().enumerate() {
        model._set(
            &format!("D{}", index + 1),
            &format!("=AGGREGATE(9,{options},A1:A5)"),
        );
    }
    model.evaluate();

    for row in 1..=4 {
        assert_eq!(model._get_text(&format!("C{row}")), "10", "row {row}");
        assert_eq!(model._get_text(&format!("D{row}")), "#DIV/0!", "row {row}");
    }
}

#[test]
fn the_aggregations() {
    let mut model = new_empty_model();
    for (index, value) in ["1", "2", "3", "4"].iter().enumerate() {
        model._set(&format!("A{}", index + 1), value);
    }
    model._set("A5", "text");

    let cases = [
        ("=AGGREGATE(1,6,A1:A5)", "2.5"),  // AVERAGE
        ("=AGGREGATE(2,6,A1:A5)", "4"),    // COUNT — numbers only
        ("=AGGREGATE(3,6,A1:A5)", "5"),    // COUNTA — text included
        ("=AGGREGATE(4,6,A1:A5)", "4"),    // MAX
        ("=AGGREGATE(5,6,A1:A5)", "1"),    // MIN
        ("=AGGREGATE(6,6,A1:A5)", "24"),   // PRODUCT
        ("=AGGREGATE(9,6,A1:A5)", "10"),   // SUM
        ("=AGGREGATE(12,6,A1:A5)", "2.5"), // MEDIAN
    ];
    for (index, (formula, _)) in cases.iter().enumerate() {
        model._set(&format!("C{}", index + 1), formula);
    }
    model.evaluate();

    for (index, (formula, expected)) in cases.iter().enumerate() {
        assert_eq!(
            model._get_text(&format!("C{}", index + 1)),
            *expected,
            "{formula}"
        );
    }
}

#[test]
fn the_forms_that_take_a_k() {
    let mut model = new_empty_model();
    for (index, value) in ["10", "20", "30", "40"].iter().enumerate() {
        model._set(&format!("A{}", index + 1), value);
    }
    model._set("C1", "=AGGREGATE(14,6,A1:A4,2)"); // LARGE, 2nd
    model._set("C2", "=AGGREGATE(15,6,A1:A4,2)"); // SMALL, 2nd
    model._set("C3", "=AGGREGATE(16,6,A1:A4,0.5)"); // PERCENTILE.INC
    model._set("C4", "=AGGREGATE(17,6,A1:A4,2)"); // QUARTILE.INC, the median
    model.evaluate();

    assert_eq!(model._get_text("C1"), "30");
    assert_eq!(model._get_text("C2"), "20");
    assert_eq!(model._get_text("C3"), "25");
    // The second quartile is the median, which is the check that quartile is
    // percentile with the argument quartered rather than a separate rule.
    assert_eq!(model._get_text("C4"), "25");
}

#[test]
fn refuses_what_it_should() {
    let mut model = new_empty_model();
    for (index, value) in ["1", "2", "3"].iter().enumerate() {
        model._set(&format!("A{}", index + 1), value);
    }
    model._set("C1", "=AGGREGATE(20,6,A1:A3)"); // no such function
    model._set("C2", "=AGGREGATE(9,8,A1:A3)"); // no such option
    model._set("C3", "=AGGREGATE(14,6,A1:A3,9)"); // k past the end
    model._set("C4", "=AGGREGATE(9,6)"); // no range at all
    model._set("C5", "=AGGREGATE(16,6,A1:A3,2)"); // percentile outside 0..1
    model.evaluate();

    assert_eq!(model._get_text("C1"), "#VALUE!");
    assert_eq!(model._get_text("C2"), "#VALUE!");
    assert_eq!(model._get_text("C3"), "#NUM!");
    assert_eq!(model._get_text("C4"), "#ERROR!");
    assert_eq!(model._get_text("C5"), "#NUM!");
}

#[test]
fn ignores_a_nested_aggregate() {
    /*
     * The behaviour that makes a column of subtotals summable. A range holding
     * its own AGGREGATE would otherwise be counted twice — once as the rows and
     * once as their total.
     */
    let mut model = new_empty_model();
    for (index, value) in ["1", "2", "3"].iter().enumerate() {
        model._set(&format!("A{}", index + 1), value);
    }
    model._set("A4", "=AGGREGATE(9,2,A1:A3)"); // 6, a subtotal within the range
    model._set("C1", "=AGGREGATE(9,2,A1:A4)");
    model._set("C2", "=AGGREGATE(9,6,A1:A4)");
    model.evaluate();

    /*
     * Options 2 and 6 both ignore errors and differ *only* in whether they
     * ignore nested aggregates, which is what makes this pair the test: 2 does
     * and 6 does not.
     *
     * The first version of this test used 6 for both and expected 6, from
     * misreading the option table — 0-3 ignore nested and 4-7 do not, while
     * hidden rows and errors alternate within each half. It is not a bit
     * pattern, which is exactly why it is worth a test rather than a glance.
     */
    assert_eq!(model._get_text("C1"), "6");
    assert_eq!(model._get_text("C2"), "12");
}

#[test]
fn keeps_its_own_name() {
    /*
     * A function's name is looked up through the localised table, and this one
     * was first registered pointing at SUBTOTAL's entry — copied from the line
     * above it. Two variants then claimed the same string, so an AGGREGATE
     * formula came back as a SUBTOTAL one and, worse, a SUBTOTAL read from a
     * file stopped round-tripping at all.
     *
     * Nothing in the engine's own tests saw it, because they set formulas and
     * read values rather than reading the formula back. It surfaced as an Excel
     * import losing a SUBTOTAL, two layers away from the cause.
     */
    let mut model = new_empty_model();
    model._set("B2", "5");
    model._set("B3", "7");
    model._set("A1", "=AGGREGATE(9,6,B2:B3)");
    model._set("A2", "=SUBTOTAL(9,B2:B3)");
    model.evaluate();

    assert_eq!(model._get_formula("A1"), "=AGGREGATE(9,6,B2:B3)");
    assert_eq!(model._get_formula("A2"), "=SUBTOTAL(9,B2:B3)");
    assert_eq!(model._get_text("A1"), "12");
    assert_eq!(model._get_text("A2"), "12");
}
