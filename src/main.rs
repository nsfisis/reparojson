use clap::Command;
use clap_complete::Shell;
use reparojson::{self, RepairErr, RepairOk};
use std::ffi::{OsStr, OsString};
use std::io::{BufWriter, Write, stdout};
use std::process::ExitCode;

struct Config {
    strict: bool,
    in_place: bool,
    file_path: Option<OsString>,
    generate_completion: Option<Shell>,
}

fn build_command() -> Command {
    use clap::{ArgAction, ValueHint, arg, command, value_parser};

    command!()
        .disable_version_flag(true)
        .arg(arg!(-i --"in-place" "Replace the input file in place").requires("FILE"))
        .arg(arg!(-s --strict "Exit with failure if the input JSON is repaired"))
        .arg(
            arg!(--"generate-completion" <SHELL> "Generate the completion script for the given shell")
                .value_parser(value_parser!(Shell))
                .exclusive(true),
        )
        .arg(arg!(-v --version "Print version").action(ArgAction::Version))
        .arg(
            arg!([FILE] "The input JSON file (default: STDIN)")
                .value_parser(value_parser!(OsString))
                .value_hint(ValueHint::FilePath),
        )
}

fn parse_args() -> std::io::Result<Config> {
    use clap::error::ErrorKind;

    let mut cmd = build_command();
    let matches = cmd.get_matches_mut();

    let strict = matches.get_flag("strict");
    let in_place = matches.get_flag("in-place");
    let file_path: Option<OsString> = matches.get_one("FILE").cloned();
    let generate_completion: Option<Shell> = matches.get_one("generate-completion").copied();
    if in_place && file_path.as_deref() == Some(OsStr::new("-")) {
        cmd.error(
            ErrorKind::ArgumentConflict,
            "the argument '--in-place' cannot be used with STDIN",
        )
        .exit();
    }
    Ok(Config {
        strict,
        in_place,
        file_path,
        generate_completion,
    })
}

fn main() -> std::io::Result<ExitCode> {
    let config = parse_args()?;

    let writer = stdout().lock();
    let mut writer = BufWriter::new(writer);

    if let Some(shell) = config.generate_completion {
        let mut cmd = build_command();
        let bin_name = cmd.get_name().to_string();
        clap_complete::generate(shell, &mut cmd, bin_name, &mut writer);
        writer.flush()?;
        return Ok(ExitCode::SUCCESS);
    }

    let result = match config.file_path.as_deref() {
        Some(file_path) if config.in_place => reparojson::repair_file_in_place(file_path),
        file_path => reparojson::repair_file(file_path, &mut writer),
    };

    let exit_code = match result {
        Ok(RepairOk::Valid) => ExitCode::SUCCESS,
        Ok(RepairOk::Repaired) => {
            if config.strict {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(RepairErr::Invalid(err)) => {
            eprintln!("{}", err);
            ExitCode::FAILURE
        }
        Err(RepairErr::IoErr(err)) => {
            eprintln!("{}", err);
            ExitCode::FAILURE
        }
    };

    writer.flush()?;
    Ok(exit_code)
}
