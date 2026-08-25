//! `AGGREGATE`, Excel's successor to `SUBTOTAL`.
//!
//! Two things make it more than a bigger `SUBTOTAL`. It takes an *options*
//! argument saying what to skip — hidden rows, error values, nested aggregates,
//! in any combination — and six of its nineteen functions take a `k`, so
//! `AGGREGATE(14, 6, A1:A99, 2)` is "the second largest, ignoring errors".
//!
//! The options are the point of the function. A column with one `#DIV/0!` in it
//! makes every ordinary `SUM` over it an error, and `AGGREGATE(9, 6, ...)` is
//! how a spreadsheet says "total this anyway". That is why it is on §4.1's
//! must-have list while its individual aggregations are already there
//! separately.

use std::cmp::Ordering;

use crate::{
    calc_result::CalcResult,
    expressions::{parser::Node, token::Error, types::CellReferenceIndex},
    functions::{subtotal::CellTableStatus, Function},
    model::Model,
};

/// What an options argument says to leave out.
///
/// Excel encodes these as 0-7 rather than as flags, and the mapping is not a bit
/// pattern anybody would guess: 0-3 ignore nested aggregates and 4-7 do not,
/// while hidden rows and errors alternate within each half.
struct Ignoring {
    hidden: bool,
    errors: bool,
    nested: bool,
}

impl Ignoring {
    fn from_options(options: i32) -> Option<Ignoring> {
        if !(0..=7).contains(&options) {
            return None;
        }
        Some(Ignoring {
            hidden: matches!(options, 1 | 3 | 5 | 7),
            errors: matches!(options, 2 | 3 | 6 | 7),
            nested: options <= 3,
        })
    }
}

/// The numbers a call collected, and how many non-empty cells it saw.
///
/// `COUNTA` counts things that are not numbers, so the count cannot be derived
/// from the values afterwards — it has to be kept while walking.
struct Collected {
    numbers: Vec<f64>,
    non_empty: usize,
}

