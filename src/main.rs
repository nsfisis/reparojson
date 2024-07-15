use reparojson::{self, RepairErr, RepairOk, RepairResult};
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{stdin, stdout, BufReader, BufWriter, Write};
use std::process::ExitCode;

struct Config {
    quiet: bool,
    file_path: Option<OsString>,
}

fn parse_args() -> std::io::Result<Config> {
    use clap::{arg, command, value_parser};

    let matches = command!()
        .arg(arg!(-q --quiet "Successfully exit if the input JSON is repaired"))
        .arg(
            arg!([FILE] "The input JSON file (default: STDIN)")
                .value_parser(value_parser!(OsString)),
        )
        .get_matches();

    let quiet = matches.get_flag("quiet");
    let file_path = matches.get_one("FILE").cloned();
    Ok(Config { quiet, file_path })
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
            if config.quiet {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(RepairErr::Invalid(err)) => {
            eprintln!("{}", err);
            ExitCode::from(2)
        }
        Err(RepairErr::IoErr(err)) => {
            eprintln!("{}", err);
            ExitCode::from(3)
        }
    };

    writer.flush()?;
    Ok(exit_code)
}
