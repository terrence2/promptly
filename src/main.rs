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

    let mut left_floats = Vec::<Div>::new();
    let mut right_floats = Vec::<Div>::new();

    let path = current_dir().chain_err(|| "failed to getcwd")?;
    let raw_path_str = path.to_str().unwrap_or("<error>");
    let home_str = var("HOME").chain_err(|| "failed to get HOME")?.to_owned();
    let path_str = if raw_path_str.starts_with(&home_str) {
        raw_path_str.replace(&home_str, "~")
    } else {
        raw_path_str.to_owned()
    };
    left_floats.push(Div::new3(" ", &path_str, " "));

    let current_time = Local::now();
    right_floats.push(Div::new3(" ", &current_time.format(DATE_FORMAT).to_string(), " "));

    let git_branch = find_git_branch();
    git_branch.map(|branch| left_floats.push(Div::new3(" @ git {", &branch, "} ")));

    let prior_runtime_str = format_run_time(prior_runtime);

    let options = LayoutOptions::new();
    let layout = build_runs(columns, &prior_runtime_str, left_floats, right_floats, &options);
    show_runs(&layout);

    return Ok(());
}

trait Emittable
{
    fn emit(&self);
    fn width(&self) -> usize;
}

#[derive(Debug, PartialEq)]
struct Span
{
    color: &'static str,
    content: String
}

impl Span {
    fn new(content: &str) -> Self {
        Span { color: "", content: content.to_owned() }
    }
    fn repeat(c: char, cnt: usize) -> Self {
        Span::new(&std::iter::repeat(c.to_string()).take(cnt).collect::<String>())
    }
}

impl Emittable for Span {
    fn emit(&self) {
        print!("{}{}", self.color, self.content);
    }

    fn width(&self) -> usize {
        return self.content.chars().count();
    }
}

#[derive(Debug, PartialEq)]
struct Div
{
    children: Vec<Span>
}

impl Div {
    fn new(a: &str) -> Self {
        Div { children: vec![Span::new(a)] }
    }
    fn new2(a: &str, b: &str) -> Self {
        Div { children: vec![Span::new(a), Span::new(b)] }
    }
    fn new3(a: &str, b: &str, c: &str) -> Self {
        Div { children: vec![Span::new(a), Span::new(b), Span::new(c)] }
    }
}

impl Emittable for Div {
    fn emit(&self) {
        self.children.iter().map(|s| s.emit());
    }

    fn width(&self) -> usize {
        return self.children.iter().map(|s| s.width()).sum();
    }
}

fn show_runs(layout: &Vec<Vec<Span>>) {
    for row in layout {
        for span in row {
            span.emit();
        }
        println!();
    }
}

struct LayoutOptions {
    verbose: bool,
}

impl LayoutOptions {
    fn new() -> LayoutOptions {
        LayoutOptions { verbose: false }
    }

