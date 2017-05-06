/*
 * Promptly: show a prompt, sooner.
 * Copyright (C) 2017  Terrence Cole
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 */
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
    let columns = args[2]
        .parse::<usize>()
        .chain_err(|| "expected usize columns")?;
    let prior_runtime = args[3]
        .parse::<i32>()
        .chain_err(|| "expected i32 run time")?;

    let mut left_floats = Vec::<Div>::new();
    let mut right_floats = Vec::<Div>::new();

    let path = current_dir().chain_err(|| "failed to getcwd")?;
    let raw_path_str = path.to_str().unwrap_or("<error>");
    let home_str = var("HOME")
        .chain_err(|| "failed to get HOME")?
        .to_owned();
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
    let runs = match build_layout(columns,
                                  &prior_runtime_str,
                                  left_floats,
                                  right_floats,
                                  &options) {
        None => {
            let mut fail_run = Run::new(2);
            fail_run.add("➤", "prompt");
            fail_run.add(" ", "clear");
            vec![fail_run]
        }
        Some(layout) => render_with_layout(columns, &layout, &prior_runtime_str, &options),
    };
    show_runs(&runs);

    return Ok(());
}

#[derive(Debug, PartialEq)]
struct Span {
    color: &'static str,
    content: String,
}

impl Span {
    fn new(content: &str) -> Self {
        Span {
            color: "",
            content: content.to_owned(),
        }
    }

    fn width(&self) -> usize {
        return self.content.chars().count();
    }
}

#[derive(Debug, PartialEq)]
struct Div {
    children: Vec<Span>,
}

impl Div {
    fn new(a: &str) -> Self {
        Div { children: vec![Span::new(a)] }
    }

    fn new3(a: &str, b: &str, c: &str) -> Self {
        Div { children: vec![Span::new(a), Span::new(b), Span::new(c)] }
    }

    fn width(&self) -> usize {
        return self.children.iter().map(|s| s.width()).sum();
    }
}

struct Layout {
    left_extent: usize,
    right_extent: usize,
    height: usize,
    left_by_row: Vec<Vec<Div>>,
    right_by_row: Vec<Vec<Div>>,
}

impl Layout {
    fn new(left_extent: usize,
           right_extent: usize,
           height: usize,
           left_floats: Vec<Div>,
           right_floats: Vec<Div>)
           -> Self {
        Layout {
            left_extent: left_extent,
            right_extent: right_extent,
            height: height,
            left_by_row: split_for_width(left_extent, left_floats),
            right_by_row: split_for_width(right_extent, right_floats),
        }
    }
}

#[derive(Debug)]
struct Run {
    width: usize,
    cells: Vec<char>,
    formats: Vec<Option<&'static str>>,
    offset: usize,
    last_format: &'static str,
}

impl Run {
    fn new(width: usize) -> Self {
        Run {
            width: width,
            cells: std::iter::repeat(' ').take(width).collect::<Vec<char>>(),
            formats: std::iter::repeat(None)
                .take(width)
                .collect::<Vec<Option<&'static str>>>(),
            offset: 0,
            last_format: "",
        }
    }

    fn add(&mut self, s: &str, fmt: &'static str) {
        if fmt != self.last_format {
            self.last_format = fmt;
            self.formats[self.offset] = Some(fmt);
        }
        for c in s.chars() {
            self.cells[self.offset] = c;
            self.offset += 1;
        }
    }

    fn repeat(&mut self, c: char, cnt: usize, fmt: &'static str) {
        self.add(&std::iter::repeat(c.to_string())
                      .take(cnt)
                      .collect::<String>(),
                 fmt);
    }

    fn add_span(&mut self, span: &Span) {
        self.add(&span.content, span.color);
    }

    fn add_div(&mut self, div: &Div) {
        for span in div.children.iter() {
            self.add_span(span);
        }
    }

    fn is_border_at(&self, offset: usize) -> bool {
        return match self.cells[offset] {
                   '─' => true,
                   '│' => true,
                   '┼' => true,
                   '┌' => true,
                   '└' => true,
                   '┐' => true,
                   '┘' => true,
                   '├' => true,
                   '┤' => true,
                   '┬' => true,
                   '┴' => true,
                   _ => false,
               };
    }

