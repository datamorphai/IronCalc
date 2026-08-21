//! One place that decides which cells a range covers.
//!
//! Every function that aggregates over a range used to spell the walk out for
//! itself: reject a range whose endpoints disagree about the sheet, then loop
//! rows inside columns. Forty-one copies of that, and `fn_sum` carried a note
//! from upstream saying so —
//!
//! ```text
//! // TODO: We should do this for all functions that run through ranges
//! // Running cargo test for the ironcalc takes around .8 seconds with this
//! // speedup and ~ 3.5 seconds without it.
//! ```
//!
//! — because `fn_sum` alone clamped `A:A` to the rows a sheet actually uses,
//! and the note recorded that every other walker scans a million empty rows.
//! The two problems have one answer: resolve a range to its bounds in one
//! place, and both the clamp and the sheet span reach every caller that asks.
//!
//! [`Model::range_bounds`] deliberately returns bounds rather than taking a
//! closure over the cells. A closure reads better, but adopting it would mean
//! rewriting each caller's `match` arms — turning `return error` into
//! `Err(error)` and adding `Ok(())` to the arms that fall through — and those
//! arms are where `MIN` differs from `AVERAGE` in what it ignores. Handing back
//! bounds lets every arm stay byte-for-byte what it was, so the diff cannot
//! change which values an aggregation counts. In a calculation engine that is
//! worth more than the nicer signature: the failure mode of the other choice is
//! a wrong number rather than an error.
//!
//! # Which functions may span sheets
//!
//! Excel does not accept a 3-D reference everywhere. It documents eighteen
//! functions that take one — `SUM`, `AVERAGE`, `AVERAGEA`, `COUNT`, `COUNTA`,
//! `MAX`, `MAXA`, `MIN`, `MINA`, `PRODUCT`, the four `STDEV` spellings and the
//! four `VAR` spellings — and everything else answers `#VALUE!`:
//! `=VLOOKUP(x, Sheet1:Sheet3!A:B, 2)` is an error in Excel, and
//! `=SUMPRODUCT(Sheet1:Sheet3!A1:A9, B1:B9)` is too.
//!
//! So the guard the hand-written walkers carry is not a limitation to be swept
//! away wherever it appears — in most of them it *is* the Excel behaviour. It
//! becomes an argument instead. A caller passing [`SheetSpan::Allowed`] is
//! saying it is on that list; a caller passing [`SheetSpan::Rejected`] is
//! saying it is not, and gets the same error it raised for itself before. Both
//! get the clamp.

use crate::{
    calc_result::CalcResult,
    constants::{LAST_COLUMN, LAST_ROW},
    expressions::{token::Error, types::CellReferenceIndex},
    model::Model,
};

/// Whether this caller is one of the functions Excel lets a 3-D reference into.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SheetSpan {
    /// On Excel's list: walk every sheet from the first endpoint to the second.
    Allowed,
    /// Not on it: a range whose endpoints disagree about the sheet is `#VALUE!`.
    Rejected,
}

/// The cells a range covers, after clamping and across every sheet it spans.
///
/// Inclusive on both ends, in the order the loops want them.
pub(crate) struct RangeBounds {
    pub sheet1: u32,
    pub sheet2: u32,
    pub row1: i32,
    pub row2: i32,
    pub column1: i32,
    pub column2: i32,
}

impl Model<'_> {
    /// Resolve a range to the bounds worth walking.
    ///
    /// `A1:A5` comes back unchanged. `A:A` — which parses as rows 1 to
    /// `LAST_ROW` — comes back clamped to the last row the sheet actually uses,
    /// which is the difference between reading five cells and a million. The
    /// clamp applies only to a whole column or a whole row, because those are
    /// the cases where the stated bound is a maximum rather than a number
    /// somebody chose.
    ///
    /// Across a sheet span the clamp takes the widest sheet, not the first: a
    /// span whose second sheet is the long one must still reach its last row.
    pub(crate) fn range_bounds(
        &self,
        left: &CellReferenceIndex,
        right: &CellReferenceIndex,
        cell: CellReferenceIndex,
        span: SheetSpan,
    ) -> Result<RangeBounds, CalcResult> {
        if left.sheet != right.sheet && span == SheetSpan::Rejected {
            return Err(CalcResult::new_error(
                Error::VALUE,
                cell,
                "Ranges are in different sheets".to_string(),
            ));
        }

        let (sheet1, sheet2) = if left.sheet <= right.sheet {
            (left.sheet, right.sheet)
        } else {
            (right.sheet, left.sheet)
        };

        let row1 = left.row;
        let mut row2 = right.row;
        let column1 = left.column;
        let mut column2 = right.column;

        let whole_column = row1 == 1 && row2 == LAST_ROW;
        let whole_row = column1 == 1 && column2 == LAST_COLUMN;

        if whole_column || whole_row {
            let mut max_row = 1;
            let mut max_column = 1;
            for sheet in sheet1..=sheet2 {
                let worksheet = match self.workbook.worksheet(sheet) {
                    Ok(worksheet) => worksheet,
                    Err(_) => {
                        return Err(CalcResult::new_error(
                            Error::ERROR,
                            cell,
                            format!("Invalid worksheet index: '{sheet}'"),
                        ));
                    }
                };
                let dimension = worksheet.dimension();
                max_row = max_row.max(dimension.max_row);
                max_column = max_column.max(dimension.max_column);
            }
            if whole_column {
                row2 = max_row;
            }
            if whole_row {
                column2 = max_column;
            }
        }

        Ok(RangeBounds {
            sheet1,
            sheet2,
            row1,
            row2,
            column1,
            column2,
        })
    }
}