impl<'a> Model<'a> {
    pub(crate) fn fn_aggregate(&mut self, args: &[Node], cell: CellReferenceIndex) -> CalcResult {
        if args.len() < 3 {
            return CalcResult::new_args_number_error(cell);
        }
        let function_number = match self.get_number(&args[0], cell) {
            Ok(f) => f.trunc() as i32,
            Err(s) => return s,
        };
        let options = match self.get_number(&args[1], cell) {
            Ok(f) => f.trunc() as i32,
            Err(s) => return s,
        };
        let Some(ignoring) = Ignoring::from_options(options) else {
            return CalcResult::new_error(
                Error::VALUE,
                cell,
                format!("Invalid options for AGGREGATE: {options}"),
            );
        };

        // 14 to 19 take a `k` after the array — LARGE, SMALL and the percentile
        // family. The rest take any number of ranges, as SUBTOTAL does.
        let takes_k = (14..=19).contains(&function_number);
        let (value_args, k) = if takes_k {
            if args.len() != 4 {
                return CalcResult::new_args_number_error(cell);
            }
            match self.get_number(&args[3], cell) {
                Ok(f) => (&args[2..3], Some(f)),
                Err(s) => return s,
            }
        } else {
            (&args[2..], None)
        };

        let collected = match self.aggregate_get_values(value_args, cell, &ignoring) {
            Ok(c) => c,
            Err(e) => return e,
        };
        let values = collected.numbers;

        let number_error = |message: &str| CalcResult::Error {
            error: Error::NUM,
            origin: cell,
            message: message.to_string(),
        };

        match function_number {
            1 => {
                if values.is_empty() {
                    return number_error("AGGREGATE(1) needs a number");
                }
                CalcResult::Number(values.iter().sum::<f64>() / values.len() as f64)
            }
            2 => CalcResult::Number(values.len() as f64),
            3 => CalcResult::Number(collected.non_empty as f64),
            4 => match values.iter().copied().reduce(f64::max) {
                // Excel's MAX of nothing is 0, not an error, and AGGREGATE
                // follows it.
                Some(v) => CalcResult::Number(v),
                None => CalcResult::Number(0.0),
            },
            5 => match values.iter().copied().reduce(f64::min) {
                Some(v) => CalcResult::Number(v),
                None => CalcResult::Number(0.0),
            },
            6 => CalcResult::Number(values.iter().product()),
            7 | 8 | 10 | 11 => {
                // The sample forms need two values; the population forms need
                // one. Excel is #DIV/0! rather than #NUM! for both.
                let n = values.len();
                let sample = matches!(function_number, 7 | 10);
                if (sample && n < 2) || n == 0 {
                    return CalcResult::Error {
                        error: Error::DIV,
                        origin: cell,
                        message: "Not enough values".to_string(),
                    };
                }
                let mean = values.iter().sum::<f64>() / n as f64;
                let sum_squares: f64 = values.iter().map(|v| (v - mean) * (v - mean)).sum();
                let divisor = if sample { n as f64 - 1.0 } else { n as f64 };
                let variance = sum_squares / divisor;
                CalcResult::Number(if matches!(function_number, 7 | 8) {
                    variance.sqrt()
                } else {
                    variance
                })
            }
            9 => CalcResult::Number(values.iter().sum()),
            12 => match median(&values) {
                Some(v) => CalcResult::Number(v),
                None => number_error("AGGREGATE(12) needs a number"),
            },
            13 => match mode(&values) {
                Some(v) => CalcResult::Number(v),
                // Excel's MODE.SNGL is #N/A when nothing repeats, which is a
                // different thing from an empty range.
                None => CalcResult::Error {
                    error: Error::NA,
                    origin: cell,
                    message: "No repeated value".to_string(),
                },
            },
            14 | 15 => {
                let Some(k) = k else {
                    return CalcResult::new_args_number_error(cell);
                };
                let rank = k.trunc() as i64;
                if rank < 1 || rank as usize > values.len() {
                    return number_error("k is out of range");
                }
                let mut sorted = values.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
                let index = if function_number == 14 {
                    sorted.len() - rank as usize
                } else {
                    rank as usize - 1
                };
                CalcResult::Number(sorted[index])
            }
            16..=19 => {
                let Some(k) = k else {
                    return CalcResult::new_args_number_error(cell);
                };
                // The quartile forms are the percentile forms with the argument
                // quartered, which is exactly how Excel defines them.
                let p = if matches!(function_number, 17 | 19) {
                    if !(0.0..=4.0).contains(&k) {
                        return number_error("Quartile must be between 0 and 4");
                    }
                    k / 4.0
                } else {
                    k
                };
                let inclusive = matches!(function_number, 16 | 17);
                match percentile(&values, p, inclusive) {
                    Some(v) => CalcResult::Number(v),
                    None => number_error("Percentile is out of range"),
                }
            }
            _ => CalcResult::new_error(
                Error::VALUE,
                cell,
                format!("Invalid function for AGGREGATE: {function_number}"),
            ),
        }
    }