    fn find_time_corner_border(&self, start: usize) -> Option<usize> {
        let mut offset = start;
        while offset < self.width {
            match self.cells[offset] {
                '┐' => return Some(offset),
                _ => {}
            }
            offset += 1;
        }
        return None;
    }

    fn find_next_border(&self, start: usize) -> Option<usize> {
        let mut offset = start;
        while offset < self.width {
            match self.cells[offset] {
                '─' => return Some(offset),
                '│' => return Some(offset),
                '┼' => return Some(offset),
                '┌' => return Some(offset),
                '└' => return Some(offset),
                '┐' => return Some(offset),
                '┘' => return Some(offset),
                '├' => return Some(offset),
                '┤' => return Some(offset),
                '┬' => return Some(offset),
                '┴' => return Some(offset),
                _ => {}
            }
            offset += 1;
        }
        return None;
    }

    fn add_south_border(&mut self, offset: usize) {
        let next = match self.cells[offset] {
            '─' => '┬',
            '│' => '│',
            '┼' => '┼',
            '┌' => '┌',
            '└' => '├',
            '┐' => '┐',
            '┘' => '┤',
            '├' => '├',
            '┤' => '┤',
            '┬' => '┬',
            '┴' => '┼',
            _ => '_',// panic!("do not know how to add south border to: {}", self.cells[offset])
        };
        self.cells[offset] = next;
    }

    fn add_east_border(&mut self) {
        let next = match self.cells[self.offset - 1] {
            '─' => '─',
            '│' => '├',
            '┼' => '┼',
            '┌' => '┌',
            '└' => '└',
            '┐' => '┬',
            '┘' => '┴',
            '├' => '├',
            '┤' => '┼',
            '┬' => '┬',
            '┴' => '┴',
            _ => '_',//panic!("do not know how to add south border to: {}", self.cells[self.offset - 1])
        };
        self.cells[self.offset - 1] = next;
    }

    fn format(&self) -> String {
        let mut out = "".to_owned();
        for c in self.cells.iter() {
            out.push(*c);
        }
        return out;
    }
}

fn format_runs(layout: &Vec<Run>) -> Vec<String> {
    layout
        .iter()
        .map(|r| r.format())
        .collect::<Vec<String>>()
}

