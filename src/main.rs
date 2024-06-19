#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]

use std::io::Read;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use wasmtime::{Config, Engine, Module, Store};
use wasmtime_wasi::preview1::WasiP1Ctx;

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

/// Internal data for a WASM store in order to execute WASM code
struct StoreData {
    /// The associated preview 1 WASI context
    context_wasi_p1: WasiP1Ctx,
}

/// Execute the job payload after building
#[allow(clippy::unused_async)]
async fn execute_payload(wasm_file: &str) -> Result<(), anyhow::Error> {
    let mut config = Config::new();
    config.async_support(true);
    let engine = Engine::new(&config)?;
    let context_wasi_p1 = wasmtime_wasi::WasiCtxBuilder::new().build_p1();
    let host = StoreData {
        context_wasi_p1,
    };
    let mut store = Store::new(&engine, host);
    let mut linker = wasmtime::Linker::new(&engine);
    wasmtime_wasi::preview1::add_to_linker_async(&mut linker, |host: &mut StoreData| &mut host.context_wasi_p1)?;

    let module_payload = Module::from_file(&engine, wasm_file)?;
    let module_payload_instance = linker.instantiate_async(&mut store, &module_payload).await?;
    let payload_main = module_payload_instance.get_typed_func::<(), ()>(&mut store, "__entry")?;
    payload_main.call_async(&mut store, ()).await?;
    Ok(())
}
