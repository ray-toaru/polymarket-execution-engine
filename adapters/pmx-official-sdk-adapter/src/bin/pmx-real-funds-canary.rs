use pmx_official_sdk_adapter::real_funds_canary_cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let argv = std::env::args().skip(1).collect::<Vec<_>>();
    if argv.iter().any(|arg| arg == "--armed") {
        anyhow::bail!(
            "pmx-real-funds-canary no longer accepts --armed; use pmx-real-funds-canary-armed"
        );
    }
    if argv.iter().any(|arg| arg == "--preflight-only") {
        anyhow::bail!(
            "pmx-real-funds-canary no longer accepts --preflight-only; use pmx-real-funds-canary-preflight"
        );
    }
    let args = real_funds_canary_cli::parse_args_from(argv)?;
    real_funds_canary_cli::run(args).await
}
