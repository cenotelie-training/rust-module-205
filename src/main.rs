#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]

use tokio::io::AsyncWriteExt;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {

    Ok(())
}

const SOURCE_FILE_NAME: &str = "main.rs";

/// Compiles the sample
async fn compile_input(sample: &str) -> Result<(), anyhow::Error> {
    {
        let source_file = tokio::fs::File::create(SOURCE_FILE_NAME).await?;
        let mut writer = tokio::io::BufWriter::new(source_file);
        writer.write_all(sample.as_bytes()).await?;
        writer
            .write_all(b"\n\n#[no_mangle]\npub extern \"C\" fn __entry() { let _ = main(); }\n")
            .await?;
        writer.flush().await?;
    }

    Ok(())
}
