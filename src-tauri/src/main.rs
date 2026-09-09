// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    taurhaus_lib::platform::terminal_io::maybe_run_child();
    taurhaus_lib::run();
}
