use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    match motrix_fnos_server::app::run_server().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
