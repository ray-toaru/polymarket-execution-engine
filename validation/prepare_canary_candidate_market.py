#!/usr/bin/env python3
"""Prepare a reviewed canary market candidate from public read-only APIs.

This is release-prep tooling for the integration repository. It must not sign,
submit, cancel, or hold trading secrets. The output shape matches the execution
engine RealFundsCanaryMarketCandidate. The execution engine remains responsible
for validating the resulting candidate immediately before any dry run or
controlled canary attempt.
"""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from decimal import Decimal, InvalidOperation, ROUND_CEILING, ROUND_FLOOR
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
INTEGRATION_ROOT = ROOT.parent
DEFAULT_GAMMA_URL = "https://gamma-api.polymarket.com"
DEFAULT_CLOB_URL = "https://clob.polymarket.com"
VERSION = (INTEGRATION_ROOT / "VERSION").read_text().strip()
USER_AGENT = f"pmx-canary-candidate-prep/{VERSION}"
MAX_PRICE = Decimal("1")
FETCH_RETRY_ATTEMPTS = 3
SENSITIVE_ERROR_ASSIGNMENT_RE = re.compile(
    r"\b(api[_-]?secret|api[_-]?key|private[_-]?key|signed[_-]?payload|signature|secret)=([^\s&]+)",
    re.IGNORECASE,
)


@dataclass(frozen=True)
class Candidate:
    market_id: str
    token_id: str
    outcome: str
    market_slug: str
    market_end_at: str
    active: bool
    accepting_orders: bool
    closed: bool
    archived: bool
    best_ask: Decimal
    limit_price: Decimal
    ask_size: Decimal
    target_size: Decimal
    spread_bps: int
    min_order_size: Decimal
    min_tick_size: Decimal
    liquidity_score: int
    source_market_hash: str
    book_snapshot_timestamp: str
    human_review_ref: str
    exchange_rule_evidence_ref: str
    exchange_rule_valid_for_minutes: int

    def to_engine_json(self) -> dict[str, Any]:
        return {
            "market_id": self.market_id,
            "token_id": self.token_id,
            "side": "BUY",
            "order_type": "GTC",
            "post_only": True,
            "active": self.active,
            "accepting_orders": self.accepting_orders,
            "closed": self.closed,
            "archived": self.archived,
            "best_ask": decimal_text(self.best_ask),
            "limit_price": decimal_text(self.limit_price),
            "ask_size": decimal_text(self.ask_size),
            "target_size": decimal_text(self.target_size),
            "estimated_order_notional_usd": decimal_text(self.limit_price * self.target_size),
            "spread_bps": self.spread_bps,
            "min_order_size": decimal_text(self.min_order_size),
            "exchange_rule_snapshot": {
                "schema_version": 1,
                "venue": "polymarket_clob",
                "order_mode": "post_only_limit",
                "order_type": "GTC",
                "side": "BUY",
                "target_size_semantics": "outcome_shares",
                "min_share_size": decimal_text(self.min_order_size),
                "min_tick_size": decimal_text(self.min_tick_size),
                "source": "public_clob_book_plus_reviewed_remote_rule",
                "captured_at": self.book_snapshot_timestamp,
                "expires_at": (
                    dt.datetime.fromisoformat(self.book_snapshot_timestamp)
                    + dt.timedelta(minutes=self.exchange_rule_valid_for_minutes)
                ).isoformat(),
                "evidence_ref": self.exchange_rule_evidence_ref,
            },
            "liquidity_score": self.liquidity_score,
            "book_snapshot_timestamp": self.book_snapshot_timestamp,
            "human_review_ref": self.human_review_ref,
        }


class CandidateError(RuntimeError):
    def __init__(self, message: str, audit: dict[str, Any] | None = None) -> None:
        super().__init__(message)
        self.audit = audit


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Prepare candidate-market.json for a controlled canary review from "
            "public read-only Polymarket APIs."
        )
    )
    parser.add_argument("--output", required=True, type=Path, help="Path for candidate-market.json")
    parser.add_argument("--audit-output", type=Path, help="Optional audit sidecar JSON path")
    parser.add_argument(
        "--human-review-ref",
        required=True,
        help="External operator review/ticket reference approving this candidate market for BUY/GTC post-only canary only.",
    )
    parser.add_argument(
        "--exchange-rule-evidence-ref",
        required=True,
        help="External evidence reference for the reviewed exchange rule snapshot bound into exchange_rule_snapshot.evidence_ref.",
    )
    parser.add_argument("--gamma-url", default=DEFAULT_GAMMA_URL, help="Gamma API base URL")
    parser.add_argument("--clob-url", default=DEFAULT_CLOB_URL, help="CLOB API base URL")
    parser.add_argument(
        "--market-url",
        help="Optional Polymarket event/market URL to use instead of scanning high-volume markets.",
    )
    parser.add_argument(
        "--market-slug",
        help="Optional Gamma market/event slug to use instead of scanning high-volume markets.",
    )
    parser.add_argument(
        "--outcome",
        help="Outcome label for --market-url/--market-slug selection, for example Yes or No.",
    )
    parser.add_argument("--max-markets", type=int, default=200, help="Maximum Gamma markets to inspect")
    parser.add_argument(
        "--target-size",
        help=(
            "Optional canary BUY size in outcome shares. When omitted, the helper "
            "uses the CLOB book min_order_size for the selected market."
        ),
    )
    parser.add_argument(
        "--max-order-notional-usd",
        default="1.00",
        help="Controlled canary order cap; selected target-size notional must not exceed it",
    )
    parser.add_argument("--max-spread-bps", type=int, default=100, help="Maximum allowed spread in bps")
    parser.add_argument(
        "--min-end-days",
        type=int,
        help="Require the Gamma market endDate to be at least this many days after the snapshot time.",
    )
    parser.add_argument(
        "--exchange-rule-valid-for-minutes",
        type=int,
        default=5,
        help="Validity window for exchange_rule_snapshot.expires_at relative to captured_at",
    )
    parser.add_argument(
        "--max-clob-requests",
        type=int,
        help="Optional total CLOB request budget for scan mode; defaults to max_markets * 2",
    )
    parser.add_argument("--timeout-seconds", type=float, default=10.0, help="HTTP timeout per request")
    return parser.parse_args()


