#[macro_use]
extern crate error_chain;
extern crate chrono;
extern crate git2;

mod errors {
    error_chain!{}
}

use chrono::Local;
use errors::*;
use git2::Repository;
use std::cmp;
use std::env::{args, current_dir, var};

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

    // let layout = compute_layout(columns, &prior_runtime_str, &left_floats, &right_floats)
    //    .chain_err(|| "failed to layout")?;
    let layout = build_runs(columns, &prior_runtime_str, left_floats, right_floats);
    show_runs(&layout);

    // for row in layout {
    // for cell in row {
    // put(&cell.content, cell.repcount);
    // }
    // put1("\n");
    // }
    //

    // put1("┬─");
    // put("─", path_str.len());
    // put1("─┬");
    // put("─", columns - prior_runtime_str.len());
    // put1(" ");
    // put1(&prior_runtime_str);
    // put1("\n");
    //
    // put1("├ ");
    // put1(path_str);
    // put1(" ╯");
    // put1("\n");
    //
    // put1("╰> ");
    //
    return Ok(());
}

fn show_runs(layout: &Vec<String>) {
    for row in layout {
        println!("{}", &row);
    }
}

struct LayoutOptions {
    verbose: bool,
}

impl LayoutOptions {
    fn new() {
        LayoutOptions { verbose: false }
    }

    fn verbose(&mut self) {
        self.verbose = true;
    }
    fn quiet(&mut self) {
        self.verbose = false;
    }
}