    fn verbose(mut self, value: bool) -> LayoutOptions {
        self.verbose = value;
        return self;
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
// We build the layout with the following box drawing characters. After drawing, if
// some other drawing characters are desired, a post-processing pass will mutate
// into that set in-place.
//
//     ─ │ ┼
//
//     ┌ └ ┐ ┘
//
//     ├ ┤ ┬ ┴
//
fn build_runs(columns: usize,
              prior_dt: &str,
              left_floats: Vec<Div>,
              right_floats: Vec<Div>,
              options: &LayoutOptions)
              -> Vec<Vec<Span>> {
    let fail = vec![vec![Span::new("➤ ")]];

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
    let inner_width = columns - (2 + prior_dt.chars().count() + 1);
    let outer_width = columns - 1;

    // Compute packing for RHS, given minimal LHS.
    //        v----row0----v
    //        v------row1+------v
    // ┬───┬──┬──────┬──────┐ TTT
    // ├ A ┘  └ EEEE ┴ FFFF ┴─────
    //
    //       v-----v
    //       v----------v
    // ┬───┬─┬──────┐ TTT
    // ├ A ┘ ├ FFFF ┼─────
    //       └ EE ──┘
    let (w_max_right, h_max_right) = match pack_into_width(inner_width - 5, outer_width - 5, &right_floats) {
        None => return fail,
        Some(p) => p,
    };

    // Try to pack the left into the maximized rhs.
    //  v----------row0----------v
    //  v----------row1+---------v
    // ┬───┬───────────────────────┬──────┬──────┐ TTT
    // ├ ? ┘                       └ EEEE ┴ FFFF ┴─────
    let minimal_left = inner_width - w_max_right - 1;
    match pack_into_width(minimal_left, minimal_left, &left_floats) {
        Some((w_min_left, h_min_left)) => {
            if h_max_right >= h_min_left {
                return do_layout(columns,
                                 w_min_left,
                                 w_max_right,
                                 cmp::max(h_min_left, h_max_right),
                                 prior_dt,
                                 left_floats,
                                 right_floats,
                                 options);
            }
        }
        None => {}
    };

    // If the maximal right did not allow the left side to fit well, re-try with a minimal right.
    let (w_min_right, h_min_right) = find_minimal_width(&right_floats,
                                                        2 + prior_dt.chars().count() + 1);

    // Try again to pack the left into the minimal rhs.
    //  v-------------row0--------------v
    //  v-------------row1+-------------v
    // ┬───┬──────────────────────────────┬─────────┐ TTT
    // ├ ? ┘                              ├ FFFF ───┴─────
    //                                    └ EEEEEEEEEEEEE
    let maximal_left = inner_width - w_min_right - 1;
    let (w_max_left, h_max_left) = match pack_into_width(maximal_left, maximal_left,
                                                         &left_floats) {
        None => return fail,
        Some(p) => p,
    };

    println!("h_max_left: {}; h_min_right: {}", h_max_left, h_min_right);
    return do_layout(columns,
                     w_max_left,
                     w_min_right,
                     cmp::max(h_max_left, h_min_right),
                     prior_dt,
                     left_floats,
                     right_floats,
                     options);

}

fn do_layout(columns: usize,
             left_extent: usize,
             right_extent: usize,
             height: usize,
             prior_dt: &str,
             left_floats: Vec<Div>,
             right_floats: Vec<Div>,
             options: &LayoutOptions)
             -> Vec<Vec<Span>> {
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
    let mut runs: Vec<Vec<Span>> = Vec::new();
    let left_start = 1;
    let left_end = left_start + left_extent;
    let right_start = columns - (2 + prior_dt.chars().count() + 1) - right_extent;
    let right_end = right_start + right_extent;
    let left_by_row = split_for_width(left_extent, left_floats);
    let right_by_row = split_for_width(right_extent, right_floats);

    if options.verbose {
        println!("metrics:");
        println!("  height:       {}", height);
        println!("  left_extent:  {}", left_extent);
        println!("  left_start:   {}", left_start);
        println!("  left_end:     {}", left_end);
        println!("  right_extent: {}", right_extent);
        println!("  right_start:  {}", right_start);
        println!("  right_end:    {}", right_end);
        println!("rows are:");
        println!("  left: {:?}", left_by_row);
        println!("  right: {:?}", right_by_row);
    }

    // row 0
    let mut row0 = vec![Span::new("┬")];
    let mut offset = left_start;
    for f in left_by_row[0].iter() {
        let w = f.width();
        row0.append(&mut vec![Span::new("─"), Span::repeat('─', w), Span::new("─┬")]);
        offset += w + 3;
        debug_assert!(offset <= left_end);
    }
    row0.push(Span::repeat('─', right_start - offset));
    offset = right_start;
    for f in right_by_row[0].iter() {
        let w = f.width();
        row0.append(&mut vec![Span::new("┬─"), Span::repeat('─', w), Span::new("─")]);
        offset += w + 3;
        debug_assert!(offset <= right_end);
    }
    row0.push(Span::repeat('─', right_end - offset));
    row0.append(&mut vec![Span::new("┐ "), Span::new(prior_dt), Span::new(" ")]);
    runs.push(row0);

    // rows n+
    for i in 0..height {
        let mut row = vec![];
        let mut offset = left_start;
        if left_by_row.len() > i {
            row.push(Span::new("├"));
            for f in left_by_row[i].iter() {
                let mut box_end = "┘";
                if f != left_by_row[i].last().unwrap() {
                    box_end = "┴";
                }
                let w = f.width();
                row.append(&mut vec![Span::new(" ")]);
                for c in f.children.iter() {
                    row.push(Span::new(&c.content));
                }
                row.append(&mut vec![Span::new(" "), Span::new(box_end)]);
                offset += w + 3;
                debug_assert!(offset <= left_end);
            }
        } else {
            row.push(Span::new("│"));
            row.push(Span::repeat(' ', right_start - offset));
            offset = right_start;
        }
        row.push(Span::repeat(' ', right_start - offset));
        offset = right_start;
        for f in right_by_row[i].iter() {
            let mut box_start = "└";
            if f != right_by_row[i].first().unwrap() {
                box_start = "┴";
            }
            if i < right_by_row.len() - 1 {
                box_start = "├";
            }
            let w = f.width();
            row.append(&mut vec![Span::new(box_start), Span::new(" ")]);
            for c in f.children.iter() {
                row.push(Span::new(&c.content));
            }
            row.append(&mut vec![Span::new(" ")]);
            offset += w + 3;
            debug_assert!(offset <= right_end, "foo");
        }
        row.push(Span::repeat('─', right_end - offset));
        if i == 0 {
            if height > 1 {
                row.append(&mut vec![Span::new("┼─"), Span::repeat('─', prior_dt.chars().count()), Span::new("─")]);
            } else {
                row.append(&mut vec![Span::new("┴─"), Span::repeat('─', prior_dt.chars().count()), Span::new("─")]);
            }
        } else {
            if offset <= right_end {
                row.push(Span::new("┘"));
            } else {
                row.push(Span::new(" "));
            }
            row.append(&mut vec![Span::repeat(' ', prior_dt.chars().count()), Span::new("  ")]);
        }
        runs.push(row);
    }

    runs.push(vec![Span::new("└➤ ")]);
    return runs;
}

fn pack_into_width(width_0: usize, width_n: usize, floats: &Vec<Div>) -> Option<(usize, usize)> {
    let mut pack_width = 0;
    let mut pack_height = 0;

    // The given width exludes any separator padding, so we can fill all
    // the way to the given width.
    //                                      / This space is not included.
    //    v-------------------------------vV
    // ...──────┬──────┬──────┬──────┬──────┬──────┬──────┐ TTT...
    // ... AAAA ┴ BBBB ┴ CCCC ┴ DDDD ┘      └ EEEE ┴ FFFF ┴─────...
    //
    // Each float extends from 1 to the left and 2 to the right for box drawing.
    //    v-----v
    // ...──────┬──────┬──────┬──────┬...
    // ... AAAA ┴ BBBB ┴ CCCC ┴ DDDD ┘...
    let mut width = width_0;
    let mut offset = 0;
    for f in floats {
        let fw = f.width();
        offset += 3 + fw;
        if offset > width {
            pack_height += 1;
            width = width_n;
            offset = 3 + fw;
            if offset > width {
                return None;
            }
        }
        if offset > pack_width {
            pack_width = offset;
        }
    }
    return Some((pack_width, pack_height + 1));
}

fn split_for_width(width: usize, mut floats: Vec<Div>) -> Vec<Vec<Div>> {
    if floats.len() == 0 {
        return vec![];
    }

    let mut out: Vec<Vec<Div>> = Vec::new();
    let mut row = 0;

    // Each float extends from 1 to the left and 2 to the right for box drawing.
    //    v-----v
    // ...──────┬──────┬──────┬──────┬...
    // ... AAAA ┴ BBBB ┴ CCCC ┴ DDDD ┘...
    let mut column = floats[0].width() + 3;

    out.push(vec![floats.remove(0)]);
    while floats.len() > 0 {
        let f = floats.remove(0);
        let fw = 3 + f.width();
        if column + fw > width {
            row += 1;
            column = 0;
            out.push(vec![]);
        }
        column += fw;
        out[row].push(f);
    }

    return out;
}

fn find_minimal_width(floats: &Vec<Div>, bump: usize) -> (usize, usize) {
    // Find the largest float. This is the minimum colums we can use.
    // Remember to increase the size of the first float for the bump.
    let mut min_columns = 0;
    for f in floats {
        let mut fw = 3 + f.width();
        if f == floats.first().unwrap() {
            fw += bump;
        }
        if fw > min_columns {
            min_columns = fw;
        }
    }

    // When we split_for_width, we will greedily pack multiple small floats
    // in the area taken by the largest, so figure out how many rows this will
    // result in.
    let mut row_count = 0;
    let mut offset = bump;
    for f in floats {
        let fw = 3 + f.width();
        if offset + fw > min_columns {
            row_count += 1;
            offset = 0;
        }
        offset += fw;
    }
    return (min_columns, row_count + 1);
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

    fn assertions(width: usize, height: usize, runs: &Vec<Vec<Span>>) {
        assert_eq!(height + 1, runs.len());
        for run in runs.iter() {
            if run == runs.last().unwrap() { break; }
            //assert_eq!(width, run.width());
        }

        /*
        let all_box_drawing = vec!['─', '┼', '│', '┌', '└', '┐', '┘', '┤','├', '┬', '┴'];
        let right_exits = vec!['─', '┼', '┌', '└', '├', '┬', '┴'];
        let left_exits = vec!['─', '┼', '┐', '┘', '┤', '┬', '┴'];
        for row in 0..height {
            let run = &runs[row];
            for col in 0..run.chars().count() {
                if col == width - 1 {
                    break;
                }
                let left = run.chars().nth(col).unwrap();
                let right = run.chars().nth(col + 1).unwrap();

                if all_box_drawing.contains(&left) && all_box_drawing.contains(&right) {
                    // -> connects to ->
                    if right_exits.contains(&left) && !left_exits.contains(&right) {
                        println!("Found invalid box sequence at row: {}, col: {}", row, col);
                        println!("  chars are: '{}' -> '{}'", left, right);
                        println!("Context:");
                        println!("  row: {:?}", run);
                        assert!(false);
                    }
                    if left_exits.contains(&right) && !right_exits.contains(&left) {
                        println!("Found invalid box sequence at row: {}, col: {}", row, col);
                        println!("  chars are: '{}' -> '{}'", left, right);
                        println!("Context:");
                        println!("  row: {:?}", run);
                        assert!(false);
                    }
                }
            }
        }
        */

        /*
        let bottom_exits = vec!['┼', '│', '┌', '┐', '┤','├', '┬'];
        let top_exits = vec!['┼', '│', '└', '┘', '┤','├', '┴'];
        for row in 0..height {
            let run0 = &runs[row];
            let run1 = &runs[row + 1];

            for col in 0..run0.chars().count() {
                if col >= run1.chars().count() {
                    break;
                }
                let above = run0.chars().nth(col).unwrap();
                let below = run1.chars().nth(col).unwrap();
                if all_box_drawing.contains(&above) && all_box_drawing.contains(&below) {
                    // v connects to v
                    if bottom_exits.contains(&above) && !top_exits.contains(&below) {
                        println!("Found invalid box sequence at row: {}, col: {}", row, col);
                        println!("  chars are: '{}' above '{}'", above, below);
                        println!("Context:");
                        println!("  row0: {:?}", run0);
                        println!("  row1: {:?}", run1);
                        assert!(false);
                    }
                    if top_exits.contains(&below) && !bottom_exits.contains(&above) {
                        println!("Found invalid box sequence at row: {}, col: {}", row, col);
                        println!("  chars are: '{}' above '{}'", above, below);
                        println!("Context:");
                        println!("  row0: {:?}", run0);
                        println!("  row1: {:?}", run1);
                        assert!(false);
                    }
                }
            }

            /*
            if row >= height - 1{
                break;
            }
            for col in 0..run.chars().count() {
                let c = run.chars().nth(col).unwrap();
                let below = runs[row + 1].chars().nth(col).unwrap();
                if all_box_drawing.contains(&c) && all_box_drawing.contains(&below) {
                    // v connects to v
                    if bottom_exits.contains(&c) && !top_exits.contains(&below) {
                        assert!(false);
                    }
                }
            }
            */
        }
        */
    }

    #[test]
    fn single_line() {
        let options = LayoutOptions::new();
        let left = vec![Div::new("AAAA"), Div::new("BBBB"), Div::new("CCCC")];
        let right = vec![Div::new("DDDD"), Div::new("EEEE")];
        let dt = "TTT";
        let width = 80;
        let runs = super::build_runs(width, dt, left, right, &options);
        assertions(width, 2, &runs);
        super::show_runs(&runs);
    }

    #[test]
    fn single_line_min() {
        let options = LayoutOptions::new();
        let left = vec![Div::new("AAAA"), Div::new("BBBB"), Div::new("CCCC")];
        let right = vec![Div::new("DDDD"), Div::new("EEEE")];
        let dt = "TTT";
        let runs = super::build_runs(43, dt, left, right, &options);
        super::show_runs(&runs);
        assertions(43, 2, &runs);
    }

    #[test]
    fn drop_right_long_short() {
        let options = LayoutOptions::new().verbose(true);
        let left = vec![Div::new("AAAA"), Div::new("BBBB"), Div::new("CCCC")];
        let right = vec![Div::new("DDDDDDDD"), Div::new("EEEE")];
        let dt = "TTT";
        let runs = super::build_runs(42, dt, left, right, &options);
        assertions(42, 3, &runs);
        super::show_runs(&runs);
    }

    #[test]
    fn drop_right_short_long() {
        let options = LayoutOptions::new();
        let left = vec![Div::new("AAAA"), Div::new("BBBB"), Div::new("CCCC")];
        let right = vec![Div::new("DDDD"), Div::new("EEEEEEEE")];
        let dt = "TTT";
        let runs = super::build_runs(42, dt, left, right, &options);
        assertions(42, 3, &runs);
        super::show_runs(&runs);
    }
}
