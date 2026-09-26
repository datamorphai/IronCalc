use ironcalc_base::expressions::types::Area;
use ironcalc_base::UserModel;
use std::time::Instant;
fn run(label: &str, step: &dyn Fn(&mut UserModel, i32)) {
    let mut m = UserModel::new_empty("wb", "en", "UTC", "en").unwrap();
    m.pause_evaluation();
    let t0 = Instant::now(); let mut last = Instant::now(); let mut marks = vec![];
    for r in 1..=300_000 { step(&mut m, r); if r % 50_000 == 0 { let now = Instant::now(); marks.push(format!("{}k:{:.1}µs", r / 1000, now.duration_since(last).as_secs_f64() * 1e6 / 50_000.0)); last = now; } }
    println!("{:<36} {} ms {}", label, t0.elapsed().as_millis(), marks.join(" "));
}
fn area(r: i32) -> Area { Area { sheet: 0, row: r, column: 1, width: 1, height: 1 } }
fn main() {
    run("A number + B formula (no format)", &|m, r| { m.set_user_input(0, r, 1, &r.to_string()).unwrap(); m.set_user_input(0, r, 2, &format!("=A{}*2", r)).unwrap(); });
    run("A number + format (no formula)", &|m, r| { m.set_user_input(0, r, 1, &r.to_string()).unwrap(); m.update_range_style(&area(r), "num_fmt", "$#,##0.00").unwrap(); });
    run("A number + format + B formula", &|m, r| { m.set_user_input(0, r, 1, &r.to_string()).unwrap(); m.update_range_style(&area(r), "num_fmt", "$#,##0.00").unwrap(); m.set_user_input(0, r, 2, &format!("=A{}*2", r)).unwrap(); });
}