// Basic layout looks like:
// ┬───────────┬──────────┬───────────┬───────────────┬─────────────────────┐ TTT
// ├ PPPPPPPPP ┴ GGGGGGGG ┘           └ NNNNNNNN@HHHH └ YYYY-MM-DD HH:MM:SS ┴─────
// └➤ ls foo/bar
//
// If there are too many chars fo the line, wrap it favoring the left.
// ┬───────────────────────┬──────────┬───────────────┬─────────────────────┐ TTT
// ├ AAAAAAAAAAAAAAAAAAAAA ┤          └ NNNNNNNN@HHHH └ YYYY-MM-DD HH:MM:SS ┴─────
// ├ BBBBBBBBBBBBBB ───────┘
// └➤ ls foo/bar
//
// But don't be afraid to wrap the right if the left is too big.
// ┬────────────────────────────────────────────┬──────────┬────────────────┐ TTT
// ├ AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA ┤          ├ NNNNNNNN@HHHH  ┴────┬
// ├ BBBBBBBBBBBBBB ────────────────────────────┘          └ YYYY-MM-DD HH:MM:SS ┘
// └➤ ls foo/bar
//
// And of course we need to be able to handle inverted lengths:
// ┬────────────────────────────────────────────┬──────────┬────────────────┐ TTT
// ├ AAAAAAAAAAAAAA ────────────────────────────┤          ├ NNNNNNNN@HHHH  ┴────┬
// ├ BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB ┘          └ YYYY-MM-DD HH:MM:SS ┘
// └➤ ls foo/bar
//
// This gets complicated when there are more items to lay out:
// ┬───────────┬──────────┬──────────┬───────────┐ TTT
// ├ AAAAAAAAA ┴ BBBBBBBB ┤          └ DDDDDDDDD ┴─────
// ├ CCCCCCCCCCCCCC ──────┘
// └➤ ls foo/bar
//
// ┬───────────┬──────────┬──────────┬───────────┐ TTT
// ├ AAAAAAAAA ┼ BBBBBBBB ┘          └ DDDDDDDDD ┴─────
// ├ CCCC ─────┘
// └➤ ls foo/bar
//
// ┬───────────────────────┬────────┬───────────┐ TTT
// ├ AAAAAAAAAAAAAAAAAAAAA ┤        └ DDDDDDDDD ┴─────
// ├ BBBBBBBB ┴ CCCCCCCC ──┘
// └➤ ls foo/bar
//
fn build_runs(columns: usize,
              prior_dt: &str,
              left_floats: Vec<String>,
              right_floats: Vec<String>)
              -> Vec<String> {
    let fail = vec!["➤ ".to_owned()];

    // MEASUREMENTS:
    //
    //  v------------------- columns ---------------------v
    //  ┬───────────────────────┬────────┬───────────┐ TTT
    //  ├ AAAAAAAAAAAAAAAAAAAAA ┘        └ DDDDDDDDD ┴─────
    //  └➤ ls foo/bar
    //
    //   v--------------- inner_width --------------v
    //  ┬───────────────────────┬────────┬───────────┐ TTT
    //  ├ AAAAAAAAAAAAAAAAAAAAA ┘        └ DDDDDDDDD ┴─────
    //  └➤ ls foo/bar
    //
    //   v----- left_extent ----v        v--- right_extent ---v
    //  ┬───────────────────────┬────────┬─────────────────────┐ TTT
    //  ├ AAAAAAAAAAAAAAAAAAAAA ┘        └ DDDDDDDDDDDDDDDDDDD ┴─────
    //  └➤ ls foo/bar
    //
    //        ┬───────────┬─────────────────────┬───────────┐ TTT
    //      > ├ AAAAAAAAA ┤                     ├ DDDDDDDDD ┴─────
    // height ├ BBBBBBBBB ┤                     └ DDDDDDDDD ┘
    //      > ├ CCCCCCCCC ┘
    //        └➤ ls foo/bar
    //
    let inner_width = columns - (1 + 2 + prior_dt.chars().count() + 1);

    // Compute packing for RHS, given minimal LHS.
    //       v-------------v
    // ┬───┬──┬──────┬──────┐ TTT
    // ├ A ┘  └ EEEE ┴ FFFF ┴─────
    //
    //       v-----v
    // ┬───┬─┬──────┐ TTT
    // ├ A ┘ ├ FFFF ┴─────
    //       └── EE ┘
    let (w_max_right, h_max_right) = match pack_into_width(inner_width - 5, &right_floats) {
        None => return fail,
        Some(p) => p,
    };

    // Try to pack the left into the maximized rhs.
    //  v------------------------v
    // ┬───┬───────────────────────┬──────┬──────┐ TTT
    // ├ ? ┘                       └ EEEE ┴ FFFF ┴─────
    match pack_into_width(inner_width - w_max_right - 1, &left_floats) {
        Some((w_min_left, h_min_left)) => {
            if h_max_right >= h_min_left {
                return do_layout(columns,
                                 w_min_left,
                                 w_max_right,
                                 cmp::max(h_min_left, h_max_right),
                                 prior_dt,
                                 left_floats,
                                 right_floats);
            }
        }
        None => {}
    };

    // If the maximal right did not allow the left side to fit well, re-try with a minimal right.
    let (w_min_right, h_min_right) = find_minimal_width(&right_floats,
                                                        2 + prior_dt.chars().count() + 1);

    // Try again to pack the left into the minimal rhs.
    //  v------------------------v
    // ┬───┬──────────────────────────────┬─────────┐ TTT
    // ├ ? ┘                              ├ FFFF ───┴─────
    //                                    └ EEEEEEEEEEEEEE
    let (w_max_left, h_max_left) = match pack_into_width(inner_width - w_min_right - 1,
                                                         &left_floats) {
        None => return fail,
        Some(p) => p,
    };

    return do_layout(columns,
                     w_max_left,
                     w_min_right,
                     cmp::max(h_max_left, h_min_right),
                     prior_dt,
                     left_floats,
                     right_floats);

}

