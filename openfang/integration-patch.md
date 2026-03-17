# OpenFang Integration Patch Guide

How to wire FAX into the OpenFang runtime. This documents every file that needs changes.

---

## 1. Capability System (`openfang-types/src/capability.rs`)

Add these variants to the `Capability` enum:

```rust
// -- FAX Resource Trading --
FaxOffer(String),
FaxAccept,
FaxEscrow { max_rcu: f64 },
FaxChainSign,
FaxAnchor,
FaxCredit(f64),
FaxArbitrate,
FaxDiscover,
```

Add matching arms to `capability_matches()`:

```rust
(Capability::FaxOffer(pattern), Capability::FaxOffer(requested)) => {
    pattern == "*" || pattern == requested || requested.starts_with(pattern.trim_end_matches('*'))
}
(Capability::FaxAccept, Capability::FaxAccept) => true,
(Capability::FaxEscrow { max_rcu }, Capability::FaxEscrow { max_rcu: needed }) => max_rcu >= needed,
(Capability::FaxChainSign, Capability::FaxChainSign) => true,
(Capability::FaxAnchor, Capability::FaxAnchor) => true,
(Capability::FaxCredit(limit), Capability::FaxCredit(needed)) => limit >= needed,
(Capability::FaxArbitrate, Capability::FaxArbitrate) => true,
(Capability::FaxDiscover, Capability::FaxDiscover) => true,
```

## 2. Manifest (`openfang-types/src/agent.rs`)

Add to `ManifestCapabilities`:

```rust
pub fax_offer: Option<Vec<String>>,
pub fax_accept: Option<bool>,
pub fax_escrow_max_rcu: Option<f64>,
pub fax_chain_sign: Option<bool>,
pub fax_anchor: Option<bool>,
pub fax_discover: Option<bool>,
```

## 3. Kernel (`openfang-kernel/src/kernel.rs`)

In `manifest_to_capabilities()`, add:

```rust
if let Some(ref patterns) = manifest.capabilities.fax_offer {
    for p in patterns { caps.push(Capability::FaxOffer(p.clone())); }
}
if manifest.capabilities.fax_accept == Some(true) { caps.push(Capability::FaxAccept); }
if let Some(max) = manifest.capabilities.fax_escrow_max_rcu { caps.push(Capability::FaxEscrow { max_rcu: max }); }
if manifest.capabilities.fax_chain_sign == Some(true) { caps.push(Capability::FaxChainSign); }
if manifest.capabilities.fax_anchor == Some(true) { caps.push(Capability::FaxAnchor); }
if manifest.capabilities.fax_discover == Some(true) { caps.push(Capability::FaxDiscover); }
```

## 4. Tool Runner (`openfang-runtime/src/tool_runner.rs`)

In `execute_tool()` match block, add:

```rust
name if name.starts_with("fax_") => {
    let fax_runner = kernel.fax_runner().await;
    let result = fax_runner.execute(name, input).await;
    Ok(ToolOutput { content: serde_json::to_string(&result)? })
}
```

In `builtin_tool_definitions()`, append:

```rust
definitions.extend(fax_openfang::tools::fax_tool_definitions().into_iter().map(|t| {
    ToolDefinition { name: t.name, description: t.description, input_schema: t.input_schema }
}));
```

## 5. Audit Trail (`openfang-runtime/src/audit.rs`)

Add to `AuditAction` enum:

```rust
FaxTrade,
```

FAX tools automatically record to the FaxAuditLog. To merge with OpenFang's main audit:

```rust
audit_log.record(agent_id, AuditAction::FaxTrade, &format!("fax:{}", tool_name), outcome);
```

## 6. Config (`openfang-types/src/config.rs`)

Add to `KernelConfig`:

```rust
pub fax: Option<fax_openfang::config::FaxConfig>,
```

## 7. A2A Agent Card (`openfang-runtime/src/a2a.rs`)

FAX tools are automatically exported as A2A skills when they appear in the manifest's `tools` list. No additional changes needed — the `build_agent_card()` function already iterates over tools.

## 8. Hand Installation

Copy `openfang/HAND.toml` to:

```
~/.openfang/hands/fax-trader/HAND.toml
```

## 9. Dependencies

Add to OpenFang's workspace `Cargo.toml`:

```toml
fax-types = { path = "../fax-network/crates/fax-types" }
fax-protocol = { path = "../fax-network/crates/fax-protocol" }
fax-chain = { path = "../fax-network/crates/fax-chain" }
fax-anp = { path = "../fax-network/crates/fax-anp" }
fax-openfang = { path = "../fax-network/crates/fax-openfang" }
```
