#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    lavinia_client::app::run();
}
