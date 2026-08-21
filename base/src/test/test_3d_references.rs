#![allow(clippy::unwrap_used)]

//! Excel's 3-D references — `=SUM(Sheet1:Sheet3!A1)`.
//!
//! Before this, the arithmetic was already there: `=SUM(Sheet1!A1, Sheet2!A1,
//! Sheet3!A1)` answered correctly. What was missing was the *representation* —
//! a range in this engine lived on one sheet, so the span parsed as the range
//! operator applied to a bare name, round-tripped through `stringify`
//! unchanged, and answered `#VALUE!`.

use crate::test::util::new_empty_model;

/// Three sheets with 10, 20 and 30 in A1, and 1, 2, 3 in A2.
fn three_sheets<'a>() -> crate::model::Model<'a> {
    let mut model = new_empty_model();
    model.new_sheet();
    model.new_sheet();
    for (index, sheet) in ["Sheet1", "Sheet2", "Sheet3"].iter().enumerate() {
        model._set(
            &format!("{sheet}!A1"),
            &((index as i32 + 1) * 10).to_string(),
        );
        model._set(&format!("{sheet}!A2"), &(index + 1).to_string());
    }
    model
}

#[test]
fn a_span_of_sheets_sums() {
    let mut model = three_sheets();
    model._set("Sheet1!C1", "=SUM(Sheet1:Sheet3!A1)");
    // The expansion it must agree with.
    model._set("Sheet1!C2", "=SUM(Sheet1!A1,Sheet2!A1,Sheet3!A1)");
    model.evaluate();
    assert_eq!(model._get_text("Sheet1!C1"), "60");
    assert_eq!(model._get_text("Sheet1!C2"), "60");
}

#[test]
fn a_span_covers_a_range_on_every_sheet() {
    let mut model = three_sheets();
    model._set("Sheet1!C1", "=SUM(Sheet1:Sheet3!A1:A2)");
    model.evaluate();
    // (10 + 1) + (20 + 2) + (30 + 3)
    assert_eq!(model._get_text("Sheet1!C1"), "66");
}

#[test]
fn the_eighteen_documented_functions_accept_one() {
    // Excel names exactly these as taking a 3-D reference. Each is compared
    // against the same call over the flattened list of cells, so the assertion
    // is about the span and not about the function's own arithmetic.
    let mut model = three_sheets();
    let functions = [
        "SUM", "AVERAGE", "AVERAGEA", "COUNT", "COUNTA", "MAX", "MAXA", "MIN", "MINA", "PRODUCT",
        "STDEV.P", "STDEV.S", "STDEVA", "STDEVPA", "VAR.P", "VAR.S", "VARA", "VARPA",
    ];
    for (index, function) in functions.iter().enumerate() {
        let row = index as i32 + 1;
        model._set(
            &format!("Sheet1!D{row}"),
            &format!("={function}(Sheet1:Sheet3!A1)"),
        );
        model._set(
            &format!("Sheet1!E{row}"),
            &format!("={function}(Sheet1!A1,Sheet2!A1,Sheet3!A1)"),
        );
    }
    model.evaluate();
    for (index, function) in functions.iter().enumerate() {
        let row = index as i32 + 1;
        let spanned = model._get_text(&format!("Sheet1!D{row}"));
        let listed = model._get_text(&format!("Sheet1!E{row}"));
        assert_eq!(
            spanned, listed,
            "{function}: the 3-D reference disagreed with the cells written out"
        );
        assert!(
            !spanned.starts_with('#'),
            "{function}: a 3-D reference should be accepted, got {spanned}"
        );
    }
}

#[test]
fn everything_else_still_refuses_one() {
    // The other half of Excel's rule, and the reason `SheetSpan::Rejected`
    // exists: a function not on the list answers `#VALUE!` rather than
    // quietly aggregating across sheets.
    let mut model = three_sheets();
    model._set("Sheet1!C1", "=VLOOKUP(10,Sheet1:Sheet3!A1:A2,1)");
    model._set("Sheet1!C2", "=MEDIAN(Sheet1:Sheet3!A1)");
    model._set("Sheet1!C3", "=SUMSQ(Sheet1:Sheet3!A1)");
    model.evaluate();
    assert_eq!(model._get_text("Sheet1!C1"), "#VALUE!");
    assert_eq!(model._get_text("Sheet1!C2"), "#VALUE!");
    assert_eq!(model._get_text("Sheet1!C3"), "#VALUE!");
}

