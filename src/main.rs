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
extern crate chrono;
#[macro_use]
extern crate clap;
extern crate failure;
extern crate git2;
extern crate hostname;
extern crate time;
extern crate users;

mod layout;
mod render;

use layout::{Color, Div, Layout, LayoutOptions, Span};
use render::Run;

use chrono::Local;
use failure::Fallible;
use git2::Repository;
use hostname::get_hostname;
use std::env::{current_dir, var};
use time::PreciseTime;
use users::{get_current_username, get_effective_uid};

fn main() -> Fallible<()> {
    let parser = clap_app!(promptly =>
        (version: "0.1")
        (author: "Terrence Cole <terrence.d.cole@gmail.com>")
        (about: "Shows a shell prompt, quickly.")
        (@arg status: -s --status <CODE> "Prior command exit code.")
        (@arg time: -t --time <SECONDS> "Prior command run time.")
        (@arg width: -w --width <COLUMNS> "The terminal width to use.")
        (@arg safe_arrow: --("safe-arrow") "Use a non-utf8 arrow character.")
        (@arg safe_corners: --("safe-corners") "Use normal box corners instead of round corners.")
        (@arg no_readline: --("no-readline") "Skip the readline escaping we do by default.")
        (@arg alternate_home: --("alternate-home") <PATH> !required "Specify a non-$HOME, home folding.")
        (@arg timed: --("show-timings") "Print out timings after the prompt.")
        (@arg verbose: -v --verbose "Sets the level of debugging information.")
    );
    let args = parser.get_matches();

    let timed = args.occurrences_of("timed") > 0;
    let columns = args.value_of("width").unwrap().parse::<usize>()?;
    let prior_runtime_seconds = args.value_of("time").unwrap().parse::<i32>()?;

    let border_template = if args.value_of("status").unwrap() == "0" {
        Span::new("").foreground(Color::Blue).bold()
    } else {
        Span::new("").foreground(Color::Red).bold()
    };
    let prompt_template = Span::new("").foreground(Color::Green).dimmed();
    let prior_runtime = format_run_time(prior_runtime_seconds);

    let mut left_floats = Vec::<Div>::new();
    let mut right_floats = Vec::<Div>::new();

    let t1 = get_time(timed);
    let path_div = format_path(args.value_of("alternate_home"))?;
    left_floats.push(path_div);

    let t2 = get_time(timed);
    let git_branch = find_git_branch();
    if let Some(branch) = git_branch {
        left_floats.push(format_git_branch(&branch));
    }

    let t3 = get_time(timed);
    right_floats.push(format_date_time());

    let t5 = get_time(timed);
    right_floats.push(format_user_host());

    let t6 = get_time(timed);
    let options = LayoutOptions::new()
        .verbose(args.occurrences_of("verbose") > 0)
        .use_safe_arrow(args.occurrences_of("safe_arrow") > 0)
        .use_safe_corners(args.occurrences_of("safe_corners") > 0)
        .escape_for_readline(args.occurrences_of("no_readline") == 0)
        .border_template(border_template)
        .prompt_template(prompt_template)
        .width(columns);
    let runs = match Layout::build(prior_runtime, left_floats, right_floats, &options) {
        Some(layout) => Run::render_layout(&layout),
        None => Run::get_fallback_run(),
    };
    let t7 = get_time(timed);
    Run::show_all(&runs, options.escape_for_readline);
    let t8 = get_time(timed);
    if timed {
        println!("Fmt Path:      {}", t1.unwrap().to(t2.unwrap()));
        println!("Fmt Git:       {}", t2.unwrap().to(t3.unwrap()));
        println!("Fmt Date:      {}", t3.unwrap().to(t5.unwrap()));
        println!("Fmt User/Host: {}", t5.unwrap().to(t6.unwrap()));
        println!("Layout&Render: {}", t6.unwrap().to(t7.unwrap()));
        println!("Writing:       {}", t7.unwrap().to(t8.unwrap()));
        println!("Total:         {}", t1.unwrap().to(t8.unwrap()));
    }
    Ok(())
}

