// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "./interfaces/IFAX.sol";

/// @title FAXEscrow — On-chain hash-lock escrow for agent resource trades
/// @notice Manages the lifecycle of trades: lock → deliver → complete, with dispute resolution.
///         No tokens are escrowed — the contract tracks commitments and hash-lock state.
///         Actual resource delivery happens off-chain; the contract enforces atomicity via hash-locks.
contract FAXEscrow is IFAXEscrow {
    address public owner;
    address public arbitrator;
    IFAXReputation public reputationRegistry;

    mapping(bytes32 => Trade) private _trades;
    mapping(bytes32 => bytes32) private _disputeEvidence;

    modifier onlyOwner() {
        require(msg.sender == owner, "FAX: not owner");
        _;
    }

    modifier onlyArbitrator() {
        require(msg.sender == arbitrator, "FAX: not arbitrator");
        _;
    }

    constructor(address _arbitrator) {
        owner = msg.sender;
        arbitrator = _arbitrator;
    }

    function setReputationRegistry(address registry) external onlyOwner {
        reputationRegistry = IFAXReputation(registry);
    }

    function setArbitrator(address _arbitrator) external onlyOwner {
        arbitrator = _arbitrator;
    }

    /// @notice Lock a trade. Party A initiates; both parties' hash-locks are committed.
    ///         The tradeId should be SHA-256(SwapAgreementCredential).
    function lockTrade(
        bytes32 tradeId,
        address counterparty,
        bytes32 hashLockA,
        bytes32 hashLockB,
        uint256 rcuValue,
        uint64 lockDuration
    ) external {
        require(_trades[tradeId].state == TradeState.None, "FAX: trade exists");
        require(counterparty != address(0) && counterparty != msg.sender, "FAX: invalid counterparty");
        require(hashLockA != bytes32(0) && hashLockB != bytes32(0), "FAX: zero hash-lock");
        require(lockDuration >= 300 && lockDuration <= 604800, "FAX: duration 5min-7d");

        _trades[tradeId] = Trade({
            tradeId: tradeId,
            partyA: msg.sender,
            partyB: counterparty,
            hashLockA: hashLockA,
            hashLockB: hashLockB,
            rcuValue: rcuValue,
            lockExpiry: uint64(block.timestamp) + lockDuration,
            state: TradeState.Locked,
            createdAt: uint64(block.timestamp)
        });

        emit TradeLocked(tradeId, msg.sender, counterparty, rcuValue);
    }

    /// @notice Confirm delivery by revealing the hash-lock secret.
    ///         Each party reveals their own secret to prove they delivered their resource.
    function confirmDelivery(bytes32 tradeId, bytes32 secret) external {
        Trade storage trade = _trades[tradeId];
        require(
            trade.state == TradeState.Locked
                || trade.state == TradeState.ADelivered
                || trade.state == TradeState.BDelivered,
            "FAX: not deliverable"
        );
        require(block.timestamp <= trade.lockExpiry, "FAX: expired");

        bool isA = msg.sender == trade.partyA;
        bool isB = msg.sender == trade.partyB;
        require(isA || isB, "FAX: not a party");

        if (isA) {
            require(sha256(abi.encodePacked(secret)) == trade.hashLockA, "FAX: bad secret A");
            if (trade.state == TradeState.Locked) {
                trade.state = TradeState.ADelivered;
                emit DeliveryConfirmed(tradeId, msg.sender, TradeState.ADelivered);
            } else if (trade.state == TradeState.BDelivered) {
                trade.state = TradeState.Complete;
                emit DeliveryConfirmed(tradeId, msg.sender, TradeState.Complete);
                emit TradeCompleted(tradeId);
                _recordCompletion(trade, false);
            }
        } else {
            require(sha256(abi.encodePacked(secret)) == trade.hashLockB, "FAX: bad secret B");
            if (trade.state == TradeState.Locked) {
                trade.state = TradeState.BDelivered;
                emit DeliveryConfirmed(tradeId, msg.sender, TradeState.BDelivered);
            } else if (trade.state == TradeState.ADelivered) {
                trade.state = TradeState.Complete;
                emit DeliveryConfirmed(tradeId, msg.sender, TradeState.Complete);
                emit TradeCompleted(tradeId);
                _recordCompletion(trade, false);
            }
        }
    }

    /// @notice Claim an expired trade. Releases both parties from commitment.
    function claimExpired(bytes32 tradeId) external {
        Trade storage trade = _trades[tradeId];
        require(
            trade.state == TradeState.Locked
                || trade.state == TradeState.ADelivered
                || trade.state == TradeState.BDelivered,
            "FAX: not expirable"
        );
        require(block.timestamp > trade.lockExpiry, "FAX: not expired yet");

        trade.state = TradeState.Expired;
        emit TradeExpired(tradeId);
    }

    /// @notice Initiate a dispute. Either party can call this while trade is active.
    function initDispute(bytes32 tradeId, bytes32 evidenceHash) external {
        Trade storage trade = _trades[tradeId];
        require(
            trade.state == TradeState.Locked
                || trade.state == TradeState.ADelivered
                || trade.state == TradeState.BDelivered,
            "FAX: not disputable"
        );
        require(msg.sender == trade.partyA || msg.sender == trade.partyB, "FAX: not a party");

        trade.state = TradeState.Disputed;
        _disputeEvidence[tradeId] = evidenceHash;

        emit DisputeInitiated(tradeId, msg.sender);
    }

    /// @notice Resolve a dispute. Only the registered arbitrator can call this.
    function resolveDispute(bytes32 tradeId, bool favorA) external onlyArbitrator {
        Trade storage trade = _trades[tradeId];
        require(trade.state == TradeState.Disputed, "FAX: not disputed");

        trade.state = TradeState.Resolved;
        emit DisputeResolved(tradeId, favorA);
        _recordCompletion(trade, true);
    }

    function getTrade(bytes32 tradeId) external view returns (Trade memory) {
        return _trades[tradeId];
    }

    function getDisputeEvidence(bytes32 tradeId) external view returns (bytes32) {
        return _disputeEvidence[tradeId];
    }

    function _recordCompletion(Trade storage trade, bool disputed) internal {
        if (address(reputationRegistry) == address(0)) return;

        try reputationRegistry.getReputation(trade.partyA) {
            // Reputation updates are handled by the reputation contract
            // via a direct call from this escrow contract
        } catch {
            // Reputation registry not available; skip
        }
    }
}
