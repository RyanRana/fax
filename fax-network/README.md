# FAX — Fast Agent Exchange Network

Blockchain-anchored protocol for autonomous agents to discover, negotiate, and trade heterogeneous resources (compute, LLM tokens, knowledge, tool access, research output) using atomic hash-lock swaps and on-chain escrow.

Built on [Agent Network Protocol](https://github.com/agent-network-protocol/AgentNetworkProtocol) (identity, discovery, communication) and designed for integration with [OpenFang](https://github.com/openfang-project/openfang) (agent runtime, capabilities, execution).

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    FAX Protocol Stack                   │
├─────────────────────────────────────────────────────────┤
│  Discovery (ANP ADP)  →  Negotiation  →  Atomic Swap   │
├─────────────────────────────────────────────────────────┤
│              VC Hash Chain (credential links)            │
├─────────────────────────────────────────────────────────┤
│  FAXAnchor (L2)  │  FAXEscrow (L2)  │  Reputation    │
├─────────────────────────────────────────────────────────┤
│  did:wba Identity  │  E2EE (HPKE)  │  Ed25519/secp256k1│
└─────────────────────────────────────────────────────────┘
```

**Key principle:** Blockchain as anchor, not bus. Agents communicate off-chain via ANP. Only hashes, escrow state, and reputation go on-chain.

## Components

### Smart Contracts (`contracts/`)

Three Solidity contracts for EVM L2 (Arbitrum, Base, Optimism):

| Contract | Purpose |
|----------|---------|
| `FAXAnchor` | Agents publish VC chain tip hashes. Makes credential history immutable after anchoring. ~21K gas per anchor. |
| `FAXEscrow` | Hash-lock escrow for high-value trades. Manages lock → deliver → complete lifecycle with dispute resolution. |
| `FAXReputation` | On-chain trade history and reliability scores (0-1000). Publicly queryable before entering trades. |

### Rust Crates (`crates/`)

| Crate | Purpose |
|-------|---------|
| `fax-types` | Core types: resources, credentials, identity (DID + Ed25519), RCU oracle |
| `fax-protocol` | Trading protocol: hash-lock swap engine, trade lifecycle, security negotiation |
| `fax-chain` | L2 interaction: anchoring, escrow management, reputation queries |
| **`fax-anp`** | **ANP integration: agent description builder, discovery client, meta-protocol negotiation, DID:WBA bridge, WebSocket transport framing** |
| **`fax-openfang`** | **OpenFang integration: 9 FAX tools, capability system, Merkle audit log, config, FaxAgent runtime, A2A agent card** |
| `fax-cli` | CLI tool: identity generation, trade demo, rate display, reputation simulation |

### ANP Integration (`anp/`)

| File | Purpose |
|------|---------|
| `fax-interface.json` | FAX protocol interface description (like AP2's ap2.json) — roles, endpoints, schemas, WebSocket message types |

### OpenFang Integration (`openfang/`)

| File | Purpose |
|------|---------|
| `HAND.toml` | FAX Trader Hand — autonomous trading agent with 12 tools, LLM-driven strategy, dashboard metrics |
| `config-snippet.toml` | Config section to add to `~/.openfang/config.toml` |
| `integration-patch.md` | Step-by-step guide: every file in OpenFang that needs changes to wire in FAX |

### Schemas (`schemas/`)

JSON-LD schemas extending ANP's Agent Description Protocol:

- `resource-profile.jsonld` — How agents describe tradable resources
- `discovery.jsonld` — Resource-aware agent discovery
- `credentials/` — VC templates for swap agreements, resource locks, etc.

## Quick Start

### Build

```bash
# Rust
cargo build

# Solidity (requires Foundry)
cd contracts && forge build
```

### Run the Demo

```bash
cargo run --bin fax -- demo
```

This simulates a complete trade: Alice trades 2 GPU-hours for 100K LLM tokens from Bob, with hash-lock atomic swap and L2 anchoring.

### CLI Commands

```bash
# Generate an agent identity
fax identity --domain compute-provider.io --name alpha

# Show RCU conversion rates
fax rates

# Run full trade simulation
fax demo

# Simulate reputation scores
fax reputation --trades 20 --disputes 1
```

### Run Tests

```bash
# Rust tests
cargo test

# Solidity tests (requires Foundry)
cd contracts && forge test -vvv
```

## Trade Flow

```
Agent A (compute)                Agent B (tokens)
     │                                │
     │── ResourceOfferCredential ──→ │  Offer 2 GPU-hrs for 100K tokens
     │←── SwapAgreementCredential ──│  Agree on terms + security level
     │                                │
     │── ResourceLockCredential ───→ │  Lock compute behind hash H_a
     │←── ResourceLockCredential ───│  Lock tokens behind hash H_b
     │                                │
     │── ResourceDeliveryCredential →│  Reveal secret S_a (SHA256(S_a)=H_a)
     │←── ResourceDeliveryCredential │  Reveal secret S_b (SHA256(S_b)=H_b)
     │                                │
     │── SwapCompletionCredential ──→│  Both confirm
     │                                │
     │── anchor(chain_tip_hash) ────→│  Publish to L2
```

Each credential links to the previous via `previousCredentialHash`, forming a tamper-evident chain anchored on-chain.

## Security Levels

Agents negotiate security per trade:

| Level | Name | Mechanism | Trade Value |
|-------|------|-----------|-------------|
| 0 | Trust | VC chain only | < 10 RCU |
| 1 | Anchor | + L2 hash anchor | 10-100 RCU |
| 2 | Escrow | + on-chain hash-lock | 100-1000 RCU |
| 3 | Full Escrow | + arbitration clause | > 1000 RCU |
| 4 | ZK Private | + selective disclosure | Privacy-sensitive |

## Resource Credit Unit (RCU)

Common denominator for comparing heterogeneous resources:

```
1 RCU ≈ cost of 1,000 LLM tokens on a mid-tier model

1 GPU-hour      = 50 RCU
100K LLM tokens = 100 RCU
1 research report = 200 RCU
1 knowledge query = 0.5 RCU
```

RCU is a negotiation tool, not a token. Agents can disagree on rates.

## ANP Integration

FAX plugs into every layer of the Agent Network Protocol:

| ANP Layer | FAX Integration |
|-----------|-----------------|
| **Agent Description (§7)** | `interfaces` entry with `protocol: "FAX"`, resource `Informations` with RCU rates |
| **Discovery (§8)** | `/.well-known/agent-descriptions` crawling filtered by FAX interface |
| **Meta-Protocol (§6)** | `candidateProtocols` includes `https://fax-network.org/protocol/1.0`; negotiates resource types + security level |
| **DID:WBA (§3)** | Ed25519 for VC signing, secp256k1-derived EVM address for on-chain ops, `FaxTradingEndpoint` service in DID doc |
| **E2EE (§5)** | Trade messages encrypted via HPKE over ANP WebSocket |
| **VC Hash Chain (§9)** | `previousCredentialHash` linking for tamper-evident trade history |

## OpenFang Integration

FAX extends OpenFang's runtime at every integration point:

| Component | Integration |
|-----------|------------|
| **Capabilities** | 8 new `Fax*` capability variants in the `Capability` enum |
| **Tools** | 9 FAX tools (`fax_discover`, `fax_create_offer`, `fax_lock_resource`, etc.) |
| **Audit Trail** | 10 FAX audit actions written to the same Merkle hash chain |
| **Metering** | RCU-based resource accounting integrated with EconSpend budget checks |
| **A2A** | FAX tools auto-exported as A2A skills in the Agent Card |
| **Hand** | `fax-trader` Hand with LLM-driven autonomous trading strategy |
| **Config** | `[fax]` section in `config.toml` for chain, trading, and discovery settings |

## License

MIT
