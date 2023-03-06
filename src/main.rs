mod error;
mod structs;


fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    Ok(())
}
