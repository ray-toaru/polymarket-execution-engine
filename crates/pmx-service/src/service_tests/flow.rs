use super::*;

#[tokio::test]
async fn service_flow_persists_and_blocks_submit() {
    let service = ExecutorService::new(InMemoryStore::default());
    let normalized = service.normalize(intent()).await.expect("normalize");
    let snapshot = service
        .capture_snapshot(normalized.clone())
        .await
        .expect("snapshot");
    let decision = service
        .evaluate_decision(DecisionRequest {
            normalized_intent: normalized.clone(),
            snapshot: snapshot.clone(),
            correlation_id: None,
        })
        .await
        .expect("decision");
    let plan = service
        .compile_plan(CompilePlanCommand {
            normalized_intent: normalized,
            snapshot: snapshot.clone(),
            decision: decision.clone(),
            approval: approval_for(&snapshot, &decision),
            correlation_id: None,
        })
        .await
        .expect("plan");
    let outcome = service
        .submit_plan(SubmitPlanCommand {
            execution_id: plan.execution_id.clone(),
            plan_hash: plan.plan_hash.0.clone(),
            idempotency_key: "idem-1".into(),
            mode: SubmitMode::BlockedDryRun,
            correlation_id: Some("corr-blocked-1".into()),
        })
        .await
        .expect("submit");
    match outcome {
        SubmitOutcome::Accepted(receipt) => assert_eq!(receipt.status, SubmitStatus::Blocked),
        SubmitOutcome::Replayed(_) => panic!("first submit cannot replay"),
    }
    let events = service
        .list_execution_lifecycle_events(pmx_store::ExecutionLifecycleQuery {
            execution_id: plan.execution_id.clone(),
            limit: 10,
            before_event_id: None,
        })
        .await
        .expect("lifecycle events");
    let blocked = events
        .iter()
        .find(|event| event.event_type == "SUBMIT_BLOCKED_BEFORE_REMOTE")
        .expect("blocked event");
    assert_eq!(blocked.payload["correlation_id"], "corr-blocked-1");
    assert_eq!(blocked.payload["reservation_written"], false);
}

#[tokio::test]
async fn service_id_bound_flow_persists_and_blocks_submit() {
    let service = ExecutorService::new(InMemoryStore::default());
    let normalized = service.normalize(intent()).await.expect("normalize");
    let snapshot = service
        .capture_snapshot(normalized.clone())
        .await
        .expect("snapshot");
    let decision = service
        .evaluate_decision_by_id(DecisionByIdRequest {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            correlation_id: None,
        })
        .await
        .expect("decision by id");
    let plan = service
        .compile_plan_by_id(CompilePlanByIdCommand {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            decision_id: decision.decision_id.clone(),
            approval: approval_for(&snapshot, &decision),
            correlation_id: None,
        })
        .await
        .expect("plan by id");
    let outcome = service
        .submit_plan(SubmitPlanCommand {
            execution_id: plan.execution_id.clone(),
            plan_hash: plan.plan_hash.0.clone(),
            idempotency_key: "idem-id-bound-1".into(),
            mode: SubmitMode::BlockedDryRun,
            correlation_id: None,
        })
        .await
        .expect("submit");
    match outcome {
        SubmitOutcome::Accepted(receipt) => assert_eq!(receipt.status, SubmitStatus::Blocked),
        SubmitOutcome::Replayed(_) => panic!("first submit cannot replay"),
    }
}

