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
extern crate hostname;
extern crate time;
extern crate users;

mod errors {
    error_chain!{}
}
mod layout;
mod render;

use layout::{Color, Div, Span, Layout, LayoutOptions};
use render::Run;

use chrono::Local;
use errors::{Result, ResultExt};
use git2::Repository;
use hostname::get_hostname;
use std::env::{current_dir, var};
use time::PreciseTime;
use users::{get_current_username, get_effective_uid};

const DATE_FORMAT: &str = "%d %b %H:%M:%S";

quick_main!(run);
fn run() -> Result<()> {
    let parser = clap_app!(promptly =>
        (version: "0.1")
        (author: "Terrence Cole <terrence.d.cole@gmail.com>")
        (about: "Shows a shell prompt, quickly.")
        (@arg status: -s --status <CODE> "Prior command exit code.")
        (@arg time: -t --time <SECONDS> "Prior command run time.")
        (@arg width: -w --width <COLUMNS> "The terminal width to use.")
        (@arg safe_arrow: --("safe-arrow") "Use a non-utf8 arrow character.")
        (@arg alternate_home: --("alternate-home") <PATH> "Specify a non-$HOME, home folding.")
        (@arg show_timings: --("show-timings") "Print out timings after the prompt.")
        (@arg verbose: -v --verbose "Sets the level of debugging information.")
    );
    let args = parser.get_matches();

    let show_timings = args.occurrences_of("show_timings") > 0;
    let columns = args
        .value_of("width")
        .unwrap()
        .parse::<usize>()
        .chain_err(|| "expected positive integer width")?;
    let prior_runtime_seconds = args
        .value_of("time")
        .unwrap()
        .parse::<i32>()
        .chain_err(|| "expected integer time")?;

    let border_template = match args.value_of("status").unwrap() == "0" {
        true => Span::new("").foreground(Color::Blue).bold(),
        false => Span::new("").foreground(Color::Red).bold(),
    };
    let prompt_template = Span::new("").foreground(Color::Green).dimmed();

    let mut left_floats = Vec::<Div>::new();
    let mut right_floats = Vec::<Div>::new();

    let t1 = if show_timings { Some(PreciseTime::now()) } else { None };
    let path_div = format_path(args.value_of("alternate_home"))
        .chain_err(|| "failed to format the path")?;
    left_floats.push(path_div);

    let t2 = if show_timings { Some(PreciseTime::now()) } else { None };
    let current_time = Local::now();
    right_floats.push(Div::new(Span::new(&current_time.format(DATE_FORMAT).to_string())));

    let t3 = if show_timings { Some(PreciseTime::now()) } else { None };
    let git_branch = find_git_branch();
    git_branch.map(|branch| left_floats.push(format_git_branch(&branch)));

    let t4 = if show_timings { Some(PreciseTime::now()) } else { None };
    let prior_runtime = format_run_time(prior_runtime_seconds);

    let t5 = if show_timings { Some(PreciseTime::now()) } else { None };
    right_floats.push(format_user_host());

    let t6 = if show_timings { Some(PreciseTime::now()) } else { None };
    let options = LayoutOptions::new()
        .verbose(args.occurrences_of("verbose") > 0)
        .use_safe_arrow(args.occurrences_of("safe_arrow") > 0)
        .border_template(border_template)
        .prompt_template(prompt_template)
        .width(columns);
    let runs = match Layout::build(prior_runtime, left_floats, right_floats, &options) {
        Some(layout) => { Run::render_layout(&layout) },
        None => Run::get_fallback_run(),
    };
    let t7 = if show_timings { Some(PreciseTime::now()) } else { None };
    Run::show_all(&runs);
    let t8 = if show_timings { Some(PreciseTime::now()) } else { None };
    if show_timings {
        println!("Fmt Path:      {}", t1.unwrap().to(t2.unwrap()));
        println!("Fmt Date:      {}", t2.unwrap().to(t3.unwrap()));
        println!("Fmt Git:       {}", t3.unwrap().to(t4.unwrap()));
        println!("Fmt Runtime:   {}", t4.unwrap().to(t5.unwrap()));
        println!("Fmt User/Host: {}", t5.unwrap().to(t6.unwrap()));
        println!("Layout&Render: {}", t6.unwrap().to(t7.unwrap()));
        println!("Writing:       {}", t7.unwrap().to(t8.unwrap()));
        println!("Total:         {}", t1.unwrap().to(t8.unwrap()));
    }
    return Ok(());
}

