use anyhow::Context;
use polymarket_client_sdk_v2::data::Client as DataClient;
use polymarket_client_sdk_v2::data::types::MarketFilter;
use polymarket_client_sdk_v2::data::types::request::{
    ActivityRequest, ClosedPositionsRequest, PositionsRequest, TradesRequest, ValueRequest,
};
use polymarket_client_sdk_v2::types::{Address, B256, U256};
use serde::Serialize;
use std::str::FromStr;

const DATA_API_HOST: &str = "https://data-api.polymarket.com";

#[derive(Debug, Serialize)]
struct AccountActivityReport {
    query_performed: bool,
    account_ref: String,
    market_id: String,
    token_id: String,
    activity_count: usize,
    trade_count: usize,
    open_position_count: usize,
    closed_position_count: usize,
    value_record_count: usize,
    matching_activity_count: usize,
    matching_trade_count: usize,
    matching_open_position_count: usize,
    matching_closed_position_count: usize,
    matching_value_record_count: usize,
    no_matching_account_activity_observed: bool,
    activity: Vec<ActivitySummary>,
    trades: Vec<DataTradeSummary>,
    open_positions: Vec<PositionSummary>,
    closed_positions: Vec<ClosedPositionSummary>,
    values: Vec<ValueSummary>,
    error_summary: Option<String>,
    remote_side_effects: bool,
    raw_signed_order_exposed: bool,
}

#[derive(Debug, Serialize)]
struct ActivitySummary {
    activity_type: String,
    condition_id: Option<String>,
    asset: Option<String>,
    side: Option<String>,
    size: String,
    usdc_size: String,
    price: Option<String>,
    timestamp: i64,
    outcome: Option<String>,
    transaction_hash: String,
}

#[derive(Debug, Serialize)]
struct DataTradeSummary {
    condition_id: String,
    asset: String,
    side: String,
    size: String,
    price: String,
    timestamp: i64,
    outcome: String,
    transaction_hash: String,
}

#[derive(Debug, Serialize)]
struct PositionSummary {
    condition_id: String,
    asset: String,
    size: String,
    current_value: String,
    total_bought: String,
    outcome: String,
    title: String,
}

#[derive(Debug, Serialize)]
struct ClosedPositionSummary {
    condition_id: String,
    asset: String,
    total_bought: String,
    realized_pnl: String,
    outcome: String,
    timestamp: i64,
    title: String,
}

#[derive(Debug, Serialize)]
struct ValueSummary {
    user: String,
    value: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = parse_args()?;
    let account = Address::from_str(&args.account).context("invalid --account address")?;
    let market = B256::from_str(&args.market_id).context("invalid --market-id")?;
    let token = U256::from_str(&args.token_id).context("invalid --token-id")?;
    let client = DataClient::new(DATA_API_HOST).context("creating Data API client")?;
    let market_filter = MarketFilter::markets([market]);

