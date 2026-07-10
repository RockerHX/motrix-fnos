use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    motrix_fnos_server::debug_logs::init_tracing();
    match motrix_fnos_server::app::run_server().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
