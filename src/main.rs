#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]

use std::future::{Future, IntoFuture};
use std::net::SocketAddr;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use wasmtime::{Config, Engine, Module, Store};
use wasmtime_wasi::preview1::WasiP1Ctx;
use wasmtime_wasi::{HostOutputStream, StdoutStream, StreamResult, Subscribe};

async fn route_hello() -> &'static str {
    "Hello, World!"
}

/// The outputs for the execution of a sample of code
#[derive(Serialize)]
struct ExecutionOutput {
    /// The stdout stream
    stdout: String,
    /// The stderr stream
    stderr: String,
}

async fn route_execute(input: String) -> Result<(StatusCode, Json<ExecutionOutput>), (StatusCode, String)> {
    match compile_input(&input).await {
        Err(e) => return Err((StatusCode::from_u16(500).unwrap(), e.to_string())),
        Ok(BuildResult::Timeout) => return Err((StatusCode::from_u16(500).unwrap(), String::from("timeout when building"))),
        Ok(BuildResult::BuildFailed { stdout, stderr }) => {
            return Err((StatusCode::from_u16(400).unwrap(), format!("{stdout}\n{stderr}")))
        }
        Ok(BuildResult::BuildSuccess) => {}
    }
    match execute_payload(WASM_FILE_NAME).await {
        Err(e) => Err((StatusCode::from_u16(500).unwrap(), e.to_string())),
        Ok((stdout, stderr)) => Ok((StatusCode::from_u16(200).unwrap(), Json(ExecutionOutput { stdout, stderr }))),
    }
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let app = Router::new()
        .route("/hello", get(route_hello))
        .route("/execute", post(route_execute));

    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .into_future()
        .await?;

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

/// A listener for a stream from wasm IO
#[derive(Debug, Clone)]
struct MyStream {
    /// The buffer for the stream
    buffer: Arc<Mutex<String>>,
}

impl StdoutStream for MyStream {
    fn stream(&self) -> Box<dyn HostOutputStream> {
        Box::new(self.clone())
    }

    fn isatty(&self) -> bool {
        false
    }
}

impl Subscribe for MyStream {
    fn ready<'life0, 'async_trait>(&'life0 mut self) -> Pin<Box<dyn Future<Output = ()> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(std::future::ready(()))
    }
}

impl HostOutputStream for MyStream {
    fn write(&mut self, bytes: bytes::Bytes) -> StreamResult<()> {
        let message = String::from_utf8_lossy(&bytes);
        self.buffer.lock().unwrap().push_str(&message);
        Ok(())
    }

    fn flush(&mut self) -> StreamResult<()> {
        Ok(())
    }

    fn check_write(&mut self) -> StreamResult<usize> {
        Ok(usize::MAX)
    }
}

const EXEC_FUEL_START: u64 = 100_000;
const EXEC_FUEL_YIELD: u64 = 5000;

/// Execute the job payload after building
#[allow(clippy::unused_async)]
async fn execute_payload(wasm_file: &str) -> Result<(String, String), anyhow::Error> {
    let mut config = Config::new();
    config.async_support(true);
    config.consume_fuel(true);
    let engine = Engine::new(&config)?;
    let stdout = Arc::new(Mutex::new(String::new()));
    let stderr = Arc::new(Mutex::new(String::new()));
    let context_wasi_p1 = wasmtime_wasi::WasiCtxBuilder::new()
        .stdout(MyStream { buffer: stdout.clone() })
        .stderr(MyStream { buffer: stderr.clone() })
        .build_p1();
    let host = StoreData {
        context_wasi_p1,
    };
    let mut store = Store::new(&engine, host);
    store.set_fuel(EXEC_FUEL_START)?;
    store.fuel_async_yield_interval(Some(EXEC_FUEL_YIELD))?;
    let mut linker = wasmtime::Linker::new(&engine);
    wasmtime_wasi::preview1::add_to_linker_async(&mut linker, |host: &mut StoreData| &mut host.context_wasi_p1)?;

    let module_payload = Module::from_file(&engine, wasm_file)?;
    let module_payload_instance = linker.instantiate_async(&mut store, &module_payload).await?;
    let payload_main = module_payload_instance.get_typed_func::<(), ()>(&mut store, "__entry")?;
    payload_main.call_async(&mut store, ()).await?;
    drop(store);
    Ok((
        Arc::try_unwrap(stdout).unwrap().into_inner().unwrap(),
        Arc::try_unwrap(stderr).unwrap().into_inner().unwrap(),
    ))
}