fn get_time(timed: bool) -> Option<PreciseTime> {
    if timed {
        return Some(PreciseTime::now());
    }
    None
}

fn format_path(alt_home: Option<&str>) -> Fallible<Div> {
    let path = current_dir()?;
    let raw_path_str = path.to_str().unwrap_or("<error>");
    let home_str = match alt_home {
        None => var("HOME")?,
        Some(alt) => alt.to_owned(),
    };
    let path_str = if raw_path_str.starts_with(&home_str) {
        raw_path_str.replace(&home_str, "~")
    } else {
        raw_path_str.to_owned()
    };
    Ok(Div::new(Span::new(&path_str).bold()))
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
        s -= 3600 * h;
        out.add_span(
            Span::new(&format!("{}", h))
                .foreground(Color::Purple)
                .bold(),
        );
        out.add_span(Span::new("h").foreground(Color::Purple).dimmed());
    }
    if s > 60 {
        let m = s / 60;
        s -= 60 * m;
        out.add_span(
            Span::new(&format!("{}", m))
                .foreground(Color::Purple)
                .bold(),
        );
        out.add_span(Span::new("m").foreground(Color::Purple).dimmed());
    }
    if s > 0 {
        out.add_span(
            Span::new(&format!("{}", s))
                .foreground(Color::Purple)
                .bold(),
        );
        out.add_span(Span::new("s").foreground(Color::Purple).dimmed());
    }
    out
}

fn find_git_branch() -> Option<String> {
    for path in &[".", "..", "../..", "../../.."] {
        if let Some(branch) = find_git_branch_at(path) {
            return Some(branch);
        }
    }
    None
}

fn find_git_branch_at(path: &'static str) -> Option<String> {
    let repo = match Repository::open(path) {
        Ok(repo) => repo,
        Err(_) => return None,
    };
    let head = match repo.head() {
        Ok(head) => head,
        Err(_) => return None,
    };
    Some(
        match head.shorthand() {
            Some(tgt) => tgt,
            None => "(detached)",
        }.to_owned(),
    )
}

fn format_git_branch(branch: &str) -> Div {
    let mut div = Div::new_empty();
    div.add_span(Span::new("@").foreground(Color::Yellow));
    div.add_span(Span::new("git").foreground(Color::Cyan));
    div.add_span(Span::new("{").bold());
    div.add_span(Span::new(branch).foreground(Color::Yellow).bold());
    div.add_span(Span::new("}").bold());
    div
}

fn format_date_time() -> Div {
    let current_time = Local::now();
    let mut div = Div::new_empty();
    div.add_span(Span::new(&current_time.format("%d ").to_string()).foreground(Color::Green));
    div.add_span(Span::new(&current_time.format("%b ").to_string()).foreground(Color::Cyan));
    div.add_span(Span::new(&current_time.format("%H").to_string()).foreground(Color::Yellow));
    div.add_span(Span::new(":").foreground(Color::White).dimmed());
    div.add_span(Span::new(&current_time.format("%M").to_string()).foreground(Color::Yellow));
    div.add_span(Span::new(":").foreground(Color::White).dimmed());
    div.add_span(Span::new(&current_time.format("%S").to_string()).foreground(Color::Yellow));
    div
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
    div
}

#[cfg(test)]
mod tests {
    use super::Layout;
    use super::*;

    fn format_runs(runs: &Vec<Run>) -> Vec<String> {
        runs.iter()
            .map(|r| r.format(false))
            .collect::<Vec<String>>()
    }

    fn do_test(width: usize, dt_str: &str, left: Vec<&str>, right: Vec<&str>, result: Vec<&str>) {
        let options = LayoutOptions::new()
            .width(width)
            .use_color(false)
            .use_safe_corners(true)
            .escape_for_readline(false);
        let dt = Div::new(Span::new(dt_str));
        let l = left
            .iter()
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
        do_test(
            30,
            "TTT",
            vec!["AAAA", "BBBB", "CCCC"],
            vec!["DDDD", "EEEEEEEE"],
            vec![
                "┬──────┬──────┬──┬──────┐ TTT ",
                "├ AAAA ┴ BBBB ┤  ├ DDDD ┴───┬─",
                "├ CCCC ───────┘  └ EEEEEEEE ┘ ",
                "└➤ ",
            ],
        );
    }

