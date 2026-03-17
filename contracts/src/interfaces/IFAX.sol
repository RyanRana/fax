// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

interface IFAXAnchor {
    event ChainAnchored(
        address indexed agent,
        bytes32 indexed chainHash,
        uint64 sequenceNum,
        uint256 timestamp
    );

    function anchor(bytes32 chainHash) external;
    function anchorBatch(bytes32[] calldata chainHashes) external;
    function getLatestAnchor(address agent) external view returns (bytes32 hash, uint64 seq, uint256 ts);
    function getAnchorAt(address agent, uint64 sequenceNum) external view returns (bytes32 hash, uint256 ts);
    function getAnchorCount(address agent) external view returns (uint64);
    function verifyAnchorExisted(address agent, bytes32 chainHash) external view returns (bool existed, uint256 anchoredAt);
}

interface IFAXEscrow {
    enum TradeState {
        None,
        Locked,
        ADelivered,
        BDelivered,
        Complete,
        Expired,
        Disputed,
        Resolved
    }

    struct Trade {
        bytes32 tradeId;
        address partyA;
        address partyB;
        bytes32 hashLockA;
        bytes32 hashLockB;
        uint256 rcuValue;
        uint64 lockExpiry;
        TradeState state;
        uint64 createdAt;
    }

    event TradeLocked(bytes32 indexed tradeId, address indexed partyA, address indexed partyB, uint256 rcuValue);
    event DeliveryConfirmed(bytes32 indexed tradeId, address indexed party, TradeState newState);
    event TradeCompleted(bytes32 indexed tradeId);
    event TradeExpired(bytes32 indexed tradeId);
    event DisputeInitiated(bytes32 indexed tradeId, address indexed initiator);
    event DisputeResolved(bytes32 indexed tradeId, bool favorA);

    function lockTrade(
        bytes32 tradeId,
        address counterparty,
        bytes32 hashLockA,
        bytes32 hashLockB,
        uint256 rcuValue,
        uint64 lockDuration
    ) external;

    function confirmDelivery(bytes32 tradeId, bytes32 secret) external;
    function claimExpired(bytes32 tradeId) external;
    function initDispute(bytes32 tradeId, bytes32 evidenceHash) external;
    function resolveDispute(bytes32 tradeId, bool favorA) external;
    function getTrade(bytes32 tradeId) external view returns (Trade memory);
}

interface IFAXReputation {
    struct AgentReputation {
        uint64 totalTrades;
        uint64 successfulTrades;
        uint64 disputesInitiated;
        uint64 disputesLost;
        uint256 totalRcuTraded;
        uint64 lastTradeBlock;
        uint64 registeredBlock;
    }

    event ReputationUpdated(address indexed agent, uint64 totalTrades, uint64 successfulTrades);
    event AgentRegistered(address indexed agent, uint64 blockNumber);

    function register() external;
    function getReputation(address agent) external view returns (AgentReputation memory);
    function getReliabilityScore(address agent) external view returns (uint256 score);
    function isRegistered(address agent) external view returns (bool);
}
