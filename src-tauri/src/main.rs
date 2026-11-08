#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    // `auladesk --notifier`: avisos en segundo plano, sin ventana (servicio de usuario opcional).
    if std::env::args().any(|a| a == "--notifier") { auladesk_lib::notifier::run_headless(); } else { auladesk_lib::run() }
}