def fetch_json(base_url: str, path: str, query: dict[str, str], timeout_seconds: float) -> Any:
    url = base_url.rstrip("/") + path
    if query:
        url += "?" + urllib.parse.urlencode(query)
    request = urllib.request.Request(
        url,
        headers={"Accept": "application/json", "User-Agent": USER_AGENT},
        method="GET",
    )
    last_error: Exception | None = None
    for attempt in range(FETCH_RETRY_ATTEMPTS):
        try:
            with urllib.request.urlopen(request, timeout=timeout_seconds) as response:
                content_type = response.headers.get("Content-Type", "")
                if not is_json_content_type(content_type):
                    raise ValueError(f"unexpected content type {content_type or '<missing>'}")
                return json.loads(response.read().decode("utf-8"))
        except urllib.error.HTTPError as exc:
            last_error = exc
            if attempt == FETCH_RETRY_ATTEMPTS - 1 or not should_retry_http_error(exc):
                break
            time.sleep(retry_delay_seconds(attempt, exc.headers.get("Retry-After") if exc.headers else None))
        except (urllib.error.URLError, TimeoutError, OSError, json.JSONDecodeError) as exc:
            last_error = exc
            if attempt == FETCH_RETRY_ATTEMPTS - 1:
                break
            time.sleep(0.2 * (attempt + 1))
    assert last_error is not None
    raise last_error


def is_json_content_type(content_type: str) -> bool:
    media_type = content_type.split(";", 1)[0].strip().lower()
    return media_type == "application/json" or media_type.endswith("+json")


def should_retry_http_error(exc: urllib.error.HTTPError) -> bool:
    return exc.code == 429 or 500 <= exc.code < 600


def retry_delay_seconds(attempt: int, retry_after: str | None) -> float:
    if retry_after is not None:
        try:
            delay = float(retry_after.strip())
        except ValueError:
            delay = 0.0
        if delay > 0:
            return min(delay, 5.0)
    return 0.2 * (attempt + 1)


def audit_url_ref(url: str) -> dict[str, str]:
    parsed = urllib.parse.urlparse(url.strip())
    origin = f"{parsed.scheme}://{parsed.netloc}" if parsed.scheme and parsed.netloc else ""
    return {
        "origin": origin,
        "sha256": hashlib.sha256(url.encode("utf-8")).hexdigest(),
    }


def redacted_error_summary(message: str) -> str:
    return SENSITIVE_ERROR_ASSIGNMENT_RE.sub(lambda match: f"{match.group(1)}=<redacted>", message)


def as_bool(value: Any) -> bool:
    if isinstance(value, bool):
        return value
    if isinstance(value, str):
        return value.lower() in {"1", "true", "yes"}
    return bool(value)


def as_decimal(value: Any) -> Decimal | None:
    if value is None:
        return None
    try:
        decimal = Decimal(str(value))
    except (InvalidOperation, ValueError):
        return None
    return decimal if decimal.is_finite() else None


def decimal_text(value: Decimal) -> str:
    normalized = value.normalize()
    text = format(normalized, "f")
    return "0" if text == "-0" else text


def parse_token_ids(market: dict[str, Any]) -> list[str]:
    for key in ("clobTokenIds", "clob_token_ids", "clobTokenIDs"):
        parsed = parse_jsonish_list(market.get(key))
        if parsed:
            return parsed
    tokens = market.get("tokens")
    if isinstance(tokens, list):
        ids = []
        for token in tokens:
            if isinstance(token, dict):
                token_id = token.get("token_id") or token.get("tokenId") or token.get("id")
                if token_id:
                    ids.append(str(token_id))
        return ids
    return []


def parse_outcomes(market: dict[str, Any]) -> list[str]:
    for key in ("outcomes", "outcomeLabels", "outcome_labels"):
        parsed = parse_jsonish_list(market.get(key))
        if parsed:
            return parsed
    tokens = market.get("tokens")
    if isinstance(tokens, list):
        outcomes = []
        for token in tokens:
            if isinstance(token, dict):
                outcome = token.get("outcome") or token.get("name")
                if outcome:
                    outcomes.append(str(outcome))
        return outcomes
    return []


