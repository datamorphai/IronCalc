use ironcalc_base::expressions::types::Area;
use ironcalc_base::{Model, UserModel};
use std::collections::HashMap;
use std::time::Instant;

#[derive(serde::Deserialize)]
struct Cell { input: String, #[serde(default)] format: Option<String> }
#[derive(serde::Deserialize)]
struct File { name: String, cells: HashMap<String, Cell> }

fn key(k: &str) -> (i32, i32) {
    let mut col = 0i32; let mut i = 0;
    for ch in k.chars() { if ch.is_ascii_alphabetic() { col = col * 26 + (ch as i32 - 64); i += 1; } else { break; } }
    (k[i..].parse().unwrap(), col)
}
fn rss_mb() -> u64 {
    let out = std::process::Command::new("ps").args(["-o", "rss=", "-p", &std::process::id().to_string()]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse::<u64>().unwrap_or(0) / 1024
}
fn main() {
    let dir = std::env::args().nth(1).unwrap();
    let mode = std::env::args().nth(2).unwrap_or("user".into());
    let mut names: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().map(|e| e == "json").unwrap_or(false)).collect();
    names.sort();
    let heads: Vec<String> = names.iter().map(|p| { let t = std::fs::read_to_string(p).unwrap(); let f: File = serde_json::from_str(&t).unwrap(); f.name }).collect();
    let t0 = Instant::now();
    let mut user = UserModel::new_empty("wb", "en", "UTC", "en").unwrap();
    let mut plain = Model::new_empty("wb", "en", "UTC", "en").unwrap();
    if mode != "plain" { user.pause_evaluation(); }
    if mode == "nohist" { user.pause_history(); }
    for (i, n) in heads.iter().enumerate() {
        if i == 0 { if mode != "plain" { user.rename_sheet(0, n).unwrap(); } else { plain.rename_sheet_by_index(0, n).unwrap(); } }
        else if mode != "plain" { user.new_sheet().unwrap(); user.rename_sheet(i as u32, n).unwrap(); } else { plain.new_sheet(); plain.rename_sheet_by_index(i as u32, n).unwrap(); }
    }
    let mut cells = 0u64;
    for (i, p) in names.iter().enumerate() {
        let t = std::fs::read_to_string(p).unwrap();
        let f: File = serde_json::from_str(&t).unwrap();
        drop(t);
        let ts = Instant::now();
        for (k, c) in &f.cells {
            if c.input.is_empty() { continue; }
            let (row, col) = key(k);
            if mode != "plain" { user.set_user_input(i as u32, row, col, &c.input).unwrap(); } else { plain.set_user_input(i as u32, row, col, c.input.clone()).unwrap(); }
            cells += 1;
        }
        for (k, c) in &f.cells {
            if let Some(fmt) = &c.format { if fmt == "general" { continue; } let (row, col) = key(k);
                if mode != "plain" { user.update_range_style(&Area { sheet: i as u32, row, column: col, width: 1, height: 1 }, "num_fmt", fmt).unwrap(); }
                else { let mut style = plain.get_style_for_cell(i as u32, row, col).unwrap(); style.num_fmt = fmt.clone(); plain.set_cell_style(i as u32, row, col, &style).unwrap(); } }
        }
        println!("{:<26} {:>9} cells so far  {:>6} ms  rss {} MB", f.name, cells, ts.elapsed().as_millis(), rss_mb());
    }
    println!("TOTAL {} cells in {} ms, rss {} MB ({})", cells, t0.elapsed().as_millis(), rss_mb(), mode);
}
