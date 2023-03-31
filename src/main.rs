#![allow(dead_code)]

use error::Error;
mod error;
mod filesystem;
mod filetypes;
mod structs;

#[allow(unknown_lints, clippy::all, unused)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    Ok(())
}
