#[macro_use] extern crate error_chain;
extern crate nix;

mod errors {
    error_chain! { }
}

use errors::*;
use nix::unistd::fork;
use std::env::{args, current_dir};

fn put1(s: &str) { put(s, 1); }
fn put(s: &str, cnt: usize) {
    for _ in 0..cnt {
        print!("{}", s);
    }
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

quick_main!(run);
fn run() -> Result<()> {
    let args = args().collect::<Vec<String>>();
    let status = "0" == &args[1];
    let columns = args[2].parse::<usize>().chain_err(|| "expected usize columns")?;
    let run_time = args[3].parse::<i32>().chain_err(|| "expected i32 run time")?;

    let path = current_dir().chain_err(|| "failed to getcwd")?;
    let path_str = path.to_str().unwrap_or("<error>");

    let time_str = format_run_time(run_time);
    put1("┬─");
    put("─", path_str.len());
    put1("─┬");
    put("─", columns - time_str.len());
    put1(" ");
    put1(&time_str);
    put1("\n");

    put1("├ ");
    put1(path_str);
    put1(" ╯");
    put1("\n");


    put1("╰> ");
    return Ok(());
}