def parse_jsonish_list(raw: Any) -> list[str]:
    if isinstance(raw, str):
        try:
            parsed = json.loads(raw)
        except json.JSONDecodeError:
            parsed = [part.strip() for part in raw.split(",")]
        if isinstance(parsed, list):
            return [str(item) for item in parsed if str(item).strip()]
    if isinstance(raw, list):
        return [str(item) for item in raw if str(item).strip()]
    return []


def market_id(market: dict[str, Any]) -> str:
    value = market.get("conditionId") or market.get("condition_id") or market.get("id")
    return "" if value is None else str(value)


def parse_market_end_at(market: dict[str, Any]) -> dt.datetime | None:
    for key in ("endDate", "endDateIso", "end_date", "end_date_iso", "gameStartTime"):
        raw = market.get(key)
        if raw is None:
            continue
        text = str(raw).strip()
        if not text:
            continue
        if text.endswith("Z"):
            text = text[:-1] + "+00:00"
        if len(text) == 10:
            text += "T00:00:00+00:00"
        try:
            parsed = dt.datetime.fromisoformat(text)
        except ValueError:
            continue
        if parsed.tzinfo is None:
            parsed = parsed.replace(tzinfo=dt.UTC)
        return parsed.astimezone(dt.UTC)
    return None


def best_ask(book: dict[str, Any]) -> tuple[Decimal, Decimal] | None:
    asks = book.get("asks")
    if not isinstance(asks, list):
        return None
    parsed: list[tuple[Decimal, Decimal]] = []
    for ask in asks:
        if not isinstance(ask, dict):
            continue
        price = as_decimal(ask.get("price"))
        size = as_decimal(ask.get("size"))
        if price is not None and size is not None and price > 0 and size > 0:
            parsed.append((price, size))
    if not parsed:
        return None
    return min(parsed, key=lambda item: item[0])


def best_bid(book: dict[str, Any]) -> tuple[Decimal, Decimal] | None:
    bids = book.get("bids")
    if not isinstance(bids, list):
        return None
    parsed: list[tuple[Decimal, Decimal]] = []
    for bid in bids:
        if not isinstance(bid, dict):
            continue
        price = as_decimal(bid.get("price"))
        size = as_decimal(bid.get("size"))
        if price is not None and size is not None and price > 0 and size > 0:
            parsed.append((price, size))
    if not parsed:
        return None
    return max(parsed, key=lambda item: item[0])


def post_only_buy_limit_price(
    best_ask_price: Decimal,
    min_tick_size: Decimal,
    *,
    best_bid_price: Decimal | None = None,
) -> Decimal | None:
    if best_ask_price <= 0 or min_tick_size <= 0:
        return None
    ask_ticks = (best_ask_price / min_tick_size).to_integral_value(rounding=ROUND_FLOOR)
    upper = ask_ticks * min_tick_size
    if upper >= best_ask_price:
        upper -= min_tick_size
    if upper <= 0:
        return None
    if best_bid_price is None or best_bid_price <= 0:
        return upper
    improved_bid = ((best_bid_price / min_tick_size).to_integral_value(rounding=ROUND_FLOOR) + 1) * min_tick_size
    if improved_bid < best_ask_price and improved_bid <= upper:
        return improved_bid
    bid_grid = (best_bid_price / min_tick_size).to_integral_value(rounding=ROUND_FLOOR) * min_tick_size
    if bid_grid > 0 and bid_grid < best_ask_price and bid_grid <= upper:
        return bid_grid
    return upper


def spread_bps(book: dict[str, Any], spread: dict[str, Any]) -> int | None:
    raw = spread.get("spread")
    api_value = as_decimal(raw)
    api_bps: int | None = None
    if api_value is not None and api_value >= 0:
        api_bps = int((api_value * Decimal("10000")).to_integral_value(rounding=ROUND_CEILING))

    ask = best_ask(book)
    bid = best_bid(book)
    book_bps: int | None = None
    if ask is not None and bid is not None:
        ask_price, _ = ask
        bid_price, _ = bid
        if ask_price > 0 and bid_price >= 0 and ask_price >= bid_price:
            width = ask_price - bid_price
            book_bps = int(((width / ask_price) * Decimal("10000")).to_integral_value(rounding=ROUND_CEILING))

    candidates = [value for value in (api_bps, book_bps) if value is not None]
    if not candidates:
        return None
    return max(candidates)


def liquidity_score(ask_size: Decimal, best_ask_price: Decimal, spread_bps_value: int) -> int:
    if ask_size <= 0 or best_ask_price <= 0:
        return 0
    notional = ask_size * best_ask_price
    depth_score = int((notional * Decimal("1000000")).to_integral_value(rounding=ROUND_FLOOR))
    spread_penalty = max(spread_bps_value, 0) * 1000
    return max(0, depth_score - spread_penalty)


