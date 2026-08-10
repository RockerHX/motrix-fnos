use std::future::Future;
use std::process::ExitCode;
use std::time::Duration;

fn main() -> ExitCode {
    motrix_fnos_server::debug_logs::init_tracing();
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let result =
        run_with_runtime(motrix_fnos_server::app::run_cli(&args)).and_then(|result| result);
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run_with_runtime<F>(future: F) -> Result<F::Output, String>
where
    F: Future,
{
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("创建 Tokio 运行时失败：{error}"))?;
    let output = runtime.block_on(future);
    runtime.shutdown_timeout(Duration::ZERO);
    Ok(output)
}

#[cfg(test)]
mod tests;