fn do_layout(columns: usize,
             left_extent: usize,
             right_extent: usize,
             height: usize,
             prior_dt: &str,
             left_floats: Vec<String>,
             right_floats: Vec<String>)
             -> Vec<String> {
    // MEASUREMENTS:
    //
    //  v------------------- columns ---------------------v
    //  ┬───────────────────────┬────────┬───────────┐ TTT
    //  ├ AAAAAAAAAAAAAAAAAAAAA ┘        └ DDDDDDDDD ┴─────
    //  └➤ ls foo/bar
    //
    //   v----- left_extent ----v        v--- right_extent ---v
    //  ┬───────────────────────┬────────┬─────────────────────┐ TTT
    //  ├ AAAAAAAAAAAAAAAAAAAAA ┘        └ DDDDDDDDDDDDDDDDDDD ┴─────
    //  └➤ ls foo/bar
    //
    //        ┬───────────┬─────────────────────┬───────────┐ TTT
    //      > ├ AAAAAAAAA ┤                     ├ DDDDDDDDD ┴─────
    // height ├ BBBBBBBBB ┤                     └ DDDDDDDDD ┘
    //      > ├ CCCCCCCCC ┘
    //        └➤ ls foo/bar
    //
    //  vv <-left_start
    //  v---- left_end ---------v
    //  v--------- right_start ---------v
    //  v---------------------- right_end --------------------v
    //  ┬───────────────────────┬────────┬─────────────────────┐ TTT
    //  ├ AAAAAAAAAAAAAAAAAAAAA ┘        └ DDDDDDDDDDDDDDDDDDD ┴─────
    //  └➤ ls foo/bar
    //
    let mut runs: Vec<String> = Vec::new();
    let left_start = 1;
    let left_end = left_start + left_extent;
    let right_start = columns - (2 + prior_dt.chars().count() + 1) - right_extent;
    let right_end = right_start + right_extent;
    println!("metrics:");
    println!("  left_extent:  {}", left_extent);
    println!("  left_start:   {}", left_start);
    println!("  left_end:     {}", left_end);
    println!("  right_extent: {}", right_extent);
    println!("  right_start:  {}", right_start);
    println!("  right_end:    {}", right_end);

    let left_by_row = split_for_width(left_extent, left_floats);
    let right_by_row = split_for_width(right_extent, right_floats);
    println!("right_by_row: {:?}", right_by_row);

    // row 0
    let mut row0 = "┬".to_owned();
    let mut offset = left_start;
    for f in left_by_row[0].iter() {
        let w = f.chars().count();
        let t = &("─".to_owned() + &repeats('─', w) + "─┬");
        row0 += t;
        println!("Adding \"{}\" from {} to {}", t, offset, offset + w + 3);
        offset += w + 3;
        debug_assert!(offset <= left_end);
    }
    println!("Adding {} blank spaces", right_start - offset);
    row0 += &repeats('─', right_start - offset);
    offset = right_start;
    println!("starting right prints at: {}", offset);
    for f in right_by_row[0].iter() {
        let w = f.chars().count();
        let t = &("┬─".to_owned() + &repeats('─', w) + "─");
        row0 += t;
        println!("Adding \"{}\" from {} to {}", t, offset, offset + w + 3);
        offset += w + 3;
        debug_assert!(offset <= right_end);
    }
    row0 += &("┐ ".to_owned() + prior_dt + " ");
    runs.push(row0);

    // while left_floats.len() > 0 || right_floats.len() > 0 {
    let mut row = "├".to_owned();
    let mut offset = left_start;
    for f in left_by_row[0].iter() {
        let w = f.chars().count();
        row += &(" ".to_owned() + &f + " ┴");
        offset += w + 3;
        debug_assert!(offset <= left_end);
    }
    row += &repeats(' ', right_start - offset);
    offset = right_start;
    for f in right_by_row[0].iter() {
        let w = f.chars().count();
        row += &("┴ ".to_owned() + f + " ");
        offset += w + 3;
        debug_assert!(offset <= right_end, "foo");
    }
    row += &("┴─".to_owned() + &repeats('─', prior_dt.chars().count()) + "─");
    runs.push(row);
    // }

    return runs;
}