def market_fingerprint(
    market: dict[str, Any],
    *,
    book: dict[str, Any] | None = None,
    spread: dict[str, Any] | None = None,
) -> str:
    payload: dict[str, Any] = {"market": market}
    if book is not None:
        payload["book"] = book
    if spread is not None:
        payload["spread"] = spread
    compact = json.dumps(payload, sort_keys=True, separators=(",", ":"), default=str)
    return hashlib.sha256(compact.encode("utf-8")).hexdigest()


def slug_from_market_url(url: str) -> str:
    parsed = urllib.parse.urlparse(url.strip())
    parts = [part for part in parsed.path.split("/") if part]
    for marker in ("event", "markets"):
        if marker in parts:
            index = parts.index(marker)
            if index + 1 < len(parts):
                return parts[index + 1]
    if parts:
        return parts[-1]
    raise CandidateError("--market-url does not contain a Polymarket slug")


def normalized_outcome(value: str) -> str:
    return value.strip().casefold()


def candidate_sort_key(candidate: Candidate) -> tuple[int, int, str, str]:
    return (-candidate.liquidity_score, candidate.spread_bps, candidate.market_id, candidate.token_id)


def market_disambiguation_summary(market: dict[str, Any]) -> dict[str, Any]:
    return {
        "market_id": market_id(market),
        "slug": str(market.get("slug") or ""),
        "outcomes": parse_outcomes(market),
        "token_ids": parse_token_ids(market),
    }


def token_id_tail(token_id: str) -> str:
    return token_id[-6:] if len(token_id) > 6 else token_id


def selected_candidate_summary(candidate: Candidate, *, target_size_source: str) -> dict[str, Any]:
    return {
        "market_id": candidate.market_id,
        "market_slug": candidate.market_slug,
        "outcome": candidate.outcome,
        "token_id_hash": hashlib.sha256(candidate.token_id.encode()).hexdigest(),
        "token_id_tail": token_id_tail(candidate.token_id),
        "book_snapshot_timestamp": candidate.book_snapshot_timestamp,
        "market_end_at": candidate.market_end_at,
        "source_market_hash": candidate.source_market_hash,
        "candidate_output_fields": [
            "market_id",
            "token_id",
            "side",
            "order_type",
            "post_only",
            "limit_price",
            "target_size",
            "book_snapshot_timestamp",
            "human_review_ref",
        ],
        "orderability_evidence": {
            "active": candidate.active,
            "accepting_orders": candidate.accepting_orders,
            "closed": candidate.closed,
            "archived": candidate.archived,
            "top_ask_available": candidate.best_ask > 0 and candidate.ask_size > 0,
            "post_only_limit_price_available": candidate.limit_price > 0,
            "spread_bps": candidate.spread_bps,
            "min_order_size": decimal_text(candidate.min_order_size),
            "min_tick_size": decimal_text(candidate.min_tick_size),
        },
        "price_size_summary": {
            "best_ask": decimal_text(candidate.best_ask),
            "ask_size": decimal_text(candidate.ask_size),
            "limit_price": decimal_text(candidate.limit_price),
            "target_size": decimal_text(candidate.target_size),
            "target_size_source": target_size_source,
            "estimated_order_notional_usd": decimal_text(candidate.limit_price * candidate.target_size),
        },
    }


def is_concrete_external_ref(value: str) -> bool:
    text = value.strip()
    if not text or "REPLACE_WITH" in text:
        return False
    parsed = urllib.parse.urlparse(text)
    if not parsed.scheme:
        return False
    return bool(parsed.netloc or parsed.path)


def fetch_json_or_error(
    *,
    base_url: str,
    path: str,
    query: dict[str, str],
    timeout_seconds: float,
    failure_message: str,
    audit: dict[str, Any],
) -> Any:
    try:
        return fetch_json(base_url, path, query, timeout_seconds)
    except (urllib.error.URLError, urllib.error.HTTPError, TimeoutError, OSError, json.JSONDecodeError, ValueError) as exc:
        audit.setdefault("fetch_errors", []).append(
            {
                "path": path,
                "query": query,
                "error": redacted_error_summary(f"{type(exc).__name__}: {exc}"),
            }
        )
        raise CandidateError(failure_message, audit) from exc


def clob_request_limit(args: argparse.Namespace) -> int:
    configured = args.max_clob_requests
    if configured is None:
        return args.max_markets * 2
    return configured


def select_market_for_outcome(
    markets: list[dict[str, Any]],
    *,
    requested_outcome: str,
    slug: str,
) -> dict[str, Any]:
    wanted = normalized_outcome(requested_outcome)
    matches = []
    for market in markets:
        if not isinstance(market, dict):
            continue
        outcomes = parse_outcomes(market)
        if any(normalized_outcome(outcome) == wanted for outcome in outcomes):
            matches.append(market)
    if not matches:
        raise CandidateError(
            f"Gamma API did not return any market for slug {slug!r} with outcome {requested_outcome!r}"
        )
    exact_slug_matches = [market for market in matches if str(market.get("slug") or "") == slug]
    if len(exact_slug_matches) == 1:
        return exact_slug_matches[0]
    if len(matches) == 1:
        return matches[0]
    summaries = [market_disambiguation_summary(market) for market in matches]
    raise CandidateError(
        "Gamma API returned multiple markets for slug "
        f"{slug!r} and outcome {requested_outcome!r}; explicit market disambiguation is required. "
        f"Candidates: {json.dumps(summaries, ensure_ascii=True, sort_keys=True)}"
    )


