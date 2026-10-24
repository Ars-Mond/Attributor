#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // MCP clients start `attributor --mcp` as a stdio bridge. It branches before `run()`, where the single-instance plugin would hand the launch over to the running app and exit.
    if std::env::args_os().nth(1).is_some_and(|arg| arg == "--mcp") {
        std::process::exit(attributor_lib::run_mcp_bridge());
    }
    attributor_lib::run()
}
