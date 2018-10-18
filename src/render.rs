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
use layout::{Div, Layout, Span};

use std;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Format {
    Clear,
    Border,
    Prompt,
    Span(String),
}

#[derive(Debug)]
pub struct Run {
    width: usize,
    cells: Vec<char>,
    formats: Vec<Option<Format>>,
    offset: usize,
    last_format: Format,
    use_color: bool,
    use_safe_corners: bool,
    clear_format: String,
    border_format: String,
    prompt_format: String,
}

impl Run {
    pub fn get_fallback_run() -> Vec<Self> {
        vec![Run {
            width: 2,
            cells: vec!['>', ' '],
            formats: vec![None, None],
            offset: 2,
            last_format: Format::Clear,
            use_color: false,
            use_safe_corners: false,
            clear_format: "".to_owned(),
            border_format: "".to_owned(),
            prompt_format: "".to_owned(),
        }]
    }

    fn new(width: usize, layout: &Layout) -> Self {
        Run {
            width,
            cells: std::iter::repeat(' ').take(width).collect::<Vec<char>>(),
            formats: std::iter::repeat(None)
                .take(width)
                .collect::<Vec<Option<Format>>>(),
            offset: 0,
            last_format: Format::Clear,
            use_color: layout.use_color,
            use_safe_corners: layout.use_safe_corners,
            clear_format: Span::get_reset_style(layout.escape_for_readline),
            border_format: layout.border_format.clone(),
            prompt_format: layout.prompt_format.clone(),
        }
    }

    fn add_formatted(&mut self, s: &str, fmt: Format) {
        if fmt != self.last_format {
            self.last_format = fmt.clone();
            self.formats[self.offset] = Some(fmt);
        }
        for c in s.chars() {
            self.cells[self.offset] = c;
            self.offset += 1;
        }
    }

    fn add(&mut self, s: &str) {
        self.add_formatted(s, Format::Clear);
    }

    fn add_border(&mut self, s: &str) {
        self.add_formatted(s, Format::Border);
    }

    fn add_prompt(&mut self, s: &str) {
        self.add_formatted(s, Format::Prompt);
    }

    fn repeat(&mut self, c: char, cnt: usize) {
        self.add(
            &std::iter::repeat(c.to_string())
                .take(cnt)
                .collect::<String>(),
        );
    }

    fn repeat_border(&mut self, c: char, cnt: usize) {
        self.add_border(
            &std::iter::repeat(c.to_string())
                .take(cnt)
                .collect::<String>(),
        );
    }

    fn add_div(&mut self, div: &Div, escape_for_readline: bool) {
        for span in div.iter_spans() {
            self.add_span(span, escape_for_readline);
        }
    }

    fn add_span(&mut self, span: &Span, escape_for_readline: bool) {
        self.add_formatted(
            &span.content,
            Format::Span(span.format_style(escape_for_readline)),
        );
    }

    fn is_border_at(&self, offset: usize) -> bool {
        match self.cells[offset] {
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
        }
    }

    fn find_time_corner_border(&self, start: usize) -> Option<usize> {
        let mut offset = start;
        while offset < self.width {
            if self.cells[offset] == '┐' {
                return Some(offset);
            }
            offset += 1;
        }
        None
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
        None
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
            _ => '_', // panic!("do not know how to add south border to: {}", self.cells[offset])
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
            _ => '_', //panic!("do not know how to add south border to: {}", self.cells[self.offset - 1])
        };
        self.cells[self.offset - 1] = next;
    }

    pub fn format(&self, escape_for_readline: bool) -> String {
        let mut out = "".to_owned();
        for (ch, maybe_fmt) in self.cells.iter().zip(self.formats.iter()) {
            if self.use_color {
                for fmt in maybe_fmt.iter() {
                    out += &Span::get_reset_style(escape_for_readline);
                    match fmt {
                        Format::Clear => {}
                        Format::Border => out += &self.border_format,
                        Format::Prompt => out += &self.prompt_format,
                        Format::Span(ref s) => out += &s,
                    }
                }
            }
            if !self.use_safe_corners {
                out.push(match *ch {
                    '┌' => '╭',
                    '└' => '╰',
                    '┐' => '╮',
                    '┘' => '╯',
                    c => c,
                });
            } else {
                out.push(*ch);
            }
        }
        out
    }

    pub fn show(&self, escape_for_readline: bool) {
        println!("{}", self.format(escape_for_readline));
    }

    pub fn show_all(runs: &[Run], escape_for_readline: bool) {
        for run in runs {
            run.show(escape_for_readline);
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
        let right_start =
            layout.width - (2 + layout.prior_runtime.width() + 1) - layout.right_extent;
        let right_end = right_start + layout.right_extent;

        // row 0
        let mut row0 = Run::new(layout.width, layout);
        row0.repeat_border('─', right_end);
        row0.add_border("┐");
        row0.add(" ");
        row0.add_div(&layout.prior_runtime, layout.escape_for_readline);
        row0.add(" ");
        runs.push(row0);

        // rows n+
        for i in 0..layout.height {
            let mut row = Run::new(layout.width, layout);
            runs[i].add_south_border(row.offset);
            row.add_border("│");

            // Emit LEFT
            if layout.left_by_row.len() > i {
                for f in &layout.left_by_row[i] {
                    row.add_east_border();
                    row.add(" ");
                    row.add_div(f, layout.escape_for_readline);
                    row.add(" ");

                    if f == layout.left_by_row[i].last().unwrap() {
                        let to_right = layout.left_extent - row.offset;
                        row.repeat_border('─', to_right);
                    }
                    if runs[i].is_border_at(row.offset) {
                        runs[i].add_south_border(row.offset);
                        row.add_border("┘");
                    } else {
                        row.add_border("─");
                    }
                }
            }

            // Emit CENTER
            let to_right = right_start - row.offset;
            row.repeat(' ', to_right);

            // Emit RIGHT
            if layout.right_by_row.len() > i {
                runs[i].add_south_border(row.offset);
                row.add_border("└");
                for f in &layout.right_by_row[i] {
                    row.add_east_border();
                    row.add(" ");
                    row.add_div(f, layout.escape_for_readline);
                    row.add(" ");

                    if i == 0 && f == layout.right_by_row[i].last().unwrap() {
                        if let Some(next_border) = runs[i].find_time_corner_border(row.offset) {
                            let offset = next_border - row.offset;
                            if offset > 0 {
                                row.repeat_border('─', offset);
                            }
                        }
                    } else if let Some(next_border) = runs[i].find_next_border(row.offset) {
                        let offset = next_border - row.offset;
                        if offset > 0 {
                            row.repeat_border('─', offset);
                        }
                    }
                    runs[i].add_south_border(row.offset);
                    row.add_border("┘");
                }
                if i == 0 {
                    let to_end = layout.width - row.offset;
                    row.add_east_border();
                    row.repeat_border('─', to_end);
                }
            }
            runs.push(row);
        }

        let mut run_last = Run::new(3, layout);
        run_last.add_border("└");
        let arrow = if layout.use_safe_arrow { ">" } else { "➤" };
        run_last.add_prompt(arrow);
        run_last.add(" ");

        runs.push(run_last);
        runs
    }
}