#[tokio::test]
async fn service_id_bound_flow_propagates_correlation_id_across_object_graph() {
    let service = ExecutorService::new(InMemoryStore::default());
    let correlation_id = "corr-flow-object-graph-1".to_string();
    let normalized = service
        .normalize_with_correlation(intent(), Some(correlation_id.clone()))
        .await
        .expect("normalize");
    assert_eq!(
        normalized.correlation_id.as_deref(),
        Some(correlation_id.as_str())
    );

    let snapshot = service
        .capture_snapshot_with_correlation(normalized.clone(), Some(correlation_id.clone()))
        .await
        .expect("snapshot");
    assert_eq!(
        snapshot.correlation_id.as_deref(),
        Some(correlation_id.as_str())
    );

    let decision = service
        .evaluate_decision_by_id(DecisionByIdRequest {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            correlation_id: Some(correlation_id.clone()),
        })
        .await
        .expect("decision by id");
    assert_eq!(
        decision.correlation_id.as_deref(),
        Some(correlation_id.as_str())
    );

    let plan = service
        .compile_plan_by_id(CompilePlanByIdCommand {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            decision_id: decision.decision_id.clone(),
            approval: approval_for(&snapshot, &decision),
            correlation_id: Some(correlation_id.clone()),
        })
        .await
        .expect("plan by id");
    assert_eq!(
        plan.correlation_id.as_deref(),
        Some(correlation_id.as_str())
    );

    let stored_normalized = service
        .store()
        .load_normalized_intent(&normalized.normalized_intent_id)
        .await
        .expect("stored normalized");
    let stored_snapshot = service
        .store()
        .load_snapshot(&snapshot.snapshot_id)
        .await
        .expect("stored snapshot");
    let stored_decision = service
        .store()
        .load_decision(&decision.decision_id)
        .await
        .expect("stored decision");
    let stored_plan = service
        .store()
        .load_plan_summary(&plan.execution_id)
        .await
        .expect("stored plan");

    assert_eq!(
        stored_normalized.correlation_id.as_deref(),
        Some(correlation_id.as_str())
    );
    assert_eq!(
        stored_snapshot.correlation_id.as_deref(),
        Some(correlation_id.as_str())
    );
    assert_eq!(
        stored_decision.correlation_id.as_deref(),
        Some(correlation_id.as_str())
    );
    assert_eq!(
        stored_plan.correlation_id.as_deref(),
        Some(correlation_id.as_str())
    );
}

#[tokio::test]
async fn service_rejects_object_graph_mismatch() {
    let service = ExecutorService::new(InMemoryStore::default());
    let normalized = service.normalize(intent()).await.expect("normalize");
    let mut snapshot = service
        .capture_snapshot(normalized.clone())
        .await
        .expect("snapshot");
    snapshot.normalized_intent_id = "other".into();
    let err = service
        .evaluate_decision(DecisionRequest {
            normalized_intent: normalized,
            snapshot,
            correlation_id: None,
        })
        .await
        .expect_err("mismatched snapshot must fail");
    assert!(matches!(err, ServiceError::Conflict(_)));
}

#[tokio::test]
async fn service_rejects_tampered_approval_hash() {
    let service = ExecutorService::new(InMemoryStore::default());
    let normalized = service.normalize(intent()).await.expect("normalize");
    let snapshot = service
        .capture_snapshot(normalized.clone())
        .await
        .expect("snapshot");
    let decision = service
        .evaluate_decision(DecisionRequest {
            normalized_intent: normalized.clone(),
            snapshot: snapshot.clone(),
            correlation_id: None,
        })
        .await
        .expect("decision");
    let mut approval = approval_for(&snapshot, &decision);
    approval.approval_hash = hash_value("tampered-approval-hash");

    let err = service
        .compile_plan(CompilePlanCommand {
            normalized_intent: normalized,
            snapshot,
            decision,
            approval,
            correlation_id: None,
        })
        .await
        .expect_err("approval hash must be recomputed and checked");
    assert!(matches!(err, ServiceError::Conflict(_)));
}

#[tokio::test]
async fn static_runtime_provider_can_reach_ready_plan_but_submit_still_blocks() {
    let service = ExecutorService::with_runtime_provider(
        InMemoryStore::default(),
        StaticRuntimeStateProvider::new(allow_runtime_state()),
        "test-executor".into(),
        DEFAULT_CONTRACT_VERSION.into(),
    );
    let normalized = service.normalize(intent()).await.expect("normalize");
    let snapshot = service
        .capture_snapshot(normalized.clone())
        .await
        .expect("snapshot");
    let decision = service
        .evaluate_decision_by_id(DecisionByIdRequest {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            correlation_id: None,
        })
        .await
        .expect("decision");
    assert_eq!(decision.status, DecisionStatus::Allow);
    let plan = service
        .compile_plan_by_id(CompilePlanByIdCommand {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            decision_id: decision.decision_id.clone(),
            approval: approval_for(&snapshot, &decision),
            correlation_id: None,
        })
        .await
        .expect("plan");
    assert_eq!(plan.status, PlanStatus::Ready);
    assert_eq!(plan.max_exposure, DecimalString("1".into()));
    let outcome = service
        .submit_plan(SubmitPlanCommand {
            execution_id: plan.execution_id.clone(),
            plan_hash: plan.plan_hash.0.clone(),
            idempotency_key: "idem-ready-still-blocked".into(),
            mode: SubmitMode::BlockedDryRun,
            correlation_id: None,
        })
        .await
        .expect("submit");
    match outcome {
        SubmitOutcome::Accepted(receipt) => assert_eq!(receipt.status, SubmitStatus::Blocked),
        SubmitOutcome::Replayed(_) => panic!("first submit cannot replay"),
    }
}

