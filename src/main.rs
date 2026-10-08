use reparojson::{self, RepairErr, RepairOk, RepairResult};
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write, stdin, stdout};
use std::process::ExitCode;

struct Config {
    strict: bool,
    file_path: Option<OsString>,
}

fn parse_args() -> std::io::Result<Config> {
    use clap::{ArgAction, arg, command, value_parser};

    let matches = command!()
        .disable_version_flag(true)
        .arg(arg!(-s --strict "Exit with failure if the input JSON is repaired"))
        .arg(arg!(-v --version "Print version").action(ArgAction::Version))
        .arg(
            arg!([FILE] "The input JSON file (default: STDIN)")
                .value_parser(value_parser!(OsString)),
        )
        .get_matches();

    let strict = matches.get_flag("strict");
    let file_path = matches.get_one("FILE").cloned();
    Ok(Config { strict, file_path })
}

fn repair(input_file_path: Option<OsString>, mut w: impl Write) -> RepairResult {
    match input_file_path.as_ref() {
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

fn main() -> std::io::Result<ExitCode> {
    let config = parse_args()?;

    let writer = stdout().lock();
    let mut writer = BufWriter::new(writer);

    let exit_code = match repair(config.file_path, &mut writer) {
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
