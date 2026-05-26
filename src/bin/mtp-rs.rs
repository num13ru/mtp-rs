#[path = "../cli/mod.rs"]
mod cli;

#[tokio::main]
async fn main() -> std::process::ExitCode {
    match cli::run().await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {}", err);
            if let Some(help) = err.help() {
                eprintln!("{}", help);
            }
            std::process::ExitCode::from(err.exit_code())
        }
    }
}