def candidate_from_market(
    args: argparse.Namespace,
    market: dict[str, Any],
    *,
    requested_outcome: str,
    order_cap: Decimal,
    requested_target_size: Decimal | None,
    snapshot_at: str,
    human_review_ref: str,
    exchange_rule_evidence_ref: str,
    audit: dict[str, Any],
) -> Candidate:
    snapshot_dt = dt.datetime.fromisoformat(snapshot_at)
    min_end_days = getattr(args, "min_end_days", None)
    min_end_at = snapshot_dt + dt.timedelta(days=min_end_days) if min_end_days is not None else None
    market_end_at = parse_market_end_at(market)
    if min_end_at is not None:
        if market_end_at is None:
            raise CandidateError("selected market is missing endDate required by --min-end-days", audit)
        if market_end_at <= min_end_at:
            raise CandidateError(
                f"selected market endDate {market_end_at.isoformat()} is not after required minimum {min_end_at.isoformat()}",
                audit,
            )
    active = as_bool(market.get("active"))
    accepting_orders = as_bool(
        market.get("acceptingOrders", market.get("accepting_orders", market.get("enableOrderBook")))
    )
    closed = as_bool(market.get("closed"))
    archived = as_bool(market.get("archived"))
    if not active:
        raise CandidateError("selected market is not active", audit)
    if not accepting_orders:
        raise CandidateError("selected market is not accepting orders", audit)
    if closed:
        raise CandidateError("selected market is closed", audit)
    if archived:
        raise CandidateError("selected market is archived", audit)
    condition_id = market_id(market)
    if not condition_id:
        raise CandidateError("selected market is missing condition id", audit)
    token_ids = parse_token_ids(market)
    outcomes = parse_outcomes(market)
    if not token_ids:
        raise CandidateError("selected market is missing CLOB token ids", audit)
    if not outcomes:
        raise CandidateError("selected market is missing outcome labels", audit)
    if len(token_ids) != len(outcomes):
        raise CandidateError("selected market outcome/token count mismatch", audit)

    wanted = normalized_outcome(requested_outcome)
    matched = [
        (outcome, token_id)
        for outcome, token_id in zip(outcomes, token_ids, strict=True)
        if normalized_outcome(outcome) == wanted
    ]
    if not matched:
        raise CandidateError(
            f"requested outcome {requested_outcome!r} not found in selected market outcomes",
            audit,
        )
    outcome, token_id = matched[0]
    book = fetch_json_or_error(
        base_url=args.clob_url,
        path="/book",
        query={"token_id": token_id},
        timeout_seconds=args.timeout_seconds,
        failure_message="selected market book request failed",
        audit=audit,
    )
    if not isinstance(book, dict):
        raise CandidateError("selected market book response was not an object", audit)
    top_ask = best_ask(book)
    top_bid = best_bid(book)
    if top_ask is None:
        raise CandidateError("selected market has no usable top ask", audit)
    price, size = top_ask
    spread = fetch_json_or_error(
        base_url=args.clob_url,
        path="/spread",
        query={"token_id": token_id},
        timeout_seconds=args.timeout_seconds,
        failure_message="selected market spread request failed",
        audit=audit,
    )
    if not isinstance(spread, dict):
        raise CandidateError("selected market spread response was not an object", audit)
    bps = spread_bps(book, spread)
    if bps is None:
        raise CandidateError("selected market spread is unavailable", audit)
    if bps > args.max_spread_bps:
        raise CandidateError(
            f"selected market spread {bps} bps exceeds max {args.max_spread_bps} bps",
            audit,
        )
    min_order_size = as_decimal(book.get("min_order_size"))
    if min_order_size is None or min_order_size <= 0:
        raise CandidateError("selected market min_order_size is unavailable", audit)
    min_tick_size = as_decimal(book.get("min_tick_size", book.get("tick_size")))
    if min_tick_size is None or min_tick_size <= 0:
        raise CandidateError("selected market min_tick_size is unavailable", audit)
    limit_price = post_only_buy_limit_price(
        price,
        min_tick_size,
        best_bid_price=top_bid[0] if top_bid is not None else None,
    )
    if limit_price is None:
        raise CandidateError("selected market best ask/min tick cannot produce a non-crossing post-only price", audit)
    target_size = requested_target_size or min_order_size
    if target_size <= 0:
        raise CandidateError("selected market min_order_size is unavailable for automatic target size", audit)
    if size < target_size:
        raise CandidateError("selected market top ask size is below target size", audit)
    if min_order_size > target_size:
        raise CandidateError("selected market min order size is above target size", audit)
    estimated_notional = limit_price * target_size
    if estimated_notional > order_cap:
        raise CandidateError("selected market target-size notional is above canary order cap", audit)
    slug = str(market.get("slug") or args.market_slug or "")
    market_end_at_text = market_end_at.isoformat() if market_end_at is not None else ""
    audit["selected"] = {
        "market_id": condition_id,
        "token_id_hash": hashlib.sha256(token_id.encode()).hexdigest(),
        "outcome": outcome,
        "market_slug": slug,
        "market_end_at": market_end_at_text,
        "source_market_hash": market_fingerprint(market, book=book, spread=spread),
        "spread_bps": bps,
        "target_size": decimal_text(target_size),
        "target_size_source": "operator_override" if requested_target_size else "book_min_order_size",
        "estimated_order_notional_usd": decimal_text(estimated_notional),
        "limit_price": decimal_text(limit_price),
        "top_ask_notional_usd": decimal_text(price * size),
        "liquidity_score": liquidity_score(size, price, bps),
    }
    candidate = Candidate(
        market_id=condition_id,
        token_id=token_id,
        outcome=outcome,
        market_slug=slug,
        market_end_at=market_end_at_text,
        active=True,
        accepting_orders=True,
        closed=False,
        archived=False,
        best_ask=price,
        limit_price=limit_price,
        ask_size=size,
        target_size=target_size,
        spread_bps=bps,
        min_order_size=min_order_size,
        min_tick_size=min_tick_size,
        liquidity_score=liquidity_score(size, price, bps),
        source_market_hash=market_fingerprint(market, book=book, spread=spread),
        book_snapshot_timestamp=snapshot_at,
        human_review_ref=human_review_ref,
        exchange_rule_evidence_ref=exchange_rule_evidence_ref,
        exchange_rule_valid_for_minutes=args.exchange_rule_valid_for_minutes,
    )
    audit["selected_candidate_summary"] = selected_candidate_summary(
        candidate,
        target_size_source="operator_override" if requested_target_size else "book_min_order_size",
    )
    return candidate


