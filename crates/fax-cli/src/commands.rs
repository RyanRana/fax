use fax_types::*;
use fax_protocol::*;
use fax_chain::*;

pub fn identity(domain: &str, name: &str) {
    println!("=== FAX Agent Identity Generator ===\n");

    let id = AgentIdentity::generate(domain, name).expect("failed to generate identity");

    println!("  DID:         {}", id.did);
    println!("  Name:        {}", id.display_name);
    println!("  Public Key:  {}", id.public_key_hex());
    println!("  EVM Address: {}", id.evm_address.as_deref().unwrap_or("N/A"));
    println!();
    println!("  This identity can sign FAX credentials and anchor");
    println!("  VC chain hashes on EVM-compatible L2 chains.");
}

pub fn rates() {
    println!("=== FAX Resource Credit Unit (RCU) Rates ===\n");
    println!("  1 RCU = cost of 1,000 LLM tokens on a mid-tier model\n");

    let resources = vec![
        ("Compute (GPU-hour)", ResourceType::Compute, Some("gpu-hour"), "gpu-hour"),
        ("Compute (CPU-hour)", ResourceType::Compute, Some("cpu-hour"), "cpu-hour"),
        ("LLM Tokens (1K)", ResourceType::LlmTokens, None, "tokens"),
        ("Knowledge Query", ResourceType::KnowledgeAccess, None, "query"),
        ("Tool Invocation", ResourceType::ToolAccess, None, "invocation"),
        ("Research Report", ResourceType::ResearchReport, None, "report"),
        ("Data Feed Record", ResourceType::DataFeed, None, "record"),
        ("Schedule Slot", ResourceType::ScheduleSlot, None, "slot"),
        ("Storage (MB-month)", ResourceType::StorageQuota, None, "MB-month"),
        ("Bandwidth (GB)", ResourceType::Bandwidth, None, "GB"),
        ("Attestation", ResourceType::Attestation, None, "attestation"),
    ];

    println!("  {:<25} {:>10} {:>12}", "Resource", "Rate/unit", "Unit");
    println!("  {}", "-".repeat(50));

    for (label, rtype, subtype, unit) in resources {
        let r = ResourceAmount {
            resource_type: rtype,
            amount: 1.0,
            unit: unit.to_string(),
            subtype: subtype.map(|s| s.to_string()),
        };
        let rcu = RcuOracle::to_rcu(&r).unwrap_or(0.0);
        println!("  {:<25} {:>8.2} RCU {:>10}", label, rcu, unit);
    }

    println!("\n  Example trades:");
    println!("    2 GPU-hours = {:.0} RCU = {:.0} LLM tokens",
        100.0, 100_000.0);
    println!("    1 research report = {:.0} RCU = {:.0} GPU-hours",
        200.0, 4.0);
}

