# promptly
Get a beautiful prompt, sooner.

### What is this?

Windows is astonishingly slow at spawning new processes. Older raspberry pis are just slow in general.
If you are used to having a complex, useful prompt on your laptops and desktops, it can take up to seconds
for the same prompt to render when you log into one of these slower platforms. Rather than accept a less
useful or uglier prompt, I decided to move my prompt into a faster language. On a fast desktop running
Linux, promptly takes ~5ms to render a prompt. On a fast desktop running windows, promptly takes about
20ms to render the same prompt. This is a marked improvement over the 0.1s and 1.5s, respectively, that
the bash equivalent takes to execute.

### Installation

1) I've not yet had enough demand to ship binary packages, so to install you'll need Rust.
Go to [rustup.rs](http://www.rustup.rs) and install Rust version 1.17 or greater by following
the easy, on-screen directions.
2) Then, in a terminal, do `cargo install promptly`. If there are red lines, please file an issue!
3) Run `promptly --status 0 --time 0 --width 80 --no-readline` to make sure it's working. If things don't
appear to be rendering properly in your terminal, adjust the command line and fonts until things
look as awesome as desired. You can run `promptly --help` to see the available rendering options.
4) Finally, install `promptly` as your prompt command. These instructions are for `bash`. For other shells,
consult your shell's documentation and please file a PR with the instructions once you get it working.
```$bash
# Add cargo's bin dir to path.
export PATH="${PATH}:${HOME}/.cargo/bin"

# Bash has a builtin timer that you can use to time process execution at second resolution.
function timer_start {
  timer=${timer:-$SECONDS}
}
function timer_stop {
  TIMER_OUT=$(($SECONDS - $timer))
  unset timer
}

# We have to wrap promptly in a sub-command to capture the status code.
function doprompt {
  STATUS_OUT=$?
  timer_stop
  export PS1=$(promptly --status ${STATUS_OUT} --width ${COLUMNS} --time ${TIMER_OUT})
}

trap 'timer_start' DEBUG
export PROMPT_COMMAND=doprompt
```
