mod bots;
mod clipboard;
mod desktop;
#[allow(dead_code)]
mod dispatch;
mod guards;
mod gui;
mod inspect;
#[allow(dead_code)]
mod model;
mod place;
mod providers;
mod ranking;
mod safe_apps;
mod theme;

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("omarchy-menu-rust: {err}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), String> {
    let mut screenshot = None;
    // Spawning stays off unless the user passes --exec. --no-exec is still accepted.
    let mut saw_no_exec = false;
    let mut saw_live = false;
    let mut saw_exec = false;
    let mut saw_safe_apps = false;
    let mut exit_after = false;
    let mut list = false;
    let mut dry_run = None;
    let mut apps_dirs = Vec::new();
    let mut terminal_raw = None;
    let mut run_guards = false;
    let mut trace_state = false;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            "--exec" | "--live" | "--no-exec" | "--safe-apps" => match arg.as_str() {
                "--no-exec" => saw_no_exec = true,
                "--live" => saw_live = true,
                "--exec" => saw_exec = true,
                "--safe-apps" => saw_safe_apps = true,
                _ => {}
            },
            "--exit-after-first-frame" => exit_after = true,
            "--screenshot" => {
                let path = args.next().ok_or("--screenshot needs a path")?;
                screenshot = Some(PathBuf::from(path));
            }
            "--list" => list = true,
            "--dry-run" => {
                dry_run = Some(args.next().ok_or("--dry-run needs the query text")?);
            }
            "--apps-dir" => {
                apps_dirs.push(PathBuf::from(
                    args.next().ok_or("--apps-dir needs a directory")?,
                ));
            }
            "--terminal-argv" => {
                terminal_raw = Some(args.next().ok_or("--terminal-argv needs the argv text")?);
            }
            "--run-guards" => run_guards = true,
            "--trace-state" => trace_state = true,
            "--version" => {
                println!("omarchy-menu-rust 0.1.0");
                println!("upstream menu contract {}", model::UPSTREAM_COMMIT.trim());
                return Ok(());
            }
            other => return Err(format!("unknown argument {other}. Try --help")),
        }
    }
    if list || dry_run.is_some() {
        let terminal = match terminal_raw.as_deref() {
            Some(raw) => desktop::parse_terminal_argv(raw)?,
            None => Vec::new(),
        };
        let dirs = if apps_dirs.is_empty() {
            providers::default_app_dirs()
        } else {
            apps_dirs
        };
        let report = if let Some(query) = dry_run {
            inspect::dry_run(&query, &dirs, &terminal)
        } else {
            inspect::list_apps(&dirs, &terminal)
        };
        print!("{}", inspect::format_report(&report));
        return Ok(());
    }
    let (no_exec, live, safe_apps) =
        resolve_launch_mode(saw_no_exec, saw_safe_apps, saw_live, saw_exec)?;
    let menu = model::full_menu();
    gui::run(
        menu,
        gui::Options {
            screenshot,
            no_exec,
            live,
            safe_apps,
            exit_after_first_frame: exit_after,
            run_guards,
            trace_state,
        },
    )
}

/// `--no-exec` refuses every launch, in any order. `--safe-apps`, `--live`, and
/// `--exec` are each valid only alone. Mixing those three is an error.
pub fn resolve_launch_mode(
    saw_no_exec: bool,
    saw_safe_apps: bool,
    saw_live: bool,
    saw_exec: bool,
) -> Result<(bool, bool, bool), String> {
    if saw_no_exec {
        return Ok((true, false, false));
    }
    let chosen = u8::from(saw_safe_apps) + u8::from(saw_live) + u8::from(saw_exec);
    if chosen > 1 {
        return Err(
            "conflicting launch flags. Pass only one of --safe-apps, --live, or --exec. --no-exec refuses every launch, in any order."
                .into(),
        );
    }
    if saw_safe_apps {
        Ok((true, false, true))
    } else if saw_live {
        Ok((true, true, false))
    } else if saw_exec {
        Ok((false, false, false))
    } else {
        Ok((true, false, false))
    }
}