pub async fn demo() {
    println!("=== FAX Full Trade Demo ===\n");

    // 1. Generate identities
    println!("[1/8] Generating agent identities...");
    let alice = AgentIdentity::generate("compute-provider.io", "alice").unwrap();
    let bob = AgentIdentity::generate("knowledge-hub.ai", "bob").unwrap();
    println!("  Alice: {} (compute provider)", alice.did);
    println!("  Bob:   {} (knowledge provider)", bob.did);

    // 2. Create offers
    println!("\n[2/8] Alice creates resource offer...");
    let mut alice_trade = Trade::new(&alice.did);
    let _offer = alice_trade.create_offer(
        vec![ResourceAmount::new(ResourceType::Compute, 2.0, "gpu-hour").with_subtype("gpu-hour")],
        vec![ResourceAmount::new(ResourceType::LlmTokens, 100_000.0, "tokens")],
    ).unwrap();
    println!("  Offering: 2 GPU-hours (100 RCU)");
    println!("  Requesting: 100K LLM tokens (100 RCU)");
    println!("  Trade balance: ~0% imbalance");

    // 3. Negotiate security
    println!("\n[3/8] Negotiating security level...");
    let alice_sec = SecurityProposal::new(SecurityLevel::Escrow)
        .with_acceptable(vec![SecurityLevel::Anchor, SecurityLevel::Escrow, SecurityLevel::FullEscrow])
        .with_reputation(850);
    let bob_sec = SecurityProposal::new(SecurityLevel::Anchor)
        .with_acceptable(vec![SecurityLevel::Anchor, SecurityLevel::Escrow]);
    let agreed_level = negotiate_security_level(&alice_sec, &bob_sec).unwrap();
    println!("  Alice proposes: {}", SecurityLevel::Escrow);
    println!("  Bob proposes:   {}", SecurityLevel::Anchor);
    println!("  Agreed:         {}", agreed_level);

    // 4. Create swap agreement
    println!("\n[4/8] Signing swap agreement...");
    let mut alice_swap = SwapEngine::new(&alice_trade.id, 3600);
    let mut bob_swap = SwapEngine::new(&alice_trade.id, 3600);
    println!("  Trade ID: {}", alice_trade.id);
    println!("  Lock duration: 3600 seconds");

    // 5. Lock resources (hash-lock)
    println!("\n[5/8] Locking resources with hash-locks...");
    let alice_lock = alice_swap.create_lock_credential(&alice.did, "wss://alice.compute-provider.io/gpu").unwrap();
    println!("  Alice locked: 2 GPU-hours at wss://alice.compute-provider.io/gpu");
    println!("  Alice hash-lock: {}...", &alice_swap.my_secret.hash_lock_hex()[..16]);

    bob_swap.receive_lock(alice_lock).unwrap();
    let bob_lock = bob_swap.create_lock_credential(&bob.did, "https://bob.knowledge-hub.ai/api/tokens").unwrap();
    println!("  Bob locked: 100K tokens at https://bob.knowledge-hub.ai/api/tokens");
    println!("  Bob hash-lock: {}...", &bob_swap.my_secret.hash_lock_hex()[..16]);

    alice_swap.receive_lock(bob_lock).unwrap();
    println!("  Both resources locked. Swap state: {:?}", alice_swap.state);

    // 6. Exchange (reveal secrets)
    println!("\n[6/8] Exchanging resources (revealing secrets)...");
    let alice_delivery = alice_swap.create_delivery_credential(&alice.did).unwrap();
    println!("  Alice revealed secret: {}...", &alice_swap.my_secret.secret_hex()[..16]);

    bob_swap.receive_delivery(alice_delivery).unwrap();
    println!("  Bob verified Alice's secret. Bob can now access GPU compute.");

    let bob_delivery = bob_swap.create_delivery_credential(&bob.did).unwrap();
    println!("  Bob revealed secret: {}...", &bob_swap.my_secret.secret_hex()[..16]);

    alice_swap.receive_delivery(bob_delivery).unwrap();
    println!("  Alice verified Bob's secret. Alice can now access LLM tokens.");
    println!("  Swap state: {:?}", alice_swap.state);

    // 7. Complete and anchor
    println!("\n[7/8] Finalizing and anchoring on L2...");
    let completion = alice_swap.create_completion_credential(&alice.did, &bob.did).unwrap();
    println!("  Completion credential: {}", completion.id);

    let chain_tip = alice_swap.chain_tip_hash().unwrap();
    println!("  VC chain tip hash: {}...", &chain_tip[..32]);
    println!("  Chain length: {} credentials", alice_swap.chain.len());

    let mut chain_client = ChainClient::new(ChainConfig::local());
    let evm_addr = alice.evm_address.as_deref().unwrap_or("0x0");
    let receipt = chain_client.anchor_hash(evm_addr, &chain_tip).await.unwrap();
    println!("  Anchored on L2 block #{}", receipt.block_number);
    println!("  Tx hash: {}...", &receipt.tx_hash[..32]);

    // 8. Verify
    println!("\n[8/8] Verifying...");
    alice_swap.verify_chain().unwrap();
    println!("  VC chain integrity: VALID");

    let anchor_exists = chain_client.verify_anchor(evm_addr, &chain_tip).await.unwrap();
    println!("  On-chain anchor: {}", if anchor_exists.is_some() { "VERIFIED" } else { "NOT FOUND" });

    // Summary
    println!("\n{}", "=".repeat(60));
    println!("  TRADE COMPLETE");
    println!("{}", "=".repeat(60));
    println!("  Alice gave:     2 GPU-hours (100 RCU)");
    println!("  Bob gave:       100,000 LLM tokens (100 RCU)");
    println!("  Security:       {}", agreed_level);
    println!("  Chain length:   {} credentials", alice_swap.chain.len());
    println!("  Anchored:       L2 block #{}", receipt.block_number);
    println!("  Tamper-proof:   VC hash chain + L2 anchor");
    println!("{}", "=".repeat(60));
}

pub fn reputation(trades: u64, disputes: u64) {
    println!("=== FAX Reputation Simulator ===\n");

    let mut service = ReputationService::new();

    service.register("0xAgent");

    for _ in 0..trades {
        service.record_completion("0xAgent", "0xCounterparty", 50.0, false);
    }
    for _ in 0..disputes {
        service.record_completion("0xAgent", "0xCounterparty", 50.0, true);
        service.record_dispute_loss("0xAgent");
    }

    let rep = service.get_reputation("0xAgent").unwrap();
    let score = service.get_score("0xAgent");

    println!("  Total trades:     {}", rep.total_trades);
    println!("  Successful:       {}", rep.successful_trades);
    println!("  Disputes lost:    {}", rep.disputes_lost);
    println!("  Total RCU traded: {:.0}", rep.total_rcu_traded);
    println!("  Reliability:      {}/1000", score);
    println!();

    let trust_tier = match score {
        0..=299 => "UNTRUSTED — only Level 3 (Full Escrow) trades accepted",
        300..=599 => "LOW — Level 2+ (Escrow) required",
        600..=799 => "MODERATE — Level 1+ (Anchor) sufficient",
        800..=1000 => "HIGH — Level 0 (Trust) accepted by most agents",
        _ => "UNKNOWN",
    };
    println!("  Trust tier: {trust_tier}");
}