    let report = match query_account(&client, account, market, market_filter, token).await {
        Ok(report) => report,
        Err(err) => AccountActivityReport {
            query_performed: true,
            account_ref: args.account,
            market_id: args.market_id,
            token_id: args.token_id,
            activity_count: 0,
            trade_count: 0,
            open_position_count: 0,
            closed_position_count: 0,
            value_record_count: 0,
            matching_activity_count: 0,
            matching_trade_count: 0,
            matching_open_position_count: 0,
            matching_closed_position_count: 0,
            matching_value_record_count: 0,
            no_matching_account_activity_observed: false,
            activity: Vec::new(),
            trades: Vec::new(),
            open_positions: Vec::new(),
            closed_positions: Vec::new(),
            values: Vec::new(),
            error_summary: Some(redact_error(&err.to_string())),
            remote_side_effects: false,
            raw_signed_order_exposed: false,
        },
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

async fn query_account(
    client: &DataClient,
    account: Address,
    market: B256,
    market_filter: MarketFilter,
    token: U256,
) -> anyhow::Result<AccountActivityReport> {
    let activity = client
        .activity(
            &ActivityRequest::builder()
                .user(account)
                .filter(market_filter.clone())
                .limit(100)?
                .build(),
        )
        .await
        .context("Data API activity query failed")?;
    let trades = client
        .trades(
            &TradesRequest::builder()
                .user(account)
                .filter(market_filter.clone())
                .limit(100)?
                .taker_only(false)
                .build(),
        )
        .await
        .context("Data API trades query failed")?;
    let positions = client
        .positions(
            &PositionsRequest::builder()
                .user(account)
                .filter(market_filter.clone())
                .limit(100)?
                .build(),
        )
        .await
        .context("Data API positions query failed")?;
    let closed_positions = client
        .closed_positions(
            &ClosedPositionsRequest::builder()
                .user(account)
                .filter(market_filter.clone())
                .limit(50)?
                .build(),
        )
        .await
        .context("Data API closed positions query failed")?;
    let values = client
        .value(
            &ValueRequest::builder()
                .user(account)
                .markets(vec![market])
                .build(),
        )
        .await
        .context("Data API value query failed")?;

    let matching_activity_count = activity
        .iter()
        .filter(|item| item.asset.as_ref().is_some_and(|asset| *asset == token))
        .count();
    let matching_trade_count = trades.iter().filter(|item| item.asset == token).count();
    let matching_open_position_count = positions.iter().filter(|item| item.asset == token).count();
    let matching_closed_position_count = closed_positions
        .iter()
        .filter(|item| item.asset == token)
        .count();
    let matching_value_record_count = values.len();

    Ok(AccountActivityReport {
        query_performed: true,
        account_ref: account.to_string(),
        market_id: market.to_string(),
        token_id: token.to_string(),
        activity_count: activity.len(),
        trade_count: trades.len(),
        open_position_count: positions.len(),
        closed_position_count: closed_positions.len(),
        value_record_count: values.len(),
        matching_activity_count,
        matching_trade_count,
        matching_open_position_count,
        matching_closed_position_count,
        matching_value_record_count,
        no_matching_account_activity_observed: matching_activity_count == 0
            && matching_trade_count == 0
            && matching_open_position_count == 0
            && matching_closed_position_count == 0,
        activity: activity
            .into_iter()
            .map(|item| ActivitySummary {
                activity_type: item.activity_type.to_string(),
                condition_id: item.condition_id.map(|id| id.to_string()),
                asset: item.asset.map(|asset| asset.to_string()),
                side: item.side.map(|side| side.to_string()),
                size: item.size.to_string(),
                usdc_size: item.usdc_size.to_string(),
                price: item.price.map(|price| price.to_string()),
                timestamp: item.timestamp,
                outcome: item.outcome,
                transaction_hash: item.transaction_hash.to_string(),
            })
            .collect(),
        trades: trades
            .into_iter()
            .map(|item| DataTradeSummary {
                condition_id: item.condition_id.to_string(),
                asset: item.asset.to_string(),
                side: item.side.to_string(),
                size: item.size.to_string(),
                price: item.price.to_string(),
                timestamp: item.timestamp,
                outcome: item.outcome,
                transaction_hash: item.transaction_hash.to_string(),
            })
            .collect(),
        open_positions: positions
            .into_iter()
            .map(|item| PositionSummary {
                condition_id: item.condition_id.to_string(),
                asset: item.asset.to_string(),
                size: item.size.to_string(),
                current_value: item.current_value.to_string(),
                total_bought: item.total_bought.to_string(),
                outcome: item.outcome,
                title: item.title,
            })
            .collect(),
        closed_positions: closed_positions
            .into_iter()
            .map(|item| ClosedPositionSummary {
                condition_id: item.condition_id.to_string(),
                asset: item.asset.to_string(),
                total_bought: item.total_bought.to_string(),
                realized_pnl: item.realized_pnl.to_string(),
                outcome: item.outcome,
                timestamp: item.timestamp,
                title: item.title,
            })
            .collect(),
        values: values
            .into_iter()
            .map(|item| ValueSummary {
                user: item.user.to_string(),
                value: item.value.to_string(),
            })
            .collect(),
        error_summary: None,
        remote_side_effects: false,
        raw_signed_order_exposed: false,
    })
}

#[derive(Debug)]
struct Args {
    account: String,
    market_id: String,
    token_id: String,
}

fn parse_args() -> anyhow::Result<Args> {
    let mut account = None;
    let mut market_id = None;
    let mut token_id = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--account" => {
                account = Some(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("missing value for --account"))?,
                );
            }
            "--market-id" => {
                market_id = Some(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("missing value for --market-id"))?,
                );
            }
            "--token-id" => {
                token_id = Some(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("missing value for --token-id"))?,
                );
            }
            _ => anyhow::bail!("unknown argument {arg}"),
        }
    }
    Ok(Args {
        account: account.ok_or_else(|| anyhow::anyhow!("missing required --account"))?,
        market_id: market_id.ok_or_else(|| anyhow::anyhow!("missing required --market-id"))?,
        token_id: token_id.ok_or_else(|| anyhow::anyhow!("missing required --token-id"))?,
    })
}

fn redact_error(value: &str) -> String {
    value.chars().take(240).collect()
}