#[tokio::test]
async fn service_market_data_snapshot_blocks_stale_or_insufficient_top_book() {
    let service = ExecutorService::with_runtime_provider(
        InMemoryStore::default(),
        StaticRuntimeStateProvider::new(allow_runtime_state()),
        "test-executor".into(),
        DEFAULT_CONTRACT_VERSION.into(),
    );
    let normalized = service.normalize(buy_base_share_intent()).await.unwrap();
    let gateway = pmx_gateway::FakeGateway::new();
    gateway.insert_market_book_for_test(MarketBookSnapshot {
        condition_id: normalized.market.condition_id.clone(),
        token_id: normalized.token_id.clone(),
        bids: vec![BookLevel {
            price: DecimalString("0.49".into()),
            shares: DecimalString("10".into()),
        }],
        asks: vec![BookLevel {
            price: DecimalString("0.5".into()),
            shares: DecimalString("1".into()),
        }],
        observed_at_ms: 1_000,
        valid_for_ms: 100,
    });

    let stale_snapshot = service
        .capture_snapshot_with_market_data(normalized.clone(), &gateway, 1_500)
        .await
        .expect("stale market-data snapshot");
    assert!(
        stale_snapshot
            .runtime_state
            .required_capabilities
            .contains(&"market_book_stale".to_string())
    );
    let stale_decision = service
        .evaluate_decision_by_id(DecisionByIdRequest {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: stale_snapshot.snapshot_id,
            correlation_id: None,
        })
        .await
        .expect("stale decision");
    assert_eq!(stale_decision.status, DecisionStatus::Block);
    assert!(
        stale_decision
            .reasons
            .contains(&BlockReason::StaleMarketData)
    );

    let insufficient_snapshot = service
        .capture_snapshot_with_market_data(normalized.clone(), &gateway, 1_050)
        .await
        .expect("insufficient market-data snapshot");
    assert!(
        insufficient_snapshot
            .runtime_state
            .required_capabilities
            .contains(&"market_book_insufficient_top_liquidity".to_string())
    );
    let insufficient_decision = service
        .evaluate_decision_by_id(DecisionByIdRequest {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: insufficient_snapshot.snapshot_id,
            correlation_id: None,
        })
        .await
        .expect("insufficient decision");
    assert_eq!(insufficient_decision.status, DecisionStatus::Block);
    assert!(
        insufficient_decision
            .reasons
            .contains(&BlockReason::InsufficientTopBookLiquidity)
    );
}

#[tokio::test]
async fn live_submit_mode_fails_closed_until_gateway_is_wired() {
    let service = ExecutorService::with_runtime_provider(
        InMemoryStore::default(),
        StaticRuntimeStateProvider::new(allow_runtime_state()),
        "test-executor".into(),
        DEFAULT_CONTRACT_VERSION.into(),
    );
    let normalized = service.normalize(intent()).await.expect("normalize");
    let snapshot = service
        .capture_snapshot(normalized.clone())
        .await
        .expect("snapshot");
    let decision = service
        .evaluate_decision_by_id(DecisionByIdRequest {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            correlation_id: None,
        })
        .await
        .expect("decision");
    let plan = service
        .compile_plan_by_id(CompilePlanByIdCommand {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            decision_id: decision.decision_id.clone(),
            approval: approval_for(&snapshot, &decision),
            correlation_id: None,
        })
        .await
        .expect("plan");
    let err = service
        .submit_plan(SubmitPlanCommand {
            execution_id: plan.execution_id.clone(),
            plan_hash: plan.plan_hash.0.clone(),
            idempotency_key: "idem-live-fail-closed".into(),
            mode: SubmitMode::Live,
            correlation_id: None,
        })
        .await
        .expect_err("live mode must fail closed until gateway is wired");
    assert!(matches!(err, ServiceError::Conflict(_)));
}

