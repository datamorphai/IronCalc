#![allow(clippy::unwrap_used)]

use crate::{
    formatter::format::format_number,
    locale::{get_locale, Locale},
};

fn get_default_locale() -> &'static Locale {
    get_locale("en").unwrap()
}

#[test]
fn simple_test() {
    let locale = get_default_locale();
    let format = "h:mm AM/PM";
    let value = 16.001_423_611_111_11; // =1/86400 => 12:02 AM
    let formatted = format_number(value, format, locale);
    assert_eq!(formatted.text, "12:02 AM");
}

#[test]
fn padded_vs_unpadded() {
    let locale = get_default_locale();
    let padded_format = "hh:mm:ss AM/PM";
    let unpadded_format = "h:m:s AM/PM";
    let value = 0.25351851851851853; // => 6:05:04 AM (21904/(24*60*60)) where 21904 = 6 * 3600 + 5*60 + 4
    let formatted = format_number(value, padded_format, locale);
    assert_eq!(formatted.text, "06:05:04 AM");

    let formatted = format_number(value, unpadded_format, locale);
    assert_eq!(formatted.text, "6:5:4 AM");
}

#[test]
fn elapsed_hour() {
    let locale = get_default_locale();
    let format = "[hh]:mm:ss";
    // 1.5 days = 36 hours
    let value = 1.5;
    let formatted = format_number(value, format, locale);
    assert_eq!(formatted.text, "36:00:00");
}

#[test]
fn elapsed_minute() {
    let locale = get_default_locale();
    let format = "[mm]:ss";
    // 2 days = 2880 minutes
    let value = 2.0;
    let formatted = format_number(value, format, locale);
    assert_eq!(formatted.text, "2880:00");
}

#[test]
fn elapsed_second() {
    let locale = get_default_locale();
    let format = "[ss]";
    // 0.5 days = 43200 seconds
    let value = 0.5;
    let formatted = format_number(value, format, locale);
    assert_eq!(formatted.text, "43200");
}

#[test]
fn elapsed_hour_padded() {
    let locale = get_default_locale();
    let format = "[hh]:mm:ss";
    // 0.1 days = 2.4 hours
    let value = 0.1;
    let formatted = format_number(value, format, locale);
    assert_eq!(formatted.text, "02:24:00");
}

// Sub-second precision: Excel's `mm:ss.0` family, of which `mmss.0` is its own
// built-in format id 47 — a file does not even spell that one out.

#[test]
fn sub_second_precision() {
    let en = get_locale("en").unwrap();
    let at = |h: f64, m: f64, s: f64| 45658.0 + (h * 3600.0 + m * 60.0 + s) / 86400.0;
    let v = at(12.0, 30.0, 15.678);

    assert_eq!(format_number(v, "mm:ss.0", en).text, "30:15.7");
    assert_eq!(format_number(v, "mm:ss.00", en).text, "30:15.68");
    assert_eq!(format_number(v, "h:mm:ss.0", en).text, "12:30:15.7");
    // Excel's built-in id 47.
    assert_eq!(format_number(v, "mmss.0", en).text, "3015.7");
    // A whole second still shows its zero, because the format asked for a digit.
    assert_eq!(
        format_number(at(12.0, 30.0, 15.0), "mm:ss.0", en).text,
        "30:15.0"
    );
}

#[test]
fn sub_second_rounding_carries() {
    /*
     * The reason the whole time is rounded once and then split, rather than the
     * seconds being rounded on their own.
     *
     * 59.98 seconds shown to one decimal is 60.0, which is the next minute.
     * Rounding the seconds by themselves produces `:60`, a time that does not
     * exist — and it is the shape of a bug that reads as plausible, since only
     * the last two characters are wrong.
     */
    let en = get_locale("en").unwrap();
    let at = |h: f64, m: f64, s: f64| 45658.0 + (h * 3600.0 + m * 60.0 + s) / 86400.0;

    assert_eq!(
        format_number(at(12.0, 30.0, 59.98), "mm:ss.0", en).text,
        "31:00.0"
    );
    assert_eq!(
        format_number(at(12.0, 59.0, 59.98), "h:mm:ss.0", en).text,
        "13:00:00.0"
    );
    // And a time that rounds up past midnight wraps rather than showing a
    // twenty-fourth hour.
    assert_eq!(
        format_number(at(23.0, 59.0, 59.99), "h:mm:ss.0", en).text,
        "0:00:00.0"
    );
}

#[test]
fn a_format_without_a_fraction_is_unchanged() {
    // The codes that already worked have to keep working: adding sub-second
    // support must not change what a whole-second format renders.
    let en = get_locale("en").unwrap();
    let at = |h: f64, m: f64, s: f64| 45658.0 + (h * 3600.0 + m * 60.0 + s) / 86400.0;
    let v = at(12.0, 30.0, 15.0);

    assert_eq!(format_number(v, "mm:ss", en).text, "30:15");
    assert_eq!(format_number(v, "h:mm:ss", en).text, "12:30:15");
    assert_eq!(format_number(v, "hh:mm", en).text, "12:30");
}

#[test]
fn elapsed_seconds_do_not_saturate() {
    /*
     * `[s]` cast to i32, and a date serial's elapsed seconds exceed it — 2025 is
     * about 3.9 billion of them, so every such format rendered 2147483647.
     * Found while probing the sub-second codes, and worse than the codes that
     * were on the list: a saturated number looks like a number.
     */
    let en = get_locale("en").unwrap();
    let text = format_number(45658.5, "[s]", en).text;
    assert_ne!(text, "2147483647");
    assert_eq!(text, "3944894400");
}