fn pack_into_width(width: usize, floats: &Vec<String>) -> Option<(usize, usize)> {
    let mut pack_width = 0;
    let mut pack_height = 0;

    // The given width exludes any separator padding, so we can fill all
    // the way to the given width.
    //                                     /This space is not included.
    //    v-------------------------------vV
    // ...──────┬──────┬──────┬──────┬──────┬──────┬──────┐ TTT...
    // ... AAAA ┴ BBBB ┴ CCCC ┴ DDDD ┘      └ EEEE ┴ FFFF ┴─────...
    //
    // Each float extends from 1 to the left and 2 to the right for box drawing.
    //    v-----v
    // ...──────┬──────┬──────┬──────┬...
    // ... AAAA ┴ BBBB ┴ CCCC ┴ DDDD ┘...
    let mut offset = 0;
    for f in floats {
        let fw = f.chars().count();
        offset += 3 + fw;
        if offset > width {
            pack_height += 1;
            offset = 3 + fw;
            if offset > width {
                return None;
            }
        }
        if offset > pack_width {
            pack_width = offset;
        }
    }
    return Some((pack_width, pack_height));
}

fn split_for_width(width: usize, mut floats: Vec<String>) -> Vec<Vec<String>> {
    if floats.len() == 0 {
        return vec![];
    }

    let mut out: Vec<Vec<String>> = Vec::new();
    let mut row = 0;

    // Each float extends from 1 to the left and 2 to the right for box drawing.
    //    v-----v
    // ...──────┬──────┬──────┬──────┬...
    // ... AAAA ┴ BBBB ┴ CCCC ┴ DDDD ┘...
    let mut column = floats[0].chars().count() + 3;

    out.push(vec![floats.remove(0)]);
    while floats.len() > 0 {
        let f = floats.remove(0);
        let fw = 3 + f.chars().count();
        if column + fw > width {
            row += 1;
            out.push(vec![]);
        }
        out[row].push(f);
    }

    return out;
}

fn find_minimal_width(floats: &Vec<String>, bump: usize) -> (usize, usize) {
    // Find the largest float. This is the minimum colums we can use.
    let mut min_columns = 0;
    for f in floats {
        let mut fw = 3 + f.chars().count();
        if fw > min_columns {
            min_columns = fw;
        }
    }

    // When we split_for_width, we will greedily pack multiple small floats
    // in the area taken by the largest, so figure out how many rows this will
    // result in.
    let mut row_count = 0;
    let mut offset = 0;
    for f in floats {
        let fw = 3 + f.chars().count();
        if offset + fw > min_columns {
            row_count += 1;
            offset = 0;
        }
        offset += fw;
    }
    return (min_columns, row_count);
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
        Err(_) => return None,
    };
    let head = match repo.head() {
        Ok(head) => head,
        Err(_) => return None,
    };
    return Some(match head.shorthand() {
            Some(tgt) => tgt,
            None => "(detached)",
        }
        .to_owned());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line() {
        let left = vec!["AAAA".to_owned(), "BBBB".to_owned(), "CCCC".to_owned()];
        let right = vec!["DDDD".to_owned(), "EEEE".to_owned()];
        let dt = "TTT";
        let runs = super::build_runs(80, dt, left, right);
        assert_eq!(80, runs[0].chars().count());
        assert_eq!(80, runs[1].chars().count());
        super::show_runs(&runs);
    }

    #[test]
    fn single_line_min() {
        let left = vec!["AAAA".to_owned(), "BBBB".to_owned(), "CCCC".to_owned()];
        let right = vec!["DDDD".to_owned(), "EEEE".to_owned()];
        let dt = "TTT";
        let runs = super::build_runs(43, dt, left, right);
        super::show_runs(&runs);
    }

    #[test]
    fn drop_right_long_short() {
        let left = vec!["AAAA".to_owned(), "BBBB".to_owned(), "CCCC".to_owned()];
        let right = vec!["DDDDDDDD".to_owned(), "EEEE".to_owned()];
        let dt = "TTT";
        let runs = super::build_runs(42, dt, left, right);
        super::show_runs(&runs);
    }

    #[test]
    fn drop_right_short_long() {
        let left = vec!["AAAA".to_owned(), "BBBB".to_owned(), "CCCC".to_owned()];
        let right = vec!["DDDD".to_owned(), "EEEEEEEE".to_owned()];
        let dt = "TTT";
        let runs = super::build_runs(42, dt, left, right);
        super::show_runs(&runs);
    }
}