fn show_runs(layout: &Vec<Run>) {
    for run in layout {
        println!("{}", run.format());
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
fn build_layout(columns: usize,
                prior_dt: &str,
                left_floats: Vec<Div>,
                right_floats: Vec<Div>,
                options: &LayoutOptions)
                -> Option<Layout> {

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
    if options.verbose {
        println!("columns:     {}", columns);
        println!("outer_width: {}", outer_width);
        println!("inner_width: {}", inner_width);
    }

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
    let (w_max_right, h_max_right) =
        match pack_into_width(inner_width - 5, outer_width - 5, &right_floats) {
            None => return None,
            Some(p) => p,
        };
    if options.verbose {
        println!("Pass1:");
        println!("    target0: {}", inner_width - 5);
        println!("    target1: {}", inner_width - 5);
        println!("    w_max_r: {}", w_max_right);
        println!("    h_max_r: {}", h_max_right);
    }

    // Try to pack the left into the maximized rhs.
    //  v----------row0----------v
    //  v----------row1+---------v
    // ┬───┬───────────────────────┬──────┬──────┐ TTT
    // ├ ? ┘                       └ EEEE ┴ FFFF ┴─────
    let minimal_left = inner_width - w_max_right - 1;
    match pack_into_width(minimal_left, minimal_left, &left_floats) {
        Some((w_min_left, h_min_left)) => {
            if options.verbose {
                println!("Pass2:");
                println!("    target0: {}", minimal_left);
                println!("    target1: {}", minimal_left);
                println!("    w_min_l: {}", w_min_left);
                println!("    h_min_l: {}", h_min_left);
            }
            if h_max_right >= h_min_left {
                return Some(Layout::new(w_min_left,
                                        w_max_right,
                                        cmp::max(h_min_left, h_max_right),
                                        left_floats,
                                        right_floats));
            }
        }
        None => {
            if options.verbose {
                println!("Pass2:");
                println!("    left does not fit into: {}", minimal_left);
            }
        }
    };

    // If the maximal right did not allow the left side to fit well, re-try with a minimal right.
    let (w_min_right, h_min_right) = find_minimal_width(&right_floats,
                                                        2 + prior_dt.chars().count());
    if options.verbose {
        println!("Pass3:");
        println!("    bump:    {}", 2 + prior_dt.chars().count());
        println!("    w_min_r: {}", w_min_right);
        println!("    h_min_r: {}", h_min_right);
    }

    // Try again to pack the left into the minimal rhs.
    //  v-------------row0--------------v
    //  v-------------row1+-------------v
    // ┬───┬──────────────────────────────┬─────────┐ TTT
    // ├ ? ┘                              ├ FFFF ───┴─────
    //                                    └ EEEEEEEEEEEEE
    let maximal_left = inner_width - w_min_right - 1;
    let (w_max_left, h_max_left) =
        match pack_into_width(maximal_left, maximal_left, &left_floats) {
            None => return None,
            Some(p) => p,
        };
    if options.verbose {
        println!("Pass4:");
        println!("    maximal_left: {}", maximal_left);
        println!("    w_max_l: {}", w_max_left);
        println!("    h_max_l: {}", h_max_left);
    }

    return Some(Layout::new(w_max_left,
                            w_min_right,
                            cmp::max(h_max_left, h_min_right),
                            left_floats,
                            right_floats));
}

fn render_with_layout(columns: usize,
                      layout: &Layout,
                      prior_dt: &str,
                      options: &LayoutOptions)
                      -> Vec<Run> {
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
    let mut runs: Vec<Run> = Vec::new();
    let left_start = 1;
    let left_end = left_start + layout.left_extent;
    let right_start = columns - (2 + prior_dt.chars().count() + 1) - layout.right_extent;
    let right_end = right_start + layout.right_extent;

    if options.verbose {
        println!("metrics:");
        println!("  height:       {}", layout.height);
        println!("  left_extent:  {}", layout.left_extent);
        println!("  left_start:   {}", left_start);
        println!("  left_end:     {}", left_end);
        println!("  right_extent: {}", layout.right_extent);
        println!("  right_start:  {}", right_start);
        println!("  right_end:    {}", right_end);
        println!("rows are:");
        println!("  left: {:?}", layout.left_by_row);
        println!("  right: {:?}", layout.right_by_row);
    }

    // row 0
    let mut row0 = Run::new(columns);
    row0.repeat('─', right_end, "border");
    row0.add("┐", "border");
    row0.add(" ", "clear");
    row0.add_span(&Span::new(prior_dt));
    row0.add(" ", "clear");
    runs.push(row0);

    // rows n+
    for i in 0..layout.height {
        let mut row = Run::new(columns);
        runs[i].add_south_border(row.offset);
        row.add("│", "border");

        // Emit LEFT
        if layout.left_by_row.len() > i {
            for f in layout.left_by_row[i].iter() {
                row.add_east_border();
                row.add(" ", "clear");
                row.add_div(f);
                row.add(" ", "clear");

                if f == layout.left_by_row[i].last().unwrap() {
                    let to_right = layout.left_extent - row.offset;
                    row.repeat('─', to_right, "border");
                }
                if runs[i].is_border_at(row.offset) {
                    runs[i].add_south_border(row.offset);
                    row.add("┘", "border");
                } else {
                    row.add("─", "border");
                }
            }
        }

        // Emit CENTER
        let to_right = right_start - row.offset;
        row.repeat(' ', to_right, "clear");

        // Emit RIGHT
        if layout.right_by_row.len() > i {
            runs[i].add_south_border(row.offset);
            row.add("└", "border");
            for f in layout.right_by_row[i].iter() {
                row.add(" ", "clear");
                row.add_div(f);
                row.add(" ", "clear");

                if i == 0 && f == layout.right_by_row[i].last().unwrap() {
                    match runs[i].find_time_corner_border(row.offset) {
                        Some(next_border) => {
                            let offset = next_border - row.offset;
                            if offset > 0 {
                                row.repeat('─', offset, "border")
                            }
                        }
                        None => {}
                    }
                } else {
                    match runs[i].find_next_border(row.offset) {
                        Some(next_border) => {
                            let offset = next_border - row.offset;
                            if offset > 0 {
                                row.repeat('─', offset, "border")
                            }
                        }
                        None => {}
                    }
                }
                runs[i].add_south_border(row.offset);
                row.add("┘", "border");
            }
            if i == 0 {
                let to_end = columns - row.offset;
                row.add_east_border();
                row.repeat('─', to_end, "border");
            }
        }
        runs.push(row);
    }

    let mut run_last = Run::new(3);
    run_last.add("└", "border");
    run_last.add("➤", "prompt");
    run_last.add(" ", "clear");

    runs.push(run_last);
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
    // Find the largest float. This is the minimum columns we can use.
    // Remember to increase the size of the first float for the bump.
    let mut min_columns = 0;
    for f in floats {
        let mut fw = 3 + f.width();
        if f != floats.first().unwrap() {
            fw -= bump;
        }
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
        let fw = 3 + f.width();
        if offset + fw > min_columns {
            row_count += 1;
            offset = 0;
        }
        offset += fw;
    }
    return (min_columns, row_count + 1);
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
        let options = LayoutOptions::new();
        let left = vec![Div::new("AAAA"), Div::new("BBBB"), Div::new("CCCC")];
        let right = vec![Div::new("DDDD"), Div::new("EEEE")];
        let dt = "TTT";
        let result = vec!["┬──────┬──────┬──────┬──────────────────────────────────────┬──────┬──────┐ TTT ",
                          "├ AAAA ┴ BBBB ┴ CCCC ┘                                      └ DDDD ┘ EEEE ┴─────",
                          "└➤ "];
        let layout = super::build_layout(80, dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(80, &layout, dt, &options);
        assert_eq!(super::format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn single_line_min() {
        let options = LayoutOptions::new();
        let left = vec![Div::new("AAAA"), Div::new("BBBB"), Div::new("CCCC")];
        let right = vec![Div::new("DDDD"), Div::new("EEEE")];
        let dt = "TTT";
        let result = vec!["┬──────┬──────┬──────┬─┬──────┬──────┐ TTT ",
                          "├ AAAA ┴ BBBB ┴ CCCC ┘ └ DDDD ┘ EEEE ┴─────",
                          "└➤ "];
        let layout = super::build_layout(43, dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(43, &layout, dt, &options);
        assert_eq!(super::format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn drop_left_2_1() {
        let options = LayoutOptions::new();
        let left = vec![Div::new("AAAA"), Div::new("BBBB"), Div::new("CCCC")];
        let right = vec![Div::new("DDDD"), Div::new("EEEEEEEE")];
        let dt = "TTT";
        let result = vec!["┬──────┬──────┬──┬──────┐ TTT ",
                          "├ AAAA ┴ BBBB ┤  ├ DDDD ┴───┬─",
                          "├ CCCC ───────┘  └ EEEEEEEE ┘ ",
                          "└➤ "];
        let layout = super::build_layout(30, dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(30, &layout, dt, &options);
        assert_eq!(super::format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn drop_left_2_1_stretch() {
        let options = LayoutOptions::new();
        let left = vec![Div::new("AAAA"), Div::new("BBBB"), Div::new("CCCCC")];
        let right = vec![Div::new("DDDD"), Div::new("EEEEEEEE")];
        let dt = "TTT";
        let result = vec!["┬──────┬──────┬──┬──────┐ TTT ",
                          "├ AAAA ┴ BBBB ┤  ├ DDDD ┴───┬─",
                          "├ CCCCC ──────┘  └ EEEEEEEE ┘ ",
                          "└➤ "];
        let layout = super::build_layout(30, dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(30, &layout, dt, &options);
        assert_eq!(super::format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn drop_left_2_1_shrink() {
        let options = LayoutOptions::new();
        let left = vec![Div::new("AAAA"), Div::new("BBBB"), Div::new("CC")];
        let right = vec![Div::new("DDDD"), Div::new("EEEEEEEE")];
        let dt = "TTT";
        let result = vec!["┬──────┬──────┬──┬──────┐ TTT ",
                          "├ AAAA ┴ BBBB ┤  ├ DDDD ┴───┬─",
                          "├ CC ─────────┘  └ EEEEEEEE ┘ ",
                          "└➤ "];
        let layout = super::build_layout(30, dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(30, &layout, dt, &options);
        assert_eq!(super::format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn drop_left_2_2() {
        let options = LayoutOptions::new();
        let left = vec![Div::new("AAAA"),
                        Div::new("BBBB"),
                        Div::new("CCCC"),
                        Div::new("DDDD")];
        let right = vec![Div::new("DDDD"), Div::new("EEEEEEEE")];
        let dt = "TTT";
        let result = vec!["┬──────┬──────┬──┬──────┐ TTT ",
                          "├ AAAA ┼ BBBB ┤  ├ DDDD ┴───┬─",
                          "├ CCCC ┴ DDDD ┘  └ EEEEEEEE ┘ ",
                          "└➤ "];
        let layout = super::build_layout(30, dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(30, &layout, dt, &options);
        assert_eq!(super::format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn drop_left_2_2_shrink() {
        let options = LayoutOptions::new();
        let left = vec![Div::new("AAAA"),
                        Div::new("BBBB"),
                        Div::new("CC"),
                        Div::new("DDDD")];
        let right = vec![Div::new("DDDD"), Div::new("EEEEEEEE")];
        let dt = "TTT";
        let result = vec!["┬──────┬──────┬──┬──────┐ TTT ",
                          "├ AAAA ┴ BBBB ┤  ├ DDDD ┴───┬─",
                          "├ CC ─ DDDD ──┘  └ EEEEEEEE ┘ ",
                          "└➤ "];
        let layout = super::build_layout(30, dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(30, &layout, dt, &options);
        assert_eq!(super::format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn drop_left_2_2_stretch() {
        let options = LayoutOptions::new();
        let left = vec![Div::new("AAAA"),
                        Div::new("BBBB"),
                        Div::new("CCCCC"),
                        Div::new("DDD")];
        let right = vec![Div::new("DDDD"), Div::new("EEEEEEEE")];
        let dt = "TTT";
        let result = vec!["┬──────┬──────┬──┬──────┐ TTT ",
                          "├ AAAA ┴ BBBB ┤  ├ DDDD ┴───┬─",
                          "├ CCCCC ─ DDD ┘  └ EEEEEEEE ┘ ",
                          "└➤ "];
        let layout = super::build_layout(30, dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(30, &layout, dt, &options);
        assert_eq!(super::format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn drop_left_2_2_stretch_more() {
        let options = LayoutOptions::new();
        let left = vec![Div::new("AAAA"),
                        Div::new("BBBB"),
                        Div::new("CCCCC"),
                        Div::new("DDDDD")];
        let right = vec![Div::new("DDDD"), Div::new("EEEEEEEE")];
        let dt = "TTT";
        let result = vec!["┬──────┬────────┬┬──────┐ TTT ",
                          "├ AAAA ┴ BBBB ──┤├ DDDD ┴───┬─",
                          "├ CCCCC ─ DDDDD ┘└ EEEEEEEE ┘ ",
                          "└➤ "];
        let layout = super::build_layout(30, dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(30, &layout, dt, &options);
        assert_eq!(super::format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn drop_left_3_2_stretch_more() {
        let options = LayoutOptions::new();
        let left = vec![Div::new("AAAA"),
                        Div::new("BBBB"),
                        Div::new("CCCCC"),
                        Div::new("DDDDD")];
        let right = vec![Div::new("DDDD"), Div::new("EEEEEEEE")];
        let dt = "TTT";
        let result = vec!["┬──────┬──────┬─┬──────┐ TTT ",
                          "├ AAAA ┴ BBBB ┤ ├ DDDD ┴───┬─",
                          "├ CCCCC ──────┤ └ EEEEEEEE ┘ ",
                          "├ DDDDD ──────┘              ",
                          "└➤ "];
        let layout = super::build_layout(29, dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(29, &layout, dt, &options);
        assert_eq!(super::format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn drop_right_long_short() {
        let options = LayoutOptions::new();
        let left = vec![Div::new("AAAA"), Div::new("BBBB"), Div::new("CCCC")];
        let right = vec![Div::new("DDDDDDDD"), Div::new("EEEE")];
        let dt = "TTT";
        let result = vec!["┬──────┬──────┬──────┬───┬──────────┐ TTT ",
                          "├ AAAA ┴ BBBB ┴ CCCC ┘   ├ DDDDDDDD ┼─────",
                          "│                        └ EEEE ────┘     ",
                          "└➤ "];
        let layout = super::build_layout(42, dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(42, &layout, dt, &options);
        assert_eq!(super::format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn drop_right_short_long() {
        let options = LayoutOptions::new();
        let left = vec![Div::new("AAAA"), Div::new("BBBB"), Div::new("CCCC")];
        let right = vec![Div::new("DDDD"), Div::new("EEEEEEEE")];
        let dt = "TTT";
        let result = vec!["┬──────┬──────┬──────┬───────┬──────┐ TTT ",
                          "├ AAAA ┴ BBBB ┴ CCCC ┘       ├ DDDD ┴───┬─",
                          "│                            └ EEEEEEEE ┘ ",
                          "└➤ "];
        let layout = super::build_layout(42, dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(42, &layout, dt, &options);
        assert_eq!(super::format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn drop_right_short_long_stretch1() {
        let options = LayoutOptions::new();
        let left = vec![Div::new("AAAA"), Div::new("BBBB"), Div::new("CCCC")];
        let right = vec![Div::new("DDDD"), Div::new("EEEEEEEEE")];
        let dt = "TTT";
        let result = vec!["┬──────┬──────┬──────┬───────┬──────┐ TTT ",
                          "├ AAAA ┴ BBBB ┴ CCCC ┘       ├ DDDD ┴────┬",
                          "│                            └ EEEEEEEEE ┘",
                          "└➤ "];
        let layout = super::build_layout(42, dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(42, &layout, dt, &options);
        assert_eq!(super::format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn drop_right_short_long_stretch2() {
        let options = LayoutOptions::new();
        let left = vec![Div::new("AAAA"), Div::new("BBBB"), Div::new("CCCC")];
        let right = vec![Div::new("DDDD"), Div::new("EEEEEEEEEE")];
        let dt = "TTT";
        let result = vec!["┬──────┬──────┬──────┬──────┬───────┐ TTT ",
                          "├ AAAA ┴ BBBB ┴ CCCC ┘      ├ DDDD ─┴────┬",
                          "│                           └ EEEEEEEEEE ┘",
                          "└➤ "];
        let layout = super::build_layout(42, dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(42, &layout, dt, &options);
        assert_eq!(super::format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn drop_right_short_long_stretch4() {
        let options = LayoutOptions::new();
        let left = vec![Div::new("AAAA"), Div::new("BBBB"), Div::new("CCCC")];
        let right = vec![Div::new("DDDD"), Div::new("EEEEEEEEEEEE")];
        let dt = "TTT";
        let result = vec!["┬──────┬──────┬──────┬────┬─────────┐ TTT ",
                          "├ AAAA ┴ BBBB ┴ CCCC ┘    ├ DDDD ───┴────┬",
                          "│                         └ EEEEEEEEEEEE ┘",
                          "└➤ "];
        let layout = super::build_layout(42, dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(42, &layout, dt, &options);
        assert_eq!(super::format_runs(&runs), result);
        //super::show_runs(&runs);
    }
}