#[tokio::test]
async fn explicit_live_gateway_posts_and_records_remote_order_lifecycle() {
    let service = ExecutorService::with_runtime_provider(
        InMemoryStore::default(),
        StaticRuntimeStateProvider::new(allow_runtime_state()),
        "test-executor".into(),
        DEFAULT_CONTRACT_VERSION.into(),
    );
    let normalized = service
        .normalize(sell_base_share_intent())
        .await
        .expect("normalize");
    let snapshot = service
        .capture_snapshot(normalized.clone())
        .await
        .expect("snapshot");
    let decision = service
        .evaluate_decision_by_id(DecisionByIdRequest {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            correlation_id: None,
        })
        .await
        .expect("decision");
    assert_eq!(decision.status, DecisionStatus::Allow);
    let plan = service
        .compile_plan_by_id(CompilePlanByIdCommand {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            decision_id: decision.decision_id.clone(),
            approval: approval_for(&snapshot, &decision),
            correlation_id: None,
        })
        .await
        .expect("plan");
    let signer = pmx_gateway::DeterministicTestSignerProvider;
    let gateway = pmx_gateway::FakeGateway::new();
    let outcome = service
        .submit_plan_with_gateway(
            SubmitPlanCommand {
                execution_id: plan.execution_id.clone(),
                plan_hash: plan.plan_hash.0.clone(),
                idempotency_key: "idem-live-posted".into(),
                mode: SubmitMode::Live,
                correlation_id: Some("corr-live-posted".into()),
            },
            &signer,
            &gateway,
        )
        .await
        .expect("live submit");
    let receipt = match outcome {
        SubmitOutcome::Accepted(receipt) => receipt,
        SubmitOutcome::Replayed(_) => panic!("first submit cannot replay"),
    };
    assert_eq!(receipt.status, SubmitStatus::Posted);
    let order_id = format!("test-order-{}", plan.execution_id);
    let lifecycle = service
        .store()
        .load_order_lifecycle(&order_id)
        .await
        .expect("load order")
        .expect("order lifecycle");
    assert_eq!(lifecycle.lifecycle_state, OrderLifecycleState::Posted);
    assert_eq!(
        lifecycle.remote_order_id,
        Some(format!("remote-{order_id}"))
    );
    let order_events = service
        .store()
        .list_order_lifecycle_events(&pmx_store::OrderLifecycleEventQuery {
            order_id: order_id.clone(),
            limit: 20,
            before_event_id: None,
        })
        .await
        .expect("order events");
    assert!(order_events.iter().all(|event| {
        event
            .correlation_id
            .as_deref()
            .is_some_and(|value| value.starts_with("corr-live-posted:"))
    }));
}

