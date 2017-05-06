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
#[macro_use]
extern crate clap;
extern crate git2;

mod errors {
    error_chain!{}
}
mod layout;
mod render;

use layout::{Div, Layout, LayoutOptions};
use render::{Run, render_with_layout};

use chrono::Local;
use errors::*;
use git2::Repository;
use std::env::{current_dir, var};

const DATE_FORMAT: &str = "%d %b %H:%M:%S";

quick_main!(run);
fn run() -> Result<()> {
    let matches = clap_app!(promptly =>
        (version: "0.1")
        (author: "Terrence Cole <terrence.d.cole@gmail.com>")
        (about: "Shows a shell prompt, quickly.")
        (@arg status: -s --status <CODE> "Prior command exit code.")
        (@arg time: -t --time <SECONDS> "Prior command run time.")
        (@arg width: -w --width <COLUMNS> "The terminal width to use.")
        (@arg verbose: -v --verbose "Sets the level of debugging information")
    )
            .get_matches();
    let status = matches.value_of("status").unwrap() == "0";
    let columns = matches
        .value_of("width")
        .unwrap()
        .parse::<usize>()
        .chain_err(|| "expected positive integer width")?;
    let prior_runtime = matches
        .value_of("time")
        .unwrap()
        .parse::<i32>()
        .chain_err(|| "expected integer time")?;

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
    left_floats.push(Div::new(&path_str));

    let current_time = Local::now();
    right_floats.push(Div::new(&current_time.format(DATE_FORMAT).to_string()));

    let git_branch = find_git_branch();
    git_branch.map(|branch| left_floats.push(Div::new3("@ git {", &branch, "}")));

    let prior_runtime_str = format_run_time(prior_runtime);

    let options = LayoutOptions::new()
        .verbose(matches.occurrences_of("verbose") > 0)
        .width(columns);
    let runs = match Layout::build(&prior_runtime_str, left_floats, right_floats, &options) {
        None => {
            let mut fail_run = Run::new(2);
            fail_run.add("➤", "prompt");
            fail_run.add(" ", "clear");
            vec![fail_run]
        }
        Some(layout) => render_with_layout(columns, &layout, &prior_runtime_str),
    };
    Run::show_all(&runs);

    return Ok(());
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
    use super::Layout;

    fn format_runs(layout: &Vec<Run>) -> Vec<String> {
        layout
            .iter()
            .map(|r| r.format())
            .collect::<Vec<String>>()
    }

    #[test]
    fn single_line() {
        let options = LayoutOptions::new().width(80);
        let left = vec![Div::new("AAAA"), Div::new("BBBB"), Div::new("CCCC")];
        let right = vec![Div::new("DDDD"), Div::new("EEEE")];
        let dt = "TTT";
        let result = vec!["┬──────┬──────┬──────┬──────────────────────────────────────┬──────┬──────┐ TTT ",
                          "├ AAAA ┴ BBBB ┴ CCCC ┘                                      └ DDDD ┘ EEEE ┴─────",
                          "└➤ "];
        let layout = Layout::build(dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(80, &layout, dt);
        assert_eq!(format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn single_line_min() {
        let options = LayoutOptions::new().width(43);
        let left = vec![Div::new("AAAA"), Div::new("BBBB"), Div::new("CCCC")];
        let right = vec![Div::new("DDDD"), Div::new("EEEE")];
        let dt = "TTT";
        let result = vec!["┬──────┬──────┬──────┬─┬──────┬──────┐ TTT ",
                          "├ AAAA ┴ BBBB ┴ CCCC ┘ └ DDDD ┘ EEEE ┴─────",
                          "└➤ "];
        let layout = Layout::build(dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(43, &layout, dt);
        assert_eq!(format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn drop_left_2_1() {
        let options = LayoutOptions::new().width(30);
        let left = vec![Div::new("AAAA"), Div::new("BBBB"), Div::new("CCCC")];
        let right = vec![Div::new("DDDD"), Div::new("EEEEEEEE")];
        let dt = "TTT";
        let result = vec!["┬──────┬──────┬──┬──────┐ TTT ",
                          "├ AAAA ┴ BBBB ┤  ├ DDDD ┴───┬─",
                          "├ CCCC ───────┘  └ EEEEEEEE ┘ ",
                          "└➤ "];
        let layout = Layout::build(dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(30, &layout, dt);
        assert_eq!(format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn drop_left_2_1_stretch() {
        let options = LayoutOptions::new().width(30);
        let left = vec![Div::new("AAAA"), Div::new("BBBB"), Div::new("CCCCC")];
        let right = vec![Div::new("DDDD"), Div::new("EEEEEEEE")];
        let dt = "TTT";
        let result = vec!["┬──────┬──────┬──┬──────┐ TTT ",
                          "├ AAAA ┴ BBBB ┤  ├ DDDD ┴───┬─",
                          "├ CCCCC ──────┘  └ EEEEEEEE ┘ ",
                          "└➤ "];
        let layout = Layout::build(dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(30, &layout, dt);
        assert_eq!(format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn drop_left_2_1_shrink() {
        let options = LayoutOptions::new().width(30);
        let left = vec![Div::new("AAAA"), Div::new("BBBB"), Div::new("CC")];
        let right = vec![Div::new("DDDD"), Div::new("EEEEEEEE")];
        let dt = "TTT";
        let result = vec!["┬──────┬──────┬──┬──────┐ TTT ",
                          "├ AAAA ┴ BBBB ┤  ├ DDDD ┴───┬─",
                          "├ CC ─────────┘  └ EEEEEEEE ┘ ",
                          "└➤ "];
        let layout = Layout::build(dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(30, &layout, dt);
        assert_eq!(format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn drop_left_2_2() {
        let options = LayoutOptions::new().width(30);
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
        let layout = Layout::build(dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(30, &layout, dt);
        assert_eq!(format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn drop_left_2_2_shrink() {
        let options = LayoutOptions::new().width(30);
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
        let layout = Layout::build(dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(30, &layout, dt);
        assert_eq!(format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn drop_left_2_2_stretch() {
        let options = LayoutOptions::new().width(30);
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
        let layout = Layout::build(dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(30, &layout, dt);
        assert_eq!(format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn drop_left_2_2_stretch_more() {
        let options = LayoutOptions::new().width(30);
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
        let layout = Layout::build(dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(30, &layout, dt);
        assert_eq!(format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn drop_left_3_2_stretch_more() {
        let options = LayoutOptions::new().width(29);
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
        let layout = Layout::build(dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(29, &layout, dt);
        assert_eq!(format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn drop_right_long_short() {
        let options = LayoutOptions::new().width(42);
        let left = vec![Div::new("AAAA"), Div::new("BBBB"), Div::new("CCCC")];
        let right = vec![Div::new("DDDDDDDD"), Div::new("EEEE")];
        let dt = "TTT";
        let result = vec!["┬──────┬──────┬──────┬───┬──────────┐ TTT ",
                          "├ AAAA ┴ BBBB ┴ CCCC ┘   ├ DDDDDDDD ┼─────",
                          "│                        └ EEEE ────┘     ",
                          "└➤ "];
        let layout = Layout::build(dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(42, &layout, dt);
        assert_eq!(format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn drop_right_short_long() {
        let options = LayoutOptions::new().width(42);
        let left = vec![Div::new("AAAA"), Div::new("BBBB"), Div::new("CCCC")];
        let right = vec![Div::new("DDDD"), Div::new("EEEEEEEE")];
        let dt = "TTT";
        let result = vec!["┬──────┬──────┬──────┬───────┬──────┐ TTT ",
                          "├ AAAA ┴ BBBB ┴ CCCC ┘       ├ DDDD ┴───┬─",
                          "│                            └ EEEEEEEE ┘ ",
                          "└➤ "];
        let layout = Layout::build(dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(42, &layout, dt);
        assert_eq!(format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn drop_right_short_long_stretch1() {
        let options = LayoutOptions::new().width(42);
        let left = vec![Div::new("AAAA"), Div::new("BBBB"), Div::new("CCCC")];
        let right = vec![Div::new("DDDD"), Div::new("EEEEEEEEE")];
        let dt = "TTT";
        let result = vec!["┬──────┬──────┬──────┬───────┬──────┐ TTT ",
                          "├ AAAA ┴ BBBB ┴ CCCC ┘       ├ DDDD ┴────┬",
                          "│                            └ EEEEEEEEE ┘",
                          "└➤ "];
        let layout = Layout::build(dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(42, &layout, dt);
        assert_eq!(format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn drop_right_short_long_stretch2() {
        let options = LayoutOptions::new().width(42);
        let left = vec![Div::new("AAAA"), Div::new("BBBB"), Div::new("CCCC")];
        let right = vec![Div::new("DDDD"), Div::new("EEEEEEEEEE")];
        let dt = "TTT";
        let result = vec!["┬──────┬──────┬──────┬──────┬───────┐ TTT ",
                          "├ AAAA ┴ BBBB ┴ CCCC ┘      ├ DDDD ─┴────┬",
                          "│                           └ EEEEEEEEEE ┘",
                          "└➤ "];
        let layout = Layout::build(dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(42, &layout, dt);
        assert_eq!(format_runs(&runs), result);
        //super::show_runs(&runs);
    }

    #[test]
    fn drop_right_short_long_stretch4() {
        let options = LayoutOptions::new().width(42);
        let left = vec![Div::new("AAAA"), Div::new("BBBB"), Div::new("CCCC")];
        let right = vec![Div::new("DDDD"), Div::new("EEEEEEEEEEEE")];
        let dt = "TTT";
        let result = vec!["┬──────┬──────┬──────┬────┬─────────┐ TTT ",
                          "├ AAAA ┴ BBBB ┴ CCCC ┘    ├ DDDD ───┴────┬",
                          "│                         └ EEEEEEEEEEEE ┘",
                          "└➤ "];
        let layout = Layout::build(dt, left, right, &options).unwrap();
        let runs = super::render_with_layout(42, &layout, dt);
        assert_eq!(format_runs(&runs), result);
        //super::show_runs(&runs);
    }
}
