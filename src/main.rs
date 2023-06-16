#![allow(dead_code)]

use filesystem::{Filesystem, FuseFs};
use log::{error, info};
use std::{
    os::unix::prelude::MetadataExt,
    sync::{Arc, Mutex},
};

use error::Error;
use fuser::MountOption;

use crate::structs::DEFAULT_BLOCK_SIZE;

mod error;
mod filesystem;
mod filetypes;
mod structs;

fn help() {
    println!("{} {}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"),);
    println!("{}", env!("CARGO_PKG_DESCRIPTION"));
    println!("{}", env!("CARGO_PKG_AUTHORS"));
    println!();
    println!("Usage:");
    println!("\ttananfs <block device> <directory> [block size]");
    println!();
    println!("Logging with RUST_LOG:");
    println!("\tnone, error (default), warn, info, debug, trace");
}

#[allow(unknown_lints, clippy::all, unused)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::panic::set_hook(Box::new(|info| {
        error!("Critical error: {info}");
    }));

    let args: Vec<String> = std::env::args().collect();

    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "error");
    }
    env_logger::init();

    let Some(blkdev_path) = args.get(1)  else {
        help();
        panic!("Block device path not provided")
    };
    let Some(mount_path) = args.get(2)  else {
        help();
        panic!("Mount point not provided")
    };

    let mut device = std::fs::File::options()
        .read(true)
        .write(true)
        .open(blkdev_path)?;

    let blkdev_size = device.metadata()?.size();

    let (block_size, existing) = match Filesystem::detect_existing(&mut device)? {
        Some(detected) => (detected, true),
        None => (
            args.get(3).map_or_else(
                || DEFAULT_BLOCK_SIZE,
                |value| value.parse().unwrap_or(DEFAULT_BLOCK_SIZE),
            ),
            false,
        ),
    };

    let mut fs = if existing {
        info!("Mounting existing filesystem {blkdev_path} to {mount_path} with block size {block_size}");
        Filesystem::load(Box::new(device), block_size)?
    } else {
        info!("Mounting new filesystem {blkdev_path} to {mount_path} with block size {block_size} and capacity {blkdev_size}");
        Filesystem::new(Box::new(device), blkdev_size, block_size)
    };

    let fs_handle = Arc::new(Mutex::new(fs));
    let fuse_fs = FuseFs {
        filesystem: fs_handle.clone(),
    };
    fuser::mount2(fuse_fs, mount_path, &[MountOption::RW])?;

    Ok(())
}
