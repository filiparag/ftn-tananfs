#![allow(dead_code)]

use error::Error;
mod error;
mod filesystem;
mod structs;


fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    Ok(())
}
