#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]

use std::io::Read;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let mut sample = String::new();
    std::io::stdin().read_to_string(&mut sample)?;

    let result = compile_input(&sample).await?;
    match result {
        BuildResult::Timeout => {
            return Err(anyhow::Error::msg("timeout when building"));
        }
        BuildResult::BuildFailed { stdout, stderr } => {
            println!("{stdout}");
            eprintln!("{stderr}");
            return Ok(());
        }
        BuildResult::BuildSuccess => {}
    }

    execute_payload(WASM_FILE_NAME).await?;

    Ok(())
}

const SOURCE_FILE_NAME: &str = "main.rs";
const WASM_FILE_NAME: &str = "main.wasm";

/// The result of building a sample of code
#[derive(Debug)]
enum BuildResult {
    /// The build timed out
    Timeout,
    /// The build failed with the given output
    BuildFailed { stdout: String, stderr: String },
    /// The build succeeded
    BuildSuccess,
}

/// Compiles the sample
async fn compile_input(sample: &str) -> Result<BuildResult, anyhow::Error> {
    {
        let source_file = tokio::fs::File::create(SOURCE_FILE_NAME).await?;
        let mut writer = tokio::io::BufWriter::new(source_file);
        writer.write_all(sample.as_bytes()).await?;
        writer
            .write_all(b"\n\n#[no_mangle]\npub extern \"C\" fn __entry() { let _ = main(); }\n")
            .await?;
        writer.flush().await?;
    }

    let mut child = tokio::process::Command::new("rustc")
        .args(["--target", "wasm32-wasi", "--crate-type", "cdylib", SOURCE_FILE_NAME])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Ok(status) = tokio::time::timeout(Duration::from_secs(20), child.wait()).await {
        let status = status?;
        if status.success() {
            Ok(BuildResult::BuildSuccess)
        } else {
            let mut stdout = String::new();
            child.stdout.take().unwrap().read_to_string(&mut stdout).await?;
            let mut stderr = String::new();
            child.stderr.take().unwrap().read_to_string(&mut stderr).await?;
            Ok(BuildResult::BuildFailed { stdout, stderr })
        }
    } else {
        child.kill().await?;
        Ok(BuildResult::Timeout)
    }
}

/// Execute the job payload after building
async fn execute_payload(wasm_file: &str) -> Result<(), anyhow::Error> {
    Ok(())
}