fn format_path(alt_home: Option<&str>) -> Result<Div> {
    let path = current_dir().chain_err(|| "failed to getcwd")?;
    let raw_path_str = path.to_str().unwrap_or("<error>");
    let home_str = match alt_home {
        None => var("HOME").chain_err(|| "failed to get HOME")?,
        Some(alt) => alt.to_owned(),
    };
    let path_str = match raw_path_str.starts_with(&home_str) {
        true => raw_path_str.replace(&home_str, "~"),
        false => raw_path_str.to_owned(),
    };
    return Ok(Div::new(Span::new(&path_str).bold()));
}

fn format_run_time(t: i32) -> Div {
    let mut out = Div::new_empty();
    if t == 0 {
        out.add_span(Span::new("ε").foreground(Color::Purple).bold());
        return out;
    }

    let mut s = t;
    if s > 3600 {
        let h = s / 3600;
        s = s - 3600 * h;
        out.add_span(Span::new(&format!("{}", h))
                         .foreground(Color::Purple)
                         .bold());
        out.add_span(Span::new("h").foreground(Color::Purple).dimmed());
    }
    if s > 60 {
        let m = s / 60;
        s = s - 60 * m;
        out.add_span(Span::new(&format!("{}", m))
                         .foreground(Color::Purple)
                         .bold());
        out.add_span(Span::new("m").foreground(Color::Purple).dimmed());
    }
    if s > 0 {
        out.add_span(Span::new(&format!("{}", s))
                         .foreground(Color::Purple)
                         .bold());
        out.add_span(Span::new("s").foreground(Color::Purple).dimmed());
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

fn format_git_branch(branch: &str) -> Div {
    let mut div = Div::new_empty();
    div.add_span(Span::new("@").foreground(Color::Yellow));
    div.add_span(Span::new("git").foreground(Color::Cyan));
    div.add_span(Span::new("{").bold());
    div.add_span(Span::new(branch).foreground(Color::Yellow).bold());
    div.add_span(Span::new("}").bold());
    return div;
}

fn format_user_host() -> Div {
    let username = match get_current_username() {
        None => "<unknown_user>".to_owned(),
        Some(un) => un,
    };
    let hostname = match get_hostname() {
        None => "<unknown_host>".to_owned(),
        Some(hn) => hn,
    };
    let mut span = Span::new(&username);
    span = match get_effective_uid() {
        0 => span.foreground(Color::Red).bold(),
        _ => span.foreground(Color::Blue).dimmed(),
    };
    let mut div = Div::new(span);
    div.add_span(Span::new("@").foreground(Color::White).dimmed());
    div.add_span(Span::new(&hostname).foreground(Color::Green).dimmed());
    return div;
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::Layout;

    fn format_runs(runs: &Vec<Run>) -> Vec<String> {
        runs.iter().map(|r| r.format()).collect::<Vec<String>>()
    }

    fn do_test(width: usize, dt_str: &str, left: Vec<&str>, right: Vec<&str>, result: Vec<&str>) {
        let options = LayoutOptions::new().width(width).use_color(false);
        let dt = Div::new(Span::new(dt_str));
        let l = left.iter()
            .map(|s| Div::new(Span::new(s)))
            .collect::<Vec<Div>>();
        let r = right
            .iter()
            .map(|s| Div::new(Span::new(s)))
            .collect::<Vec<Div>>();
        let layout = Layout::build(dt, l, r, &options).unwrap();
        let runs = Run::render_layout(&layout);
        assert_eq!(format_runs(&runs), result);
        //for r in runs { r.show(); }
    }

    #[test]
    fn single_line() {
        do_test(80,
                "TTT",
                vec!["AAAA", "BBBB", "CCCC"],
                vec!["DDDD", "EEEE"],
                vec!["┬──────┬──────┬──────┬──────────────────────────────────────┬──────┬──────┐ TTT ",
                     "├ AAAA ┴ BBBB ┴ CCCC ┘                                      └ DDDD ┴ EEEE ┴─────",
                     "└➤ "]);
    }

    #[test]
    fn single_line_min() {
        do_test(43,
                "TTT",
                vec!["AAAA", "BBBB", "CCCC"],
                vec!["DDDD", "EEEE"],
                vec!["┬──────┬──────┬──────┬─┬──────┬──────┐ TTT ",
                     "├ AAAA ┴ BBBB ┴ CCCC ┘ └ DDDD ┴ EEEE ┴─────",
                     "└➤ "]);
    }

    #[test]
    fn drop_left_2_1() {
        do_test(30,
                "TTT",
                vec!["AAAA", "BBBB", "CCCC"],
                vec!["DDDD", "EEEEEEEE"],
                vec!["┬──────┬──────┬──┬──────┐ TTT ",
                     "├ AAAA ┴ BBBB ┤  ├ DDDD ┴───┬─",
                     "├ CCCC ───────┘  └ EEEEEEEE ┘ ",
                     "└➤ "]);
    }

    #[test]
    fn drop_left_2_1_stretch() {
        do_test(30,
                "TTT",
                vec!["AAAA", "BBBB", "CCCCC"],
                vec!["DDDD", "EEEEEEEE"],
                vec!["┬──────┬──────┬──┬──────┐ TTT ",
                     "├ AAAA ┴ BBBB ┤  ├ DDDD ┴───┬─",
                     "├ CCCCC ──────┘  └ EEEEEEEE ┘ ",
                     "└➤ "]);
    }

    #[test]
    fn drop_left_2_1_shrink() {
        do_test(30,
                "TTT",
                vec!["AAAA", "BBBB", "CC"],
                vec!["DDDD", "EEEEEEEE"],
                vec!["┬──────┬──────┬──┬──────┐ TTT ",
                     "├ AAAA ┴ BBBB ┤  ├ DDDD ┴───┬─",
                     "├ CC ─────────┘  └ EEEEEEEE ┘ ",
                     "└➤ "]);
    }

    #[test]
    fn drop_left_2_2() {
        do_test(30,
                "TTT",
                vec!["AAAA", "BBBB", "CCCC", "DDDD"],
                vec!["DDDD", "EEEEEEEE"],
                vec!["┬──────┬──────┬──┬──────┐ TTT ",
                     "├ AAAA ┼ BBBB ┤  ├ DDDD ┴───┬─",
                     "├ CCCC ┴ DDDD ┘  └ EEEEEEEE ┘ ",
                     "└➤ "]);
    }

    #[test]
    fn drop_left_2_2_shrink() {
        do_test(30,
                "TTT",
                vec!["AAAA", "BBBB", "CC", "DDDD"],
                vec!["DDDD", "EEEEEEEE"],
                vec!["┬──────┬──────┬──┬──────┐ TTT ",
                     "├ AAAA ┴ BBBB ┤  ├ DDDD ┴───┬─",
                     "├ CC ─ DDDD ──┘  └ EEEEEEEE ┘ ",
                     "└➤ "]);
    }

    #[test]
    fn drop_left_2_2_stretch() {
        do_test(30,
                "TTT",
                vec!["AAAA", "BBBB", "CCCCC", "DDD"],
                vec!["DDDD", "EEEEEEEE"],
                vec!["┬──────┬──────┬──┬──────┐ TTT ",
                     "├ AAAA ┴ BBBB ┤  ├ DDDD ┴───┬─",
                     "├ CCCCC ─ DDD ┘  └ EEEEEEEE ┘ ",
                     "└➤ "]);
    }

    #[test]
    fn drop_left_2_2_stretch_more() {
        do_test(30,
                "TTT",
                vec!["AAAA", "BBBB", "CCCCC", "DDDDD"],
                vec!["DDDD", "EEEEEEEE"],
                vec!["┬──────┬────────┬┬──────┐ TTT ",
                     "├ AAAA ┴ BBBB ──┤├ DDDD ┴───┬─",
                     "├ CCCCC ─ DDDDD ┘└ EEEEEEEE ┘ ",
                     "└➤ "]);
    }

    #[test]
    fn drop_left_3_2_stretch_more() {
        do_test(29,
                "TTT",
                vec!["AAAA", "BBBB", "CCCCC", "DDDDD"],
                vec!["DDDD", "EEEEEEEE"],
                vec!["┬──────┬──────┬─┬──────┐ TTT ",
                     "├ AAAA ┴ BBBB ┤ ├ DDDD ┴───┬─",
                     "├ CCCCC ──────┤ └ EEEEEEEE ┘ ",
                     "├ DDDDD ──────┘              ",
                     "└➤ "]);
    }

    #[test]
    fn drop_right_long_short() {
        do_test(42,
                "TTT",
                vec!["AAAA", "BBBB", "CCCC"],
                vec!["DDDDDDDD", "EEEE"],
                vec!["┬──────┬──────┬──────┬───┬──────────┐ TTT ",
                     "├ AAAA ┴ BBBB ┴ CCCC ┘   ├ DDDDDDDD ┼─────",
                     "│                        └ EEEE ────┘     ",
                     "└➤ "]);
    }

    #[test]
    fn drop_right_short_long() {
        do_test(42,
                "TTT",
                vec!["AAAA", "BBBB", "CCCC"],
                vec!["DDDD", "EEEEEEEE"],
                vec!["┬──────┬──────┬──────┬───────┬──────┐ TTT ",
                     "├ AAAA ┴ BBBB ┴ CCCC ┘       ├ DDDD ┴───┬─",
                     "│                            └ EEEEEEEE ┘ ",
                     "└➤ "]);
    }

    #[test]
    fn drop_right_short_long_stretch1() {
        do_test(42,
                "TTT",
                vec!["AAAA", "BBBB", "CCCC"],
                vec!["DDDD", "EEEEEEEEE"],
                vec!["┬──────┬──────┬──────┬───────┬──────┐ TTT ",
                     "├ AAAA ┴ BBBB ┴ CCCC ┘       ├ DDDD ┴────┬",
                     "│                            └ EEEEEEEEE ┘",
                     "└➤ "]);
    }

    #[test]
    fn drop_right_short_long_stretch2() {
        do_test(42,
                "TTT",
                vec!["AAAA", "BBBB", "CCCC"],
                vec!["DDDD", "EEEEEEEEEE"],
                vec!["┬──────┬──────┬──────┬──────┬───────┐ TTT ",
                     "├ AAAA ┴ BBBB ┴ CCCC ┘      ├ DDDD ─┴────┬",
                     "│                           └ EEEEEEEEEE ┘",
                     "└➤ "]);
    }

    #[test]
    fn drop_right_short_long_stretch4() {
        do_test(42,
                "TTT",
                vec!["AAAA", "BBBB", "CCCC"],
                vec!["DDDD", "EEEEEEEEEEEE"],
                vec!["┬──────┬──────┬──────┬────┬─────────┐ TTT ",
                     "├ AAAA ┴ BBBB ┴ CCCC ┘    ├ DDDD ───┴────┬",
                     "│                         └ EEEEEEEEEEEE ┘",
                     "└➤ "]);
    }
}
