use std::path::{Path, PathBuf};
use tokio::process::Command;

use crate::error::HdcError;

#[derive(Debug, Clone)]
pub struct ExecRunner {
    bin: PathBuf,
    connect_key: String,
}

impl ExecRunner {
    pub fn new(bin: &Path, connect_key: &str) -> Self {
        Self {
            bin: bin.to_path_buf(),
            connect_key: connect_key.to_string(),
        }
    }

    pub async fn run(&self, args: &[String]) -> Result<String, HdcError> {
        let output = self.build_command(args).output().await?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            return Err(HdcError::SubprocessFailure {
                command: self.full_command(args).join(" "),
                code: output.status.code(),
                stdout,
                stderr,
            });
        }

        Ok(stdout)
    }

    fn build_command(&self, args: &[String]) -> Command {
        let mut command = Command::new(&self.bin);
        command.arg("-t").arg(&self.connect_key);

        for arg in args {
            command.arg(arg);
        }

        command
    }

    fn full_command(&self, args: &[String]) -> Vec<String> {
        let mut full_command = vec![
            self.bin.display().to_string(),
            "-t".to_string(),
            self.connect_key.clone(),
        ];
        full_command.extend(args.iter().cloned());
        full_command
    }
}

pub fn resolve_path(path: &Path) -> Result<PathBuf, HdcError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }

    Ok(std::env::current_dir()?.join(path))
}
