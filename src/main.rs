use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};

mod input_sim;
mod keys;
mod params;
mod server;
mod state;
mod tools;

use server::WindowsComputerUseServer;

#[tokio::main]
async fn main() -> Result<()> {
    let service = WindowsComputerUseServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
