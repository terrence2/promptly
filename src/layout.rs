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
use std::cmp;

#[derive(Debug, PartialEq)]
pub struct Span {
    pub color: &'static str,
    pub content: String,
}

impl Span {
    pub fn new(content: &str) -> Self {
        Span {
            color: "",
            content: content.to_owned(),
        }
    }

    pub fn width(&self) -> usize {
        return self.content.chars().count();
    }
}

#[derive(Debug, PartialEq)]
pub struct Div {
    pub children: Vec<Span>,
}

impl Div {
    pub fn new(a: &str) -> Self {
        Div { children: vec![Span::new(a)] }
    }

    pub fn new3(a: &str, b: &str, c: &str) -> Self {
        Div { children: vec![Span::new(a), Span::new(b), Span::new(c)] }
    }

    pub fn width(&self) -> usize {
        return self.children.iter().map(|s| s.width()).sum();
    }
}

pub struct LayoutOptions {
    pub verbose: bool,
}

impl LayoutOptions {
    pub fn new() -> LayoutOptions {
        LayoutOptions { verbose: false }
    }

    pub fn verbose(mut self, value: bool) -> LayoutOptions {
        self.verbose = value;
        return self;
    }
}

pub struct Layout {
    pub left_extent: usize,
    pub right_extent: usize,
    pub height: usize,
    pub left_by_row: Vec<Vec<Div>>,
    pub right_by_row: Vec<Vec<Div>>,
}

impl Layout {
    pub fn new(left_extent: usize,
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
pub fn build_layout(columns: usize,
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
