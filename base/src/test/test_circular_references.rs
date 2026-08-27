#![allow(clippy::unwrap_used)]

use crate::test::util::new_empty_model;

#[test]
fn test_simple_circ() {
    let mut model = new_empty_model();
    model._set("A1", "=A1+1");
    model.evaluate();
    assert_eq!(model._get_text("A1"), "#CIRC!");
}

#[test]
fn test_simple_circ_propagate() {
    let mut model = new_empty_model();
    model._set("A1", "=B6");
    model._set("A2", "=A1+1");
    model._set("A3", "=A2+1");
    model._set("A4", "=A3+5");
    model._set("B6", "=A4*7");
    model.evaluate();
    assert_eq!(model._get_text("A1"), "#CIRC!");
    assert_eq!(model._get_text("A2"), "#CIRC!");
    assert_eq!(model._get_text("A3"), "#CIRC!");
    assert_eq!(model._get_text("A4"), "#CIRC!");
    assert_eq!(model._get_text("B6"), "#CIRC!");
}

/*
 * Iterative calculation (morphbook §4.2).
 *
 * Excel ships this switched off, so the two tests above are what an
 * unconfigured workbook does and must keep doing. What follows is what happens
 * when the user turns it on, which is the incompatibility §4.2 named: refusing
 * every circular reference is right by default and wrong as the only option.
 */

#[test]
fn iteration_is_off_until_it_is_asked_for() {
    let mut model = new_empty_model();
    model._set("A1", "=A1+1");
    // Enabled, then disabled again: the flag is read at evaluation time rather
    // than latched when the cycle was first seen.
    model.set_iterative_calculation(true, 100, 0.001);
    model.set_iterative_calculation(false, 100, 0.001);
    model.evaluate();
    assert_eq!(model._get_text("A1"), "#CIRC!");
}

#[test]
fn a_cycle_converges_to_its_fixed_point() {
    let mut model = new_empty_model();
    // x = x/2 + 5 settles at 10 from any starting value.
    model._set("A1", "=A1/2+5");
    model.set_iterative_calculation(true, 100, 0.000_001);
    model.evaluate();

    let value: f64 = model._get_text("A1").parse().unwrap();
    assert!((value - 10.0).abs() < 0.001, "settled at {value}, not 10");
}

#[test]
fn it_starts_from_zero_rather_than_from_the_error() {
    let mut model = new_empty_model();
    model._set("A1", "=A1+1");

    // Evaluated once with iteration off, so A1 now holds `#CIRC!`.
    model.evaluate();
    assert_eq!(model._get_text("A1"), "#CIRC!");

    // Switching iteration on must escape that: reading the leftover error as
    // the previous value would propagate it forever and the cell could never
    // leave the state it was in.
    model.set_iterative_calculation(true, 10, 0.001);
    model.evaluate();
    assert_eq!(model._get_text("A1"), "10");
}

#[test]
fn a_divergent_cycle_stops_at_the_cap() {
    let mut model = new_empty_model();
    // Never settles; the cap is the only thing that ends it.
    model._set("A1", "=A1+1");
    model.set_iterative_calculation(true, 7, 0.001);
    model.evaluate();
    // Seven passes, each adding one, starting from zero.
    assert_eq!(model._get_text("A1"), "7");
}

#[test]
fn the_threshold_decides_when_to_stop() {
    let mut model = new_empty_model();
    model._set("A1", "=A1/2+5");

    // Coarse: stops early, short of the fixed point.
    model.set_iterative_calculation(true, 100, 1.0);
    model.evaluate();
    let coarse: f64 = model._get_text("A1").parse().unwrap();

    let mut fine_model = new_empty_model();
    fine_model._set("A1", "=A1/2+5");
    fine_model.set_iterative_calculation(true, 100, 0.000_001);
    fine_model.evaluate();
    let fine: f64 = fine_model._get_text("A1").parse().unwrap();

    assert!(coarse < fine, "a coarser threshold stopped no earlier: {coarse} vs {fine}");
    assert!((fine - 10.0).abs() < 0.001);
}

#[test]
fn a_cycle_through_several_cells_converges_too() {
    let mut model = new_empty_model();
    // The shape §4.2 cites: a balance that depends on interest that depends on
    // the balance.
    model._set("A1", "1000");           // opening
    model._set("A2", "=A1+A3/2");       // average balance
    model._set("A3", "=A2*0.1");        // interest on it
    model.set_iterative_calculation(true, 200, 0.000_001);
    model.evaluate();

    let average: f64 = model._get_text("A2").parse().unwrap();
    let interest: f64 = model._get_text("A3").parse().unwrap();
    // A2 = 1000 + (A2*0.1)/2  →  A2 = 1000/0.95
    assert!((average - 1000.0 / 0.95).abs() < 0.001, "average was {average}");
    assert!((interest - average * 0.1).abs() < 0.001);
}

#[test]
fn a_workbook_with_no_cycle_is_unaffected() {
    let mut model = new_empty_model();
    model._set("A1", "2");
    model._set("A2", "=A1*3");
    model.set_iterative_calculation(true, 100, 0.001);
    model.evaluate();
    assert_eq!(model._get_text("A2"), "6");
}
