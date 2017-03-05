#[macro_use] extern crate error_chain;
extern crate chrono;
extern crate git2;

mod errors {
    error_chain! { }
}

use chrono::Local;
use errors::*;
use git2::Repository;
use std::env::{args, current_dir};
use std::io::BufRead;
use std::process::Command;

fn put1(s: &str) { put(s, 1); }
fn put(s: &str, cnt: usize) {
    for _ in 0..cnt {
        print!("{}", s);
    }
}

quick_main!(run);
fn run() -> Result<()> {
    // This is not meant to be human callable, so worry about arg parsing later.
    // The format is currently: <status> <columns> <runtime_seconds>
    let args = args().collect::<Vec<String>>();
    let status = "0" == &args[1];
    let columns = args[2].parse::<usize>().chain_err(|| "expected usize columns")?;
    let prior_runtime = args[3].parse::<i32>().chain_err(|| "expected i32 run time")?;

    let mut left_floats = Vec::<String>::new();
    let mut right_floats = Vec::<String>::new();

    let path = current_dir().chain_err(|| "failed to getcwd")?;
    let path_str = path.to_str().unwrap_or("<error>");
    left_floats.push(path_str.to_owned());

    let current_time = Local::now();
    right_floats.push(current_time.to_string());

    let git_branch = find_git_branch();
    git_branch.map(|branch| left_floats.push(branch));

    let prior_runtime_str = format_run_time(prior_runtime);

    let layout = compute_layout(columns, &prior_runtime_str, &left_floats, &right_floats)
        .chain_err(|| "failed to layout")?;

    for row in layout {
        for cell in row {
            let cnt = if cell.repeat { cell.len } else { 1 };
            put(&cell.content, cnt);
        }
        put1("\n");
    }

    /*
    put1("┬─");
    put("─", path_str.len());
    put1("─┬");
    put("─", columns - prior_runtime_str.len());
    put1(" ");
    put1(&prior_runtime_str);
    put1("\n");

    put1("├ ");
    put1(path_str);
    put1(" ╯");
    put1("\n");

    put1("╰> ");
    */
    return Ok(());
}

struct Cell {
    row: usize,
    col: usize, // 0 based
    len: usize,
    repeat: bool,
    content: String,
}

fn compute_layout(columns: usize, prior_runtime: &str, left_floats: &Vec<String>, right_floats: &Vec<String>)
    -> Result<Vec<Vec<Cell>>>
{
    // Start with the basic top structure.
    let mut row0 = vec![Cell{row:0, col:0, len:columns, repeat:true, content:"─".to_owned()}];
    let mut row1 = vec![Cell{row:0, col:0, len:columns, repeat:true, content:" ".to_owned()}];
    let mut row2 = vec![Cell{row:0, col:0, len:columns, repeat:true, content:" ".to_owned()}];

    // Insert prior runtime.

    return Ok(vec![row0, row1, row2]);
}

fn format_run_time(t: i32) -> String {
    let mut out = "".to_owned();
    if t == 0 {
        return "ε".to_owned();
    }

    let mut s = t;
    if s > 3600 {
        let h = s / 3600;
        s = s - 3600 * h;
        out += &format!("{}h", h);
    }
    if s > 60 {
        let m = s / 60;
        s = s - 60 * m;
        out += &format!("{}m", m);
    }
    if s > 0 {
        out += &format!("{}s", s);
    }
    return out;
}

fn find_git_branch() -> Option<String> {
    let repo = match Repository::open(".") {
        Ok(repo) => repo,
        Err(e) => return None
    };
    let head = match repo.head() {
        Ok(head) => head,
        Err(e) => return None
    };
    return Some(match head.shorthand() {
        Some(tgt) => tgt,
        None => "(detached)"
    }.to_owned());
}
