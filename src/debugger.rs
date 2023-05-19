#![allow(dead_code)]

use error::Error;
use filesystem::Filesystem;

mod error;
mod filesystem;
mod filetypes;
mod structs;

fn prompt(separator: &str) -> Option<Vec<String>> {
    use std::io::Write;
    let mut line = String::new();
    print!("{separator}");
    std::io::stdout().flush().unwrap();
    match std::io::stdin().read_line(&mut line) {
        Ok(_) => Some(line.trim().split(' ').map(str::to_string).collect()),
        Err(_) => None,
    }
}

fn execute(cmd: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let dev = std::fs::File::options()
        .read(true)
        .write(false)
        .open(args.get(0).unwrap_or(&"/tmp/fakefs".to_owned()))?;
    let mut fs = Filesystem::load(Box::new(dev), 512)?;
    if cmd.is_empty() {
        return Ok(());
    }
    match cmd[0].as_str() {
        "s" => println!["{}", fs.superblock],
        "b" => {
            if cmd.len() == 2 {
                println!["{}", fs.load_block(cmd[1].parse()?, false)?];
            } else {
                println!["{}", fs.blocks]
            }
        }
        "i" => {
            if cmd.len() == 2 {
                println!["{}", fs.load_inode(cmd[1].parse()?)?];
            } else {
                println!["{}", fs.inodes]
            }
        }
        _ => {}
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    loop {
        let cmd = prompt(">> ").unwrap_or_default();
        if let Err(e) = execute(&cmd) {
            eprintln!("{e}");
        }
    }
}