#[tokio::test]
async fn explicit_live_gateway_posts_buy_size_and_records_quote_exposure() {
    let service = ExecutorService::with_runtime_provider(
        InMemoryStore::default(),
        StaticRuntimeStateProvider::new(allow_runtime_state()),
        "test-executor".into(),
        DEFAULT_CONTRACT_VERSION.into(),
    );
    let normalized = service
        .normalize(buy_base_share_intent())
        .await
        .expect("normalize");
    assert!(matches!(
        normalized.quantity_bound,
        QuantityBound::WorstCaseBaseShares(_)
    ));
    let snapshot = service
        .capture_snapshot(normalized.clone())
        .await
        .expect("snapshot");
    let decision = service
        .evaluate_decision_by_id(DecisionByIdRequest {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            correlation_id: None,
        })
        .await
        .expect("decision");
    assert_eq!(decision.status, DecisionStatus::Allow);
    let plan = service
        .compile_plan_by_id(CompilePlanByIdCommand {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            decision_id: decision.decision_id.clone(),
            approval: approval_for(&snapshot, &decision),
            correlation_id: None,
        })
        .await
        .expect("plan");
    assert_eq!(plan.status, PlanStatus::Ready);
    assert_eq!(plan.max_exposure, DecimalString("2.5".into()));

    let signer = pmx_gateway::DeterministicTestSignerProvider;
    let gateway = pmx_gateway::FakeGateway::new();
    let outcome = service
        .submit_plan_with_gateway(
            SubmitPlanCommand {
                execution_id: plan.execution_id.clone(),
                plan_hash: plan.plan_hash.0.clone(),
                idempotency_key: "idem-live-buy-size-posted".into(),
                mode: SubmitMode::Live,
                correlation_id: Some("corr-live-buy-size-posted".into()),
            },
            &signer,
            &gateway,
        )
        .await
        .expect("live submit");
    let receipt = match outcome {
        SubmitOutcome::Accepted(receipt) => receipt,
        SubmitOutcome::Replayed(_) => panic!("first submit cannot replay"),
    };
    assert_eq!(receipt.status, SubmitStatus::Posted);
    let order_id = format!("test-order-{}", plan.execution_id);
    let lifecycle = service
        .store()
        .load_order_lifecycle(&order_id)
        .await
        .expect("load order")
        .expect("order lifecycle");
    assert_eq!(lifecycle.side, "Buy");
    assert_eq!(
        lifecycle.remote_order_id,
        Some(format!("remote-{order_id}"))
    );
}

#[tokio::test]
async fn explicit_live_gateway_marks_operator_required_when_runtime_degrades_after_post_ack() {
    let mut post_ack_block = allow_runtime_state();
    post_ack_block.kill_switch_enabled = true;
    let service = ExecutorService::with_runtime_provider(
        InMemoryStore::default(),
        SequencedRuntimeStateProvider::new(vec![
            allow_runtime_state(),
            allow_runtime_state(),
            allow_runtime_state(),
            post_ack_block,
        ]),
        "test-executor".into(),
        DEFAULT_CONTRACT_VERSION.into(),
    );
    let normalized = service
        .normalize(sell_base_share_intent())
        .await
        .expect("normalize");
    let snapshot = service
        .capture_snapshot(normalized.clone())
        .await
        .expect("snapshot");
    let decision = service
        .evaluate_decision_by_id(DecisionByIdRequest {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            correlation_id: None,
        })
        .await
        .expect("decision");
    let plan = service
        .compile_plan_by_id(CompilePlanByIdCommand {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            decision_id: decision.decision_id.clone(),
            approval: approval_for(&snapshot, &decision),
            correlation_id: None,
        })
        .await
        .expect("plan");
    let signer = pmx_gateway::DeterministicTestSignerProvider;
    let gateway = pmx_gateway::FakeGateway::new();
    let outcome = service
        .submit_plan_with_gateway(
            SubmitPlanCommand {
                execution_id: plan.execution_id.clone(),
                plan_hash: plan.plan_hash.0.clone(),
                idempotency_key: "idem-live-post-ack-runtime-degraded".into(),
                mode: SubmitMode::Live,
                correlation_id: Some("corr-live-post-ack-runtime-degraded".into()),
            },
            &signer,
            &gateway,
        )
        .await
        .expect("live submit with post-ack runtime degradation");
    let receipt = match outcome {
        SubmitOutcome::Accepted(receipt) => receipt,
        SubmitOutcome::Replayed(_) => panic!("first submit cannot replay"),
    };
    assert_eq!(receipt.status, SubmitStatus::PartialRemoteUnknown);
    let order_id = format!("test-order-{}", plan.execution_id);
    let lifecycle = service
        .store()
        .load_order_lifecycle(&order_id)
        .await
        .expect("load order")
        .expect("order lifecycle");
    assert_eq!(lifecycle.lifecycle_state, OrderLifecycleState::Posted);
    let events = service
        .list_execution_lifecycle_events(pmx_store::ExecutionLifecycleQuery {
            execution_id: plan.execution_id.clone(),
            limit: 20,
            before_event_id: None,
        })
        .await
        .expect("execution lifecycle events");
    let post_ack_event = events
        .iter()
        .find(|event| event.event_type == "LIVE_SUBMIT_POST_ACK_RUNTIME_DEGRADED")
        .expect("post-ack runtime degraded event");
    assert_eq!(
        post_ack_event.payload["correlation_id"],
        "corr-live-post-ack-runtime-degraded"
    );
    assert_eq!(post_ack_event.payload["body"]["operator_required"], true);
    assert_eq!(post_ack_event.payload["body"]["remote_side_effect"], true);
    assert_eq!(
        post_ack_event.payload["body"]["reason"],
        "kill_switch_enabled"
    );
}