    #[test]
    fn drop_left_2_1_stretch() {
        do_test(
            30,
            "TTT",
            vec!["AAAA", "BBBB", "CCCCC"],
            vec!["DDDD", "EEEEEEEE"],
            vec![
                "┬──────┬──────┬──┬──────┐ TTT ",
                "├ AAAA ┴ BBBB ┤  ├ DDDD ┴───┬─",
                "├ CCCCC ──────┘  └ EEEEEEEE ┘ ",
                "└➤ ",
            ],
        );
    }

    #[test]
    fn drop_left_2_1_shrink() {
        do_test(
            30,
            "TTT",
            vec!["AAAA", "BBBB", "CC"],
            vec!["DDDD", "EEEEEEEE"],
            vec![
                "┬──────┬──────┬──┬──────┐ TTT ",
                "├ AAAA ┴ BBBB ┤  ├ DDDD ┴───┬─",
                "├ CC ─────────┘  └ EEEEEEEE ┘ ",
                "└➤ ",
            ],
        );
    }

    #[test]
    fn drop_left_2_2() {
        do_test(
            30,
            "TTT",
            vec!["AAAA", "BBBB", "CCCC", "DDDD"],
            vec!["DDDD", "EEEEEEEE"],
            vec![
                "┬──────┬──────┬──┬──────┐ TTT ",
                "├ AAAA ┼ BBBB ┤  ├ DDDD ┴───┬─",
                "├ CCCC ┴ DDDD ┘  └ EEEEEEEE ┘ ",
                "└➤ ",
            ],
        );
    }

    #[test]
    fn drop_left_2_2_shrink() {
        do_test(
            30,
            "TTT",
            vec!["AAAA", "BBBB", "CC", "DDDD"],
            vec!["DDDD", "EEEEEEEE"],
            vec![
                "┬──────┬──────┬──┬──────┐ TTT ",
                "├ AAAA ┴ BBBB ┤  ├ DDDD ┴───┬─",
                "├ CC ─ DDDD ──┘  └ EEEEEEEE ┘ ",
                "└➤ ",
            ],
        );
    }

    #[test]
    fn drop_left_2_2_stretch() {
        do_test(
            30,
            "TTT",
            vec!["AAAA", "BBBB", "CCCCC", "DDD"],
            vec!["DDDD", "EEEEEEEE"],
            vec![
                "┬──────┬──────┬──┬──────┐ TTT ",
                "├ AAAA ┴ BBBB ┤  ├ DDDD ┴───┬─",
                "├ CCCCC ─ DDD ┘  └ EEEEEEEE ┘ ",
                "└➤ ",
            ],
        );
    }

    #[test]
    fn drop_left_2_2_stretch_more() {
        do_test(
            30,
            "TTT",
            vec!["AAAA", "BBBB", "CCCCC", "DDDDD"],
            vec!["DDDD", "EEEEEEEE"],
            vec![
                "┬──────┬────────┬┬──────┐ TTT ",
                "├ AAAA ┴ BBBB ──┤├ DDDD ┴───┬─",
                "├ CCCCC ─ DDDDD ┘└ EEEEEEEE ┘ ",
                "└➤ ",
            ],
        );
    }

    #[test]
    fn drop_left_3_2_stretch_more() {
        do_test(
            29,
            "TTT",
            vec!["AAAA", "BBBB", "CCCCC", "DDDDD"],
            vec!["DDDD", "EEEEEEEE"],
            vec![
                "┬──────┬──────┬─┬──────┐ TTT ",
                "├ AAAA ┴ BBBB ┤ ├ DDDD ┴───┬─",
                "├ CCCCC ──────┤ └ EEEEEEEE ┘ ",
                "├ DDDDD ──────┘              ",
                "└➤ ",
            ],
        );
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
