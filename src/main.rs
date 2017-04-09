#[macro_use] extern crate error_chain;
extern crate chrono;
extern crate git2;

mod errors {
    error_chain! { }
}

use chrono::Local;
use errors::*;
use git2::Repository;
use std::env::{args, current_dir, var};
use std::io::BufRead;
use std::process::Command;

const DATE_FORMAT: &'static str = "%d %b %H:%M:%S";

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
    let raw_path_str = path.to_str().unwrap_or("<error>");
    let home_str = var("HOME").chain_err(|| "failed to get HOME")?.to_owned();
    let path_str = if raw_path_str.starts_with(&home_str) {
        raw_path_str.replace(&home_str, "~")
    } else {
        raw_path_str.to_owned()
    };
    left_floats.push(" ".to_owned() + &path_str + " ");

    let current_time = Local::now();
    right_floats.push(" ".to_owned() + &current_time.format(DATE_FORMAT).to_string() + " ");

    let git_branch = find_git_branch();
    git_branch.map(|branch| left_floats.push(" @ git {".to_owned() + &branch + "} "));

    let prior_runtime_str = format_run_time(prior_runtime);

    //let layout = compute_layout(columns, &prior_runtime_str, &left_floats, &right_floats)
    //    .chain_err(|| "failed to layout")?;
    let layout = build_runs(columns, &prior_runtime_str, &left_floats, &right_floats);
    show_runs(&layout);

    /*
    for row in layout {
        for cell in row {
            put(&cell.content, cell.repcount);
        }
        put1("\n");
    }
    */

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

fn show_runs(layout: &Vec<String>) {
    for row in layout {
        println!("{}", &row);
    }
}

/*
Basic layout looks like:
┬───────────┬──────────┬───────────┬───────────────┬─────────────────────┐ TTT
├ PPPPPPPPP ┴ GGGGGGGG ┘           └ NNNNNNNN@HHHH └ YYYY-MM-DD HH:MM:SS ┴─────
└➤ ls foo/bar

If there are too many chars fo the line, wrap it favoring the left.
┬───────────────────────┬──────────┬───────────────┬─────────────────────┐ TTT
├ AAAAAAAAAAAAAAAAAAAAA ┤          └ NNNNNNNN@HHHH └ YYYY-MM-DD HH:MM:SS ┴─────
├ BBBBBBBBBBBBBB ───────┘
└➤ ls foo/bar

But don't be afraid to wrap the right if the left is too big.
┬────────────────────────────────────────────┬──────────┬────────────────┐ TTT
├ AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA ┤          ├ NNNNNNNN@HHHH  ┴────┬
├ BBBBBBBBBBBBBB ────────────────────────────┘          └ YYYY-MM-DD HH:MM:SS ┘
└➤ ls foo/bar

And of course we need to be able to handle inverted lengths:
┬────────────────────────────────────────────┬──────────┬────────────────┐ TTT
├ AAAAAAAAAAAAAA ────────────────────────────┤          ├ NNNNNNNN@HHHH  ┴────┬
├ BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB ┘          └ YYYY-MM-DD HH:MM:SS ┘
└➤ ls foo/bar

This gets complicated when there are more items to lay out:
┬───────────┬──────────┬──────────┬───────────┐ TTT
├ AAAAAAAAA ┴ BBBBBBBB ┤          └ DDDDDDDDD ┴─────
├ CCCCCCCCCCCCCC ──────┘
└➤ ls foo/bar

┬───────────┬──────────┬──────────┬───────────┐ TTT
├ AAAAAAAAA ┼ BBBBBBBB ┘          └ DDDDDDDDD ┴─────
├ CCCC ─────┘
└➤ ls foo/bar

┬───────────────────────┬────────┬───────────┐ TTT
├ AAAAAAAAAAAAAAAAAAAAA ┤        └ DDDDDDDDD ┴─────
├ BBBBBBBB ┴ CCCCCCCC ──┘
└➤ ls foo/bar
*/
fn build_runs(columns: usize, prior_dt: &str, left_floats: &Vec<String>, right_floats: &Vec<String>) -> Vec<String>
{
    // Attempt to compute a simple, single-line layout.
    let mut rh: usize = 1;
    let mut rw: usize = right_floats.iter().map(|s| s.chars().count()).sum::<usize>() +
                        2 * right_floats.len() +
                        3 + prior_dt.chars().count() + 1;
    let mut left_layout = layout_left(rw, left_floats);

    // If we fail to layout, we need to squeeze the right and re-try.
    if left_layout.is_none() {
        //right_layout = layout_right(right_floats);
        //if right_layout.is_none() {
        return vec!["➤ ".to_owned()];
        //}
        //let (rw, rh) = right_layout;
        //left_layout = layout_left(rw, left_floats);
    }

    // Figure out how much room we have for left floats.
    // ┬────────────────────────────────┬───────────┐ TTT
    let required_columns = 1 + 0000000 + 3 + right_floats[0].chars().count() + 3 + prior_dt.chars().count() + 1;
    if required_columns > columns {
        return vec!["➤ ".to_owned()];
    }
    let width = columns - required_columns;

    // Split up our left floats by row.
    let mut left_width = 0;
    let mut left_splits: Vec<Vec<String>> = vec![vec![]];
    let mut x = 0;
    let mut y = 0;
    for s in left_floats {
        let next = x + 1 + s.chars().count() + 2;
        if next > width {
            if left_splits[y].len() == 0 {
                return vec!["➤ ".to_owned()];
            }
            x = 0;
            y += 1;
            left_splits[y] = Vec::new();
        }
        x = next;
        if x > left_width {
            left_width = x;
        }
        left_splits[y].push(s.to_owned());
    }

    let mut runs: Vec<String> = Vec::new();

    // Build the top row.
    let mut row0 = "┬".to_owned() + &repeats('─', left_width) + "┬";
    runs.push(row0);
    /*
    let mut prior_run = "┬".to_owned() + &repeats('─', width) + "┬" + &repeats('─', right_floats[0].chars().count()) + "┐" + prior_dt + " ";
    let mut current_run = "├ ".to_owned();
    y = 1;

    for row in left_splits {
        for s in row {
        }

        if y == 1 {
        }
    }

    runs.push(prior_run);
    runs.push(current_run);
    */
    return runs;
}

fn layout_left(columns: usize, left_floats: &Vec<String>) -> Option<(usize, usize)>
{
    None
}

fn repeats(c: char, cnt: usize) -> String {
    std::iter::repeat(c.to_string()).take(cnt).collect::<String>()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line() {
        let left = vec!["AAAA".to_owned(), "BBBB".to_owned(), "CCCC".to_owned()];
        let right = vec!["DDDD".to_owned(), "EEEE".to_owned()];
        let dt = "TTT";
        super::show_runs(&super::build_runs(80, dt, &left, &right));
    }
}