#[tokio::test]
async fn explicit_live_gateway_remote_unknown_freezes_for_operator_review() {
    let service = ExecutorService::with_runtime_provider(
        InMemoryStore::default(),
        StaticRuntimeStateProvider::new(allow_runtime_state()),
        "test-executor".into(),
        DEFAULT_CONTRACT_VERSION.into(),
    );
    let normalized = service
        .normalize(sell_base_share_intent())
        .await
        .expect("normalize");
    let snapshot = service
        .capture_snapshot(normalized.clone())
        .await
        .expect("snapshot");
    let decision = service
        .evaluate_decision_by_id(DecisionByIdRequest {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            correlation_id: None,
        })
        .await
        .expect("decision");
    let plan = service
        .compile_plan_by_id(CompilePlanByIdCommand {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            decision_id: decision.decision_id.clone(),
            approval: approval_for(&snapshot, &decision),
            correlation_id: None,
        })
        .await
        .expect("plan");
    let signer = pmx_gateway::DeterministicTestSignerProvider;
    let gateway = pmx_gateway::FakeGateway::new().with_post_failure(
        pmx_gateway::FakeGatewayFailure::RemoteUnknown("post timeout".into()),
    );
    let outcome = service
        .submit_plan_with_gateway(
            SubmitPlanCommand {
                execution_id: plan.execution_id.clone(),
                plan_hash: plan.plan_hash.0.clone(),
                idempotency_key: "idem-live-remote-unknown".into(),
                mode: SubmitMode::Live,
                correlation_id: Some("corr-live-remote-unknown".into()),
            },
            &signer,
            &gateway,
        )
        .await
        .expect("live submit remote unknown");
    let receipt = match outcome {
        SubmitOutcome::Accepted(receipt) => receipt,
        SubmitOutcome::Replayed(_) => panic!("first submit cannot replay"),
    };
    assert_eq!(receipt.status, SubmitStatus::RemoteUnknown);
    let order_id = format!("test-order-{}", plan.execution_id);
    let lifecycle = service
        .store()
        .load_order_lifecycle(&order_id)
        .await
        .expect("load order")
        .expect("order lifecycle");
    assert_eq!(
        lifecycle.lifecycle_state,
        OrderLifecycleState::RemoteUnknown
    );
}

#[tokio::test]
async fn explicit_live_gateway_remote_rejection_records_failed_lifecycle() {
    let service = ExecutorService::with_runtime_provider(
        InMemoryStore::default(),
        StaticRuntimeStateProvider::new(allow_runtime_state()),
        "test-executor".into(),
        DEFAULT_CONTRACT_VERSION.into(),
    );
    let normalized = service
        .normalize(sell_base_share_intent())
        .await
        .expect("normalize");
    let snapshot = service
        .capture_snapshot(normalized.clone())
        .await
        .expect("snapshot");
    let decision = service
        .evaluate_decision_by_id(DecisionByIdRequest {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            correlation_id: None,
        })
        .await
        .expect("decision");
    let plan = service
        .compile_plan_by_id(CompilePlanByIdCommand {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            decision_id: decision.decision_id.clone(),
            approval: approval_for(&snapshot, &decision),
            correlation_id: None,
        })
        .await
        .expect("plan");
    let signer = pmx_gateway::DeterministicTestSignerProvider;
    let gateway = pmx_gateway::FakeGateway::new().with_post_failure(
        pmx_gateway::FakeGatewayFailure::RemoteRejected("invalid order".into()),
    );
    let outcome = service
        .submit_plan_with_gateway(
            SubmitPlanCommand {
                execution_id: plan.execution_id.clone(),
                plan_hash: plan.plan_hash.0.clone(),
                idempotency_key: "idem-live-remote-rejected".into(),
                mode: SubmitMode::Live,
                correlation_id: Some("corr-live-remote-rejected".into()),
            },
            &signer,
            &gateway,
        )
        .await
        .expect("live submit remote rejected");
    let receipt = match outcome {
        SubmitOutcome::Accepted(receipt) => receipt,
        SubmitOutcome::Replayed(_) => panic!("first submit cannot replay"),
    };
    assert_eq!(receipt.status, SubmitStatus::Rejected);
    let order_id = format!("test-order-{}", plan.execution_id);
    let lifecycle = service
        .store()
        .load_order_lifecycle(&order_id)
        .await
        .expect("load order")
        .expect("order lifecycle");
    assert_eq!(lifecycle.lifecycle_state, OrderLifecycleState::Failed);
}

