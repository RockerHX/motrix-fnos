use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    motrix_fnos_server::debug_logs::init_tracing();
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match motrix_fnos_server::app::run_cli(&args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
