use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};

mod params;
mod server;
mod tools;

use server::WindowsComputerUseServer;

#[tokio::main]
async fn main() -> Result<()> {
    let service = WindowsComputerUseServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