#[test]
fn the_formula_reads_back_as_written() {
    // A 3-D reference is written with both sheet names in front of one `!`.
    // Anything else and a workbook saved from here would not reopen the same.
    let mut model = three_sheets();
    model._set("Sheet1!C1", "=SUM(Sheet1:Sheet3!A1)");
    model._set("Sheet1!C2", "=SUM(Sheet1:Sheet3!A1:A2)");
    model._set("Sheet1!C3", "=SUM(Sheet2:Sheet3!$A$1)");
    model.evaluate();
    assert_eq!(model._get_formula("Sheet1!C1"), "=SUM(Sheet1:Sheet3!A1)");
    assert_eq!(model._get_formula("Sheet1!C2"), "=SUM(Sheet1:Sheet3!A1:A2)");
    assert_eq!(model._get_formula("Sheet1!C3"), "=SUM(Sheet2:Sheet3!$A$1)");
}

#[test]
fn a_defined_name_is_not_mistaken_for_a_sheet() {
    // `three_dimensional_range` fires only when the left name *is* a worksheet
    // and the right side was written with a sheet. Neither holds here, so this
    // must stay the ordinary range operator.
    let mut model = three_sheets();
    model._set("Sheet1!C1", "=SUM(A1:A2)");
    model.evaluate();
    assert_eq!(model._get_text("Sheet1!C1"), "11");
    assert_eq!(model._get_formula("Sheet1!C1"), "=SUM(A1:A2)");
}

#[test]
fn the_pre_2010_spellings_take_one_too() {
    // STDEV, STDEVP, VAR and VARP are aliases for four of the eighteen, and
    // Excel accepts a 3-D reference in them on the same terms.
    let mut model = three_sheets();
    for (index, function) in ["STDEV", "STDEVP", "VAR", "VARP"].iter().enumerate() {
        let row = index as i32 + 1;
        model._set(
            &format!("Sheet1!D{row}"),
            &format!("={function}(Sheet1:Sheet3!A1)"),
        );
        model._set(
            &format!("Sheet1!E{row}"),
            &format!("={function}(Sheet1!A1,Sheet2!A1,Sheet3!A1)"),
        );
    }
    model.evaluate();
    for (index, function) in ["STDEV", "STDEVP", "VAR", "VARP"].iter().enumerate() {
        let row = index as i32 + 1;
        let spanned = model._get_text(&format!("Sheet1!D{row}"));
        assert_eq!(
            spanned,
            model._get_text(&format!("Sheet1!E{row}")),
            "{function}: the 3-D reference disagreed with the cells written out"
        );
        assert!(!spanned.starts_with('#'), "{function}: got {spanned}");
    }
}

#[test]
fn a_span_written_backwards_still_reads_forwards() {
    // `Sheet3:Sheet1!A1` covers the same three sheets. `range_bounds` orders
    // the endpoints, and the formula keeps the order it was written in.
    let mut model = three_sheets();
    model._set("Sheet1!C1", "=SUM(Sheet3:Sheet1!A1)");
    model.evaluate();
    assert_eq!(model._get_text("Sheet1!C1"), "60");
    assert_eq!(model._get_formula("Sheet1!C1"), "=SUM(Sheet3:Sheet1!A1)");
}

#[test]
fn a_span_of_one_sheet_is_an_ordinary_range() {
    let mut model = three_sheets();
    model._set("Sheet1!C1", "=SUM(Sheet2:Sheet2!A1:A2)");
    model.evaluate();
    assert_eq!(model._get_text("Sheet1!C1"), "22");
}

#[test]
fn a_bare_span_outside_a_function_is_an_error() {
    // Excel answers `#VALUE!` for a 3-D reference used as a value. There is no
    // single cell it could mean.
    let mut model = three_sheets();
    model._set("Sheet1!C1", "=Sheet1:Sheet3!A1");
    model.evaluate();
    assert_eq!(model._get_text("Sheet1!C1"), "#VALUE!");
}

#[test]
fn the_range_operator_cannot_write_a_span() {
    // `Sheet1!A1:Sheet3!A2` is not how Excel writes a 3-D reference — both
    // sheet names go in front of one `!` — and it is worth knowing that this
    // spelling cannot reach the engine as a range at all. The parser refuses
    // it ("Expecting reference in range"), which is what makes the check in
    // `evaluate_function` sufficient: the only way to build a spanning range
    // is the node the parser produces for the real syntax.
    let mut model = three_sheets();
    model._set("Sheet1!C1", "=SUM(Sheet1!A1:Sheet3!A2)");
    model._set("Sheet1!C2", "=VLOOKUP(10,Sheet1!A1:Sheet3!A2,1)");
    model.evaluate();
    assert_eq!(model._get_text("Sheet1!C1"), "#ERROR!");
    assert_eq!(model._get_text("Sheet1!C2"), "#ERROR!");
}