    /// Walks the arguments, honouring what the options said to skip.
    ///
    /// Modelled on `subtotal_get_values`, and differing where `AGGREGATE`
    /// differs: errors can be skipped rather than propagated, and a nested
    /// `AGGREGATE` is skipped alongside a nested `SUBTOTAL`.
    fn aggregate_get_values(
        &mut self,
        args: &[Node],
        cell: CellReferenceIndex,
        ignoring: &Ignoring,
    ) -> Result<Collected, CalcResult> {
        let mut collected = Collected {
            numbers: Vec::new(),
            non_empty: 0,
        };

        for arg in args {
            if ignoring.nested
                && matches!(
                    arg,
                    Node::FunctionKind {
                        kind: Function::Subtotal | Function::Aggregate,
                        args: _,
                    }
                )
            {
                continue;
            }

            match self.evaluate_node_with_reference(arg, cell) {
                CalcResult::String(_) | CalcResult::Boolean(_) => {
                    // Counted by COUNTA, ignored by everything else — the same
                    // rule the other aggregations follow.
                    collected.non_empty += 1;
                }
                CalcResult::Number(f) => {
                    collected.numbers.push(f);
                    collected.non_empty += 1;
                }
                error @ CalcResult::Error { .. } => {
                    if !ignoring.errors {
                        return Err(error);
                    }
                    collected.non_empty += 1;
                }
                CalcResult::Range { left, right } => {
                    if left.sheet != right.sheet {
                        return Err(CalcResult::new_error(
                            Error::VALUE,
                            cell,
                            "Ranges are in different sheets".to_string(),
                        ));
                    }
                    for row in left.row..=right.row {
                        let status = self
                            .cell_hidden_status(left.sheet, row, left.column)
                            .map_err(|message| {
                                CalcResult::new_error(Error::ERROR, cell, message)
                            })?;
                        // A filtered row is out regardless: filtering is a
                        // statement about which rows the sheet is *about*,
                        // where hiding is only a statement about the view.
                        if status == CellTableStatus::Filtered {
                            continue;
                        }
                        if ignoring.hidden && status == CellTableStatus::Hidden {
                            continue;
                        }
                        for column in left.column..=right.column {
                            if ignoring.nested && self.cell_is_subtotal(left.sheet, row, column) {
                                continue;
                            }
                            match self.evaluate_cell(CellReferenceIndex {
                                sheet: left.sheet,
                                row,
                                column,
                            }) {
                                CalcResult::Number(value) => {
                                    collected.numbers.push(value);
                                    collected.non_empty += 1;
                                }
                                error @ CalcResult::Error { .. } => {
                                    if !ignoring.errors {
                                        return Err(error);
                                    }
                                    collected.non_empty += 1;
                                }
                                CalcResult::EmptyCell | CalcResult::EmptyArg => {}
                                _ => {
                                    // Text and booleans: COUNTA sees them, the
                                    // arithmetic does not.
                                    collected.non_empty += 1;
                                }
                            }
                        }
                    }
                }
                CalcResult::EmptyCell | CalcResult::EmptyArg => {}
                CalcResult::Array(_) | CalcResult::Lambda(_) => {
                    return Err(CalcResult::Error {
                        error: Error::NIMPL,
                        origin: cell,
                        message: "Arrays not supported yet".to_string(),
                    })
                }
            }
        }

        Ok(collected)
    }
}

fn sorted_copy(values: &[f64]) -> Vec<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    sorted
}

fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let sorted = sorted_copy(values);
    let n = sorted.len();
    if n % 2 == 1 {
        Some(sorted[n / 2])
    } else {
        Some((sorted[n / 2 - 1] + sorted[n / 2]) / 2.0)
    }
}

/// The most frequent value, or `None` when nothing repeats.
///
/// Ties go to whichever appeared first, which is what Excel's `MODE.SNGL` does —
/// the "SNGL" is the acknowledgement that there may be more than one answer.
fn mode(values: &[f64]) -> Option<f64> {
    let mut best: Option<(f64, usize)> = None;
    for (index, value) in values.iter().enumerate() {
        let count = values[index..].iter().filter(|v| *v == value).count()
            + values[..index].iter().filter(|v| *v == value).count();
        if count < 2 {
            continue;
        }
        match best {
            Some((_, best_count)) if best_count >= count => {}
            _ => best = Some((*value, count)),
        }
    }
    best.map(|(value, _)| value)
}

/// `PERCENTILE.INC` and `PERCENTILE.EXC`, which differ in where the ends are.
///
/// The inclusive form spreads the values across `0..=1`, so `p = 0` is the
/// minimum. The exclusive form places them at `i/(n+1)`, so neither end is
/// reachable and a `p` outside that band is an error rather than a clamp —
/// Excel is strict here and a clamp would answer a question nobody asked.
fn percentile(values: &[f64], p: f64, inclusive: bool) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let sorted = sorted_copy(values);
    let n = sorted.len();

    let rank = if inclusive {
        if !(0.0..=1.0).contains(&p) {
            return None;
        }
        p * (n as f64 - 1.0)
    } else {
        let lower = 1.0 / (n as f64 + 1.0);
        let upper = n as f64 / (n as f64 + 1.0);
        if p < lower || p > upper {
            return None;
        }
        p * (n as f64 + 1.0) - 1.0
    };

    let low = rank.floor() as usize;
    let high = (low + 1).min(n - 1);
    let fraction = rank - low as f64;
    Some(sorted[low] + fraction * (sorted[high] - sorted[low]))
}