def load_market_by_slug(args: argparse.Namespace, slug: str, requested_outcome: str) -> dict[str, Any]:
    markets = fetch_json(args.gamma_url, "/markets", {"slug": slug}, args.timeout_seconds)
    if isinstance(markets, list) and markets:
        return select_market_for_outcome(markets, requested_outcome=requested_outcome, slug=slug)
    events = fetch_json(args.gamma_url, "/events", {"slug": slug}, args.timeout_seconds)
    if isinstance(events, list) and events:
        event = events[0]
        if isinstance(event, dict):
            event_markets = event.get("markets")
            if isinstance(event_markets, list) and event_markets:
                market = select_market_for_outcome(
                    event_markets,
                    requested_outcome=requested_outcome,
                    slug=slug,
                )
                market.setdefault("slug", slug)
                return market
    raise CandidateError(
        f"Gamma API did not return a market for slug {slug!r} and outcome {requested_outcome!r}"
    )


def scan(args: argparse.Namespace) -> tuple[Candidate, dict[str, Any]]:
    order_cap = as_decimal(args.max_order_notional_usd)
    target_size = as_decimal(args.target_size) if args.target_size else None
    if order_cap is None or order_cap <= 0:
        raise CandidateError("--max-order-notional-usd must be a positive decimal")
    if args.target_size and (target_size is None or target_size <= 0):
        raise CandidateError("--target-size must be a positive decimal")
    if args.max_markets <= 0:
        raise CandidateError("--max-markets must be positive")
    if args.max_clob_requests is not None and args.max_clob_requests <= 0:
        raise CandidateError("--max-clob-requests must be positive")
    if args.max_spread_bps < 0:
        raise CandidateError("--max-spread-bps must be non-negative")
    if args.exchange_rule_valid_for_minutes <= 0:
        raise CandidateError("--exchange-rule-valid-for-minutes must be positive")
    human_review_ref = args.human_review_ref.strip()
    if not is_concrete_external_ref(human_review_ref):
        raise CandidateError("--human-review-ref must be a concrete external review reference")
    exchange_rule_evidence_ref = args.exchange_rule_evidence_ref.strip()
    if not is_concrete_external_ref(exchange_rule_evidence_ref):
        raise CandidateError("--exchange-rule-evidence-ref must be a concrete external review reference")
    if exchange_rule_evidence_ref == human_review_ref:
        raise CandidateError("--exchange-rule-evidence-ref must differ from --human-review-ref")
    if args.market_url and args.market_slug:
        raise CandidateError("use only one of --market-url or --market-slug")
    if (args.market_url or args.market_slug) and not args.outcome:
        raise CandidateError("--outcome is required with --market-url or --market-slug")
    snapshot_at = dt.datetime.now(dt.UTC).isoformat()
    snapshot_dt = dt.datetime.fromisoformat(snapshot_at)
    min_end_days = getattr(args, "min_end_days", None)
    min_end_at = snapshot_dt + dt.timedelta(days=min_end_days) if min_end_days is not None else None

    requested_slug = args.market_slug or (slug_from_market_url(args.market_url) if args.market_url else None)
    audit: dict[str, Any] = {
        "generated_at": snapshot_at,
        "source": "public-read-only",
        "remote_side_effects": False,
        "authorized_for_live": False,
        "side": "BUY",
        "order_type": "GTC",
        "post_only": True,
        "market_url": args.market_url,
        "market_slug": requested_slug,
        "requested_outcome": args.outcome,
        "human_review_ref_hash": hashlib.sha256(human_review_ref.encode()).hexdigest(),
        "gamma_url_ref": audit_url_ref(args.gamma_url),
        "clob_url_ref": audit_url_ref(args.clob_url),
        "max_markets": args.max_markets,
        "max_clob_requests": clob_request_limit(args),
        "clob_requests_used": 0,
        "target_size": decimal_text(target_size) if target_size else "book_min_order_size",
        "target_size_source": "operator_override" if target_size else "book_min_order_size",
        "max_order_notional_usd": decimal_text(order_cap),
        "max_spread_bps": args.max_spread_bps,
        "min_end_days": min_end_days,
        "min_end_at": min_end_at.isoformat() if min_end_at is not None else None,
        "inspected_markets": 0,
        "inspected_tokens": 0,
        "rejections": {
            "inactive": 0,
            "not_accepting_orders": 0,
            "closed": 0,
            "archived": 0,
            "missing_market_id": 0,
            "missing_token_id": 0,
            "book_unavailable": 0,
            "spread_unavailable": 0,
            "spread_too_wide": 0,
            "target_size_above_top_ask_size": 0,
            "target_notional_above_cap": 0,
            "post_only_price_unavailable": 0,
            "min_order_size_above_target_size": 0,
            "missing_or_too_early_end_date": 0,
            "clob_request_budget_exhausted": 0,
        },
    }

    if requested_slug:
        market = load_market_by_slug(args, requested_slug, args.outcome)
        candidate = candidate_from_market(
            args,
            market,
            requested_outcome=args.outcome,
            order_cap=order_cap,
            requested_target_size=target_size,
            snapshot_at=snapshot_at,
            human_review_ref=human_review_ref,
            exchange_rule_evidence_ref=exchange_rule_evidence_ref,
            audit=audit,
        )
        audit["candidate_count"] = 1
        return candidate, audit

    markets: list[dict[str, Any]] = []
    page_size = min(args.max_markets, 100)
    for offset in range(0, args.max_markets, page_size):
        page = fetch_json(
            args.gamma_url,
            "/markets",
            {
                "active": "true",
                "closed": "false",
                "archived": "false",
                "limit": str(min(page_size, args.max_markets - offset)),
                "offset": str(offset),
                "order": "volume24hr",
                "ascending": "false",
            },
            args.timeout_seconds,
        )
        if not isinstance(page, list):
            raise CandidateError("Gamma /markets response was not a JSON array")
        markets.extend(market for market in page if isinstance(market, dict))
        if len(page) < page_size:
            break

    candidates: list[Candidate] = []
    max_clob_requests = clob_request_limit(args)
    for market in markets[: args.max_markets]:
        if not isinstance(market, dict):
            continue
        audit["inspected_markets"] += 1
        active = as_bool(market.get("active"))
        accepting_orders = as_bool(
            market.get("acceptingOrders", market.get("accepting_orders", market.get("enableOrderBook")))
        )
        closed = as_bool(market.get("closed"))
        archived = as_bool(market.get("archived"))
        if not active:
            audit["rejections"]["inactive"] += 1
            continue
        if not accepting_orders:
            audit["rejections"]["not_accepting_orders"] += 1
            continue
        if closed:
            audit["rejections"]["closed"] += 1
            continue
        if archived:
            audit["rejections"]["archived"] += 1
            continue
        market_end_at = parse_market_end_at(market)
        if min_end_at is not None and (market_end_at is None or market_end_at <= min_end_at):
            audit["rejections"]["missing_or_too_early_end_date"] += 1
            continue
        condition_id = market_id(market)
        if not condition_id:
            audit["rejections"]["missing_market_id"] += 1
            continue
        token_ids = parse_token_ids(market)
        if not token_ids:
            audit["rejections"]["missing_token_id"] += 1
            continue
        outcomes = parse_outcomes(market)
        if len(outcomes) != len(token_ids):
            audit["rejections"]["missing_token_id"] += 1
            continue
        for outcome, token_id in zip(outcomes, token_ids, strict=True):
            audit["inspected_tokens"] += 1
            if audit["clob_requests_used"] >= max_clob_requests:
                audit["rejections"]["clob_request_budget_exhausted"] += 1
                break
            try:
                audit["clob_requests_used"] += 1
                book = fetch_json(args.clob_url, "/book", {"token_id": token_id}, args.timeout_seconds)
            except (urllib.error.URLError, urllib.error.HTTPError, TimeoutError, OSError, json.JSONDecodeError, ValueError):
                audit["rejections"]["book_unavailable"] += 1
                continue
            if not isinstance(book, dict):
                audit["rejections"]["book_unavailable"] += 1
                continue
            top_ask = best_ask(book)
            top_bid = best_bid(book)
            min_order_size = as_decimal(book.get("min_order_size"))
            min_tick_size = as_decimal(book.get("min_tick_size", book.get("tick_size")))
            if min_order_size is None or min_order_size <= 0:
                audit["rejections"]["min_order_size_above_target_size"] += 1
                continue
            if min_tick_size is None or min_tick_size <= 0:
                audit["rejections"]["post_only_price_unavailable"] += 1
                continue
            if top_ask is None:
                audit["rejections"]["book_unavailable"] += 1
                continue
            price, size = top_ask
            limit_price = post_only_buy_limit_price(
                price,
                min_tick_size,
                best_bid_price=top_bid[0] if top_bid is not None else None,
            )
            if limit_price is None:
                audit["rejections"]["post_only_price_unavailable"] += 1
                continue
            if audit["clob_requests_used"] >= max_clob_requests:
                audit["rejections"]["clob_request_budget_exhausted"] += 1
                break
            try:
                audit["clob_requests_used"] += 1
                spread = fetch_json(args.clob_url, "/spread", {"token_id": token_id}, args.timeout_seconds)
            except (urllib.error.URLError, urllib.error.HTTPError, TimeoutError, OSError, json.JSONDecodeError, ValueError):
                audit["rejections"]["spread_unavailable"] += 1
                continue
            if not isinstance(spread, dict):
                audit["rejections"]["spread_unavailable"] += 1
                continue
            bps = spread_bps(book, spread)
            if bps is None:
                audit["rejections"]["spread_unavailable"] += 1
                continue
            if bps > args.max_spread_bps:
                audit["rejections"]["spread_too_wide"] += 1
                continue
            candidate_target_size = target_size or min_order_size
            if candidate_target_size <= 0:
                audit["rejections"]["min_order_size_above_target_size"] += 1
                continue
            if size < candidate_target_size:
                audit["rejections"]["target_size_above_top_ask_size"] += 1
                continue
            if min_order_size > candidate_target_size:
                audit["rejections"]["min_order_size_above_target_size"] += 1
                continue
            if limit_price * candidate_target_size > order_cap:
                audit["rejections"]["target_notional_above_cap"] += 1
                continue
            candidates.append(
                Candidate(
                    market_id=condition_id,
                    token_id=token_id,
                    outcome=outcome,
                    market_slug=str(market.get("slug") or ""),
                    market_end_at=market_end_at.isoformat() if market_end_at is not None else "",
                    active=True,
                    accepting_orders=True,
                    closed=False,
                    archived=False,
                    best_ask=price,
                    limit_price=limit_price,
                    ask_size=size,
                    target_size=candidate_target_size,
                    spread_bps=bps,
                    min_order_size=min_order_size,
                    min_tick_size=min_tick_size,
                    liquidity_score=liquidity_score(size, price, bps),
                    source_market_hash=market_fingerprint(market, book=book, spread=spread),
                    book_snapshot_timestamp=snapshot_at,
                    human_review_ref=human_review_ref,
                    exchange_rule_evidence_ref=exchange_rule_evidence_ref,
                    exchange_rule_valid_for_minutes=args.exchange_rule_valid_for_minutes,
                )
            )
        if audit["clob_requests_used"] >= max_clob_requests:
            break

    if not candidates:
        raise CandidateError("no candidate satisfied the configured canary market constraints", audit)
    selected = sorted(candidates, key=candidate_sort_key)[0]
    audit["candidate_count"] = len(candidates)
    audit["selected"] = {
        "market_id": selected.market_id,
        "token_id_hash": hashlib.sha256(selected.token_id.encode()).hexdigest(),
        "outcome": selected.outcome,
        "market_slug": selected.market_slug,
        "market_end_at": selected.market_end_at,
        "source_market_hash": selected.source_market_hash,
        "spread_bps": selected.spread_bps,
        "target_size": decimal_text(selected.target_size),
        "target_size_source": "operator_override" if target_size else "book_min_order_size",
        "estimated_order_notional_usd": decimal_text(selected.limit_price * selected.target_size),
        "limit_price": decimal_text(selected.limit_price),
        "top_ask_notional_usd": decimal_text(selected.best_ask * selected.ask_size),
        "liquidity_score": selected.liquidity_score,
    }
    audit["selected_candidate_summary"] = selected_candidate_summary(
        selected,
        target_size_source="operator_override" if target_size else "book_min_order_size",
    )
    return selected, audit


def write_json(path: Path, data: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")


def main() -> int:
    args = parse_args()
    try:
        candidate, audit = scan(args)
    except CandidateError as exc:
        if args.audit_output and exc.audit is not None:
            exc.audit["status"] = "failed"
            exc.audit["error"] = str(exc)
            write_json(args.audit_output, exc.audit)
        print(f"candidate preparation failed: {exc}", file=sys.stderr)
        return 1
    write_json(args.output, candidate.to_engine_json())
    if args.audit_output:
        write_json(args.audit_output, audit)
    print(
        json.dumps(
            {
                "status": "ok",
                "candidate_market": str(args.output),
                "audit_output": str(args.audit_output) if args.audit_output else None,
                "remote_side_effects": False,
                "authorized_for_live": False,
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
