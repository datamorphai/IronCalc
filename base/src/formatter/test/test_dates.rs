#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use crate::{
    formatter::format::format_number,
    locale::{get_locale, Locale},
};

fn get_default_locale() -> &'static Locale {
    get_locale("de").unwrap()
}

#[test]
fn simple_test() {
    let locale = get_default_locale();
    let b = format_number(46015.0, "m.d.yy", locale);
    assert_eq!(b.text, "12.24.25");
}

// A locale bracket with no currency symbol, and Excel's other minute rule.

#[test]
fn a_bare_locale_bracket_renders() {
    /*
     * Excel writes `[$-409]` onto date formats as a matter of routine, so the
     * bracket carrying only a locale id is the ordinary case rather than an
     * exotic one — it was the *decorated* form, `[$€-2]`, that worked.
     *
     * It says nothing about how to render, so it is dropped: the locale part of
     * a currency bracket was already ignored, and this is the same bracket
     * without the symbol.
     */
    // This file's `get_default_locale` is German, so the number cases name the
    // English one explicitly rather than carrying separators nobody reading the
    // assertion would predict.
    let de = get_default_locale();
    let en = get_locale("en").unwrap();
    let v = 45658.0;

    assert_eq!(format_number(v, "[$-409]d-mmm-yy", de).text, "1-Jan-25");
    assert_eq!(
        format_number(v, "[$-en-US]d/mmm/yyyy", de).text,
        "1/Jan/2025"
    );
    assert_eq!(
        format_number(1234.5, "[$-409]#,##0.00", en).text,
        "1,234.50"
    );
    // The decorated form still carries its symbol, which is what worked before.
    assert_eq!(
        format_number(1234.5, "[$$-409]#,##0.00", en).text,
        "$1,234.50"
    );
    // And the locale bracket does not change the separators — it is dropped,
    // not honoured, which is the same thing the currency form's locale does.
    assert_eq!(
        format_number(1234.5, "[$-409]#,##0.00", de).text,
        "1.234,50"
    );
}

#[test]
fn m_before_seconds_is_a_minute() {
    /*
     * The rule has two halves and only one could be applied reading left to
     * right: an `m` after an `h` is a minute, which is known at the time, and an
     * `m` before an `s` is also a minute, which is not — the `s` has not arrived
     * and a separator sits between them.
     *
     * Without the second half `mm:ss` renders the month and the seconds:
     * `01:15` for half past twelve on the first of January, which is a
     * plausible-looking time and the wrong one.
     */
    let locale = get_default_locale();
    let v = 45658.0 + (12.0 * 3600.0 + 30.0 * 60.0 + 15.0) / 86400.0;

    assert_eq!(format_number(v, "mm:ss", locale).text, "30:15");
    assert_eq!(format_number(v, "m:s", locale).text, "30:15");
    // The half that already worked.
    assert_eq!(format_number(v, "h:mm:ss", locale).text, "12:30:15");

    // And a month is still a month when no seconds follow it.
    assert_eq!(format_number(v, "m/d/yyyy", locale).text, "1/1/2025");
    assert_eq!(format_number(v, "mmm-yy", locale).text, "Jan-25");
    assert_eq!(format_number(v, "mm", locale).text, "01");
}
