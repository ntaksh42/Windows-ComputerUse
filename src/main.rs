use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};

mod apps;
mod capture;
mod display;
mod fuzzy;
mod ia2;
mod input_sim;
mod keys;
mod params;
mod powershell;
mod server;
mod state;
mod tool_policy;
mod tools;
mod uia;
mod vdm;
mod win;
mod window;

use server::WindowsComputerUseServer;

#[tokio::main]
async fn main() -> Result<()> {
    let service = WindowsComputerUseServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
