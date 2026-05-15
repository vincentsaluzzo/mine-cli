use std::io::{self, Write};
use std::process::ExitCode;

const HELP: &str = "\
minecli - manage Minecraft server mods, datapacks, and plugins

Usage:
  minecli --version
  minecli --help

This is the initial project scaffold. Functional server management commands
will be added phase by phase.
";

fn main() -> ExitCode {
    let mut stdout = io::stdout();
    let mut stderr = io::stderr();
    match run(std::env::args().skip(1), &mut stdout, &mut stderr) {
        Ok(()) => ExitCode::SUCCESS,
        Err(()) => ExitCode::FAILURE,
    }
}

fn run<I, S, W, E>(args: I, stdout: &mut W, stderr: &mut E) -> Result<(), ()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
    E: Write,
{
    let args = args
        .into_iter()
        .map(|arg| arg.as_ref().to_owned())
        .collect::<Vec<_>>();

    match args.as_slice() {
        [] => {
            writeln!(stdout, "{HELP}").map_err(|_| ())?;
            Ok(())
        }
        [arg] if arg == "--help" || arg == "-h" => {
            writeln!(stdout, "{HELP}").map_err(|_| ())?;
            Ok(())
        }
        [arg] if arg == "--version" || arg == "-V" => {
            writeln!(stdout, "minecli {}", env!("CARGO_PKG_VERSION")).map_err(|_| ())?;
            Ok(())
        }
        [command, ..] => {
            writeln!(stderr, "unknown command or flag: {command}").map_err(|_| ())?;
            writeln!(stderr, "run `minecli --help` for usage").map_err(|_| ())?;
            Err(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn prints_version() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let result = run(["--version"], &mut stdout, &mut stderr);

        assert!(result.is_ok());
        assert_eq!(
            String::from_utf8(stdout).unwrap(),
            format!("minecli {}\n", env!("CARGO_PKG_VERSION"))
        );
        assert!(stderr.is_empty());
    }

    #[test]
    fn prints_help_without_arguments() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let result = run(std::iter::empty::<&str>(), &mut stdout, &mut stderr);

        assert!(result.is_ok());
        assert!(String::from_utf8(stdout).unwrap().contains("Usage:"));
        assert!(stderr.is_empty());
    }

    #[test]
    fn rejects_unknown_commands() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let result = run(["install"], &mut stdout, &mut stderr);

        assert!(result.is_err());
        assert!(stdout.is_empty());
        assert!(
            String::from_utf8(stderr)
                .unwrap()
                .contains("unknown command")
        );
    }
}