#[tokio::test]
async fn explicit_live_gateway_blocks_unsupported_quote_notional_without_stuck_idempotency() {
    let service = ExecutorService::with_runtime_provider(
        InMemoryStore::default(),
        StaticRuntimeStateProvider::new(allow_runtime_state()),
        "test-executor".into(),
        DEFAULT_CONTRACT_VERSION.into(),
    );
    let normalized = service.normalize(intent()).await.expect("normalize");
    let snapshot = service
        .capture_snapshot(normalized.clone())
        .await
        .expect("snapshot");
    let decision = service
        .evaluate_decision_by_id(DecisionByIdRequest {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            correlation_id: None,
        })
        .await
        .expect("decision");
    let plan = service
        .compile_plan_by_id(CompilePlanByIdCommand {
            normalized_intent_id: normalized.normalized_intent_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            decision_id: decision.decision_id.clone(),
            approval: approval_for(&snapshot, &decision),
            correlation_id: None,
        })
        .await
        .expect("plan");
    let signer = pmx_gateway::DeterministicTestSignerProvider;
    let gateway = pmx_gateway::FakeGateway::new();
    let command = SubmitPlanCommand {
        execution_id: plan.execution_id.clone(),
        plan_hash: plan.plan_hash.0.clone(),
        idempotency_key: "idem-live-quote-notional-blocked".into(),
        mode: SubmitMode::Live,
        correlation_id: Some("corr-live-quote-notional-blocked".into()),
    };
    let first = service
        .submit_plan_with_gateway(command.clone(), &signer, &gateway)
        .await
        .expect("live quote notional blocked");
    match first {
        SubmitOutcome::Accepted(receipt) => assert_eq!(receipt.status, SubmitStatus::Blocked),
        SubmitOutcome::Replayed(_) => panic!("first submit cannot replay"),
    }
    let replay = service
        .submit_plan_with_gateway(command, &signer, &gateway)
        .await
        .expect("blocked live quote notional replay");
    assert!(
        matches!(replay, SubmitOutcome::Replayed(receipt) if receipt.status == SubmitStatus::Blocked)
    );
}

fn sell_base_share_intent() -> TradeIntent {
    let mut intent = intent();
    intent.side = Side::Sell;
    intent.quantity = QuantityIntent {
        max_notional: None,
        max_shares: Some(DecimalString("5".into())),
    };
    intent
}

fn buy_base_share_intent() -> TradeIntent {
    let mut intent = intent();
    intent.quantity = QuantityIntent {
        max_notional: None,
        max_shares: Some(DecimalString("5".into())),
    };
    intent
}

#[derive(Debug, Clone)]
struct SequencedRuntimeStateProvider {
    states: std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<RuntimeStateSummary>>>,
}

impl SequencedRuntimeStateProvider {
    fn new(states: Vec<RuntimeStateSummary>) -> Self {
        Self {
            states: std::sync::Arc::new(std::sync::Mutex::new(states.into())),
        }
    }
}

#[async_trait::async_trait]
impl RuntimeStateProvider for SequencedRuntimeStateProvider {
    async fn capture_runtime_state(
        &self,
        _normalized_intent: &NormalizedIntent,
    ) -> RuntimeStateSummary {
        self.states
            .lock()
            .expect("sequenced runtime provider lock")
            .pop_front()
            .unwrap_or_else(allow_runtime_state)
    }
}
