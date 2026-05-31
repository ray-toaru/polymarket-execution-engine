use pmx_official_sdk_adapter::real_funds_canary_cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut argv = vec![
        "--armed".to_string(),
        "--allow-live-submit-config".to_string(),
        "--allow-real-funds-canary-config".to_string(),
    ];
    argv.extend(std::env::args().skip(1));
    let args = real_funds_canary_cli::parse_args_from(argv)?;
    real_funds_canary_cli::run(args).await
}
