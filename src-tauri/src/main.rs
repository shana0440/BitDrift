// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod torrent;

#[tokio::main]
async fn main() {
    bitdrift_lib::run()
}