fn print_help() {
    println!(
        "omarchy-menu-rust\n\
         Alongside-stock scaffold. Does not change Hyprland, PATH, or the stock menu.\n\
         \n\
         Usage: omarchy-menu-rust [--no-exec] [--safe-apps] [--live] [--exec] [--run-guards] [--trace-state] [--list] [--dry-run QUERY]\n\
                [--apps-dir DIR] [--terminal-argv ARGV] [--screenshot FILE] [--exit-after-first-frame]\n\
         \n\
         --list and --dry-run only read desktop files and print a report. They do not start apps, guards, or clipboard tools, and they do not write files.\n\
         --terminal-argv is a JSON array of strings, or a quoted argv. Example: ghostty -e with a quoted path. It is not a shell command.\n\
         The default does not start commands. --no-exec refuses every launch no matter which other mode flag is also present.\n\
         --safe-apps, --live, and --exec are each valid only by themselves. Combining any two of them is an error, not a wider launch.\n\
         --safe-apps starts only a desktop file whose canonical path stays in /usr/share/applications or /usr/local/share/applications, and whose executable canonicalizes to the absolute path in src/safe_apps.rs. PATH is not searched. A copied desktop file or a same-named binary elsewhere does not match. Stock actions, scripts, updates, messaging, clipboard, and guards stay off.\n\
         None of those flags runs a guard or a clipboard tool. --run-guards is a separate opt-in.\n\
         With --run-guards, an argv guard runs only from an exact JSON string array, and a shell guard only from an exact shell line.\n\
         The literals true and false are decided in process and do not start a program.\n\
         --live confirms a desktop app or an omarchy-* program as argv. It does not run shell actions or guards.\n\
         --exec is required before a confirmed stock action or a dangerous command may run. It does not run guards.\n\
         Wayland draws a panel-sized layer surface named omarchy-menu-rust. X11 is an opaque window of the same size.\n\
         Up/Down move. A left click inside the rounded pill activates the row under it. A corner of the buffer does not.\n\
         Enter opens a menu, a provider list, or a confirm step. A held Enter does not repeat.\n\
         A second Enter starts only what the chosen flag allows. Esc goes back one level, then closes.\n\
         Search text and input text are never placed in bash -c.\n\
         Music, bots, groceries, inbox, and store rows are hidden. They are not shown as live panels.\n\
         A press on the visible dim layer dismisses the menu and is not delivered below.\n\
         --trace-state prints one stderr line per key or click: query, selected title, confirm, focus, close, refusal.\n\
         It does not start programs, open a port, or change the window. Omit it and those lines are not printed.\n\
         \n\
         Upstream contract: {}\n\
         Palette snapshot: bg #050805  fg #6dff57  accent #ff6a00",
        model::UPSTREAM_COMMIT.trim()
    );
}

#[cfg(test)]
mod tests {
    use super::resolve_launch_mode;

    fn mode(flags: &[&str]) -> Result<(bool, bool, bool), String> {
        let mut saw_no = false;
        let mut saw_safe = false;
        let mut saw_live = false;
        let mut saw_exec = false;
        for flag in flags {
            match *flag {
                "--no-exec" => saw_no = true,
                "--safe-apps" => saw_safe = true,
                "--live" => saw_live = true,
                "--exec" => saw_exec = true,
                _ => {}
            }
        }
        resolve_launch_mode(saw_no, saw_safe, saw_live, saw_exec)
    }

    #[test]
    fn no_exec_refuses_every_order_and_mixed_modes_are_rejected() {
        let orders = [
            vec!["--no-exec"],
            vec!["--safe-apps", "--no-exec"],
            vec!["--no-exec", "--safe-apps"],
            vec!["--live", "--no-exec"],
            vec!["--no-exec", "--live"],
            vec!["--exec", "--no-exec"],
            vec!["--no-exec", "--exec"],
            vec!["--safe-apps", "--live", "--no-exec"],
            vec!["--exec", "--safe-apps", "--no-exec"],
            vec!["--no-exec", "--safe-apps", "--live", "--exec"],
        ];
        for flags in orders {
            let (no_exec, live, safe_apps) =
                mode(&flags).unwrap_or_else(|err| panic!("{flags:?}: {err}"));
            assert!(no_exec && !live && !safe_apps, "{flags:?}");
        }
        assert_eq!(mode(&[]).unwrap(), (true, false, false));
        assert_eq!(mode(&["--safe-apps"]).unwrap(), (true, false, true));
        assert_eq!(mode(&["--live"]).unwrap(), (true, true, false));
        assert_eq!(mode(&["--exec"]).unwrap(), (false, false, false));
        for flags in [
            vec!["--safe-apps", "--live"],
            vec!["--live", "--safe-apps"],
            vec!["--safe-apps", "--exec"],
            vec!["--exec", "--safe-apps"],
            vec!["--live", "--exec"],
            vec!["--exec", "--live"],
            vec!["--safe-apps", "--live", "--exec"],
        ] {
            assert!(mode(&flags).is_err(), "{flags:?}");
        }
    }
}
