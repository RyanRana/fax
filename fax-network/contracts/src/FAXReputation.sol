// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "./interfaces/IFAX.sol";

/// @title FAXReputation — On-chain agent reputation registry
/// @notice Tracks trade completion rates, dispute history, and computes reliability scores.
///         Scores are publicly queryable so agents can assess counterparty risk before trading.
contract FAXReputation is IFAXReputation {
    address public owner;
    address public escrowContract;

    mapping(address => AgentReputation) private _reputations;
    mapping(address => bool) private _registered;

    modifier onlyOwner() {
        require(msg.sender == owner, "FAX: not owner");
        _;
    }

    modifier onlyEscrow() {
        require(msg.sender == escrowContract, "FAX: not escrow");
        _;
    }

    constructor() {
        owner = msg.sender;
    }

    function setEscrowContract(address _escrow) external onlyOwner {
        escrowContract = _escrow;
    }

    /// @notice Register an agent in the reputation system.
    function register() external {
        require(!_registered[msg.sender], "FAX: already registered");
        _registered[msg.sender] = true;
        _reputations[msg.sender].registeredBlock = uint64(block.number);
        emit AgentRegistered(msg.sender, uint64(block.number));
    }

    /// @notice Record a completed trade. Only callable by the escrow contract.
    function recordTradeCompletion(
        address partyA,
        address partyB,
        uint256 rcuValue,
        bool disputed
    ) external onlyEscrow {
        _updateAgent(partyA, rcuValue, disputed);
        _updateAgent(partyB, rcuValue, disputed);
    }

    /// @notice Record that an agent lost a dispute. Only callable by escrow.
    function recordDisputeLoss(address agent) external onlyEscrow {
        if (_registered[agent]) {
            _reputations[agent].disputesLost++;
        }
    }

    function getReputation(address agent) external view returns (AgentReputation memory) {
        require(_registered[agent], "FAX: not registered");
        return _reputations[agent];
    }

    /// @notice Compute a reliability score from 0 to 1000 (basis points).
    ///         70% completion rate + 20% dispute-free rate + 10% longevity.
    function getReliabilityScore(address agent) external view returns (uint256 score) {
        if (!_registered[agent]) return 0;
        AgentReputation memory r = _reputations[agent];
        if (r.totalTrades == 0) return 100; // new agent base score

        uint256 completionScore = (uint256(r.successfulTrades) * 700) / uint256(r.totalTrades);

        uint256 disputeScore;
        if (r.disputesLost == 0) {
            disputeScore = 200;
        } else {
            uint256 penalty = (uint256(r.disputesLost) * 200) / uint256(r.totalTrades);
            disputeScore = penalty >= 200 ? 0 : 200 - penalty;
        }

        uint256 age = block.number - uint256(r.registeredBlock);
        uint256 longevityScore = age >= 2_000_000 ? 100 : (age * 100) / 2_000_000;

        return completionScore + disputeScore + longevityScore;
    }

    function isRegistered(address agent) external view returns (bool) {
        return _registered[agent];
    }

    function _updateAgent(address agent, uint256 rcuValue, bool disputed) internal {
        if (!_registered[agent]) {
            _registered[agent] = true;
            _reputations[agent].registeredBlock = uint64(block.number);
            emit AgentRegistered(agent, uint64(block.number));
        }
        AgentReputation storage r = _reputations[agent];
        r.totalTrades++;
        if (!disputed) {
            r.successfulTrades++;
        }
        if (disputed) {
            r.disputesInitiated++;
        }
        r.totalRcuTraded += rcuValue;
        r.lastTradeBlock = uint64(block.number);

        emit ReputationUpdated(agent, r.totalTrades, r.successfulTrades);
    }
}
