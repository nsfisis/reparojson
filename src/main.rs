use reparojson::{self, RepairErr, RepairOk, RepairResult};
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write, stdin, stdout};
use std::process::ExitCode;

struct Config {
    strict: bool,
    in_place: bool,
    file_path: Option<OsString>,
}

fn parse_args() -> std::io::Result<Config> {
    use clap::error::ErrorKind;
    use clap::{ArgAction, arg, command, value_parser};

    let mut cmd = command!()
        .disable_version_flag(true)
        .arg(arg!(-i --"in-place" "Replace the input file in place").requires("FILE"))
        .arg(arg!(-s --strict "Exit with failure if the input JSON is repaired"))
        .arg(arg!(-v --version "Print version").action(ArgAction::Version))
        .arg(
            arg!([FILE] "The input JSON file (default: STDIN)")
                .value_parser(value_parser!(OsString)),
        );
    let matches = cmd.get_matches_mut();

    let strict = matches.get_flag("strict");
    let in_place = matches.get_flag("in-place");
    let file_path: Option<OsString> = matches.get_one("FILE").cloned();
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
    })
}

fn repair(input_file_path: Option<&OsStr>, mut w: impl Write) -> RepairResult {
    match input_file_path {
        None => {
            let reader = stdin().lock();
            let reader = BufReader::new(reader);
            reparojson::repair(reader, &mut w)
        }
        Some(file_path) => {
            if file_path == OsStr::new("-") {
                let reader = stdin().lock();
                let reader = BufReader::new(reader);
                reparojson::repair(reader, &mut w)
            } else {
                let reader = File::open(file_path)?;
                let reader = BufReader::new(reader);
                reparojson::repair(reader, &mut w)
            }
        }
    }
}

fn repair_in_place(file_path: &OsStr) -> RepairResult {
    // Buffer the whole output so that the file is left untouched on failure.
    let mut output = Vec::new();
    let result = repair(Some(file_path), &mut output)?;
    if matches!(result, RepairOk::Repaired) {
        std::fs::write(file_path, output)?;
    }
    Ok(result)
}

fn main() -> std::io::Result<ExitCode> {
    let config = parse_args()?;

    let writer = stdout().lock();
    let mut writer = BufWriter::new(writer);

    let result = match config.file_path.as_deref() {
        Some(file_path) if config.in_place => repair_in_place(file_path),
        file_path => repair(file_path, &mut writer),
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
