use reparojson::{self, RepairErr, RepairOk, cli};
use std::io::{BufWriter, Write, stdout};
use std::process::ExitCode;

fn main() -> std::io::Result<ExitCode> {
    let config = cli::parse_args()?;

    let writer = stdout().lock();
    let mut writer = BufWriter::new(writer);

    if let Some(shell) = config.generate_completion {
        cli::generate_completion(shell, &mut writer);
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
