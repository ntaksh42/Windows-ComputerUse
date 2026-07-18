use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};

mod apps;
mod fuzzy;
mod params;
mod powershell;
mod server;
mod tools;
mod win;
mod window;

use server::WindowsComputerUseServer;

#[tokio::main]
async fn main() -> Result<()> {
    let service = WindowsComputerUseServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
