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
use layout::{Div, Span, Layout};

use std;

#[derive(Debug)]
pub struct Run {
    width: usize,
    cells: Vec<char>,
    formats: Vec<Option<&'static str>>,
    offset: usize,
    last_format: &'static str,
}

impl Run {
    pub fn get_fallback_run() -> Vec<Self> {
        let mut fail_run = Run::new(2);
        fail_run.add("➤", "prompt");
        fail_run.add(" ", "clear");
        return vec![fail_run];
    }

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
        for span in div.iter_spans() {
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

    pub fn format(&self) -> String {
        let mut out = "".to_owned();
        for c in self.cells.iter() {
            out.push(*c);
        }
        return out;
    }

    pub fn show(&self) {
        println!("{}", self.format());
    }

    pub fn show_all(runs: &Vec<Run>) {
        for run in runs {
            run.show();
        }
    }

    pub fn render_layout(layout: &Layout) -> Vec<Self> {
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
        let right_start = layout.width - (2 + layout.prior_runtime.width() + 1) -
                          layout.right_extent;
        let right_end = right_start + layout.right_extent;

        // row 0
        let mut row0 = Run::new(layout.width);
        row0.repeat('─', right_end, "border");
        row0.add("┐", "border");
        row0.add(" ", "clear");
        row0.add_div(&layout.prior_runtime);
        row0.add(" ", "clear");
        runs.push(row0);

        // rows n+
        for i in 0..layout.height {
            let mut row = Run::new(layout.width);
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
                    let to_end = layout.width - row.offset;
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
}
