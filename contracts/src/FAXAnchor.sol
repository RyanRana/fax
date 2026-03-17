// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "./interfaces/IFAX.sol";

/// @title FAXAnchor — VC hash chain anchoring on L2
/// @notice Agents publish SHA-256 hashes of their verifiable credential chain tips.
///         Once anchored, no agent can retroactively rewrite VC history before that block.
contract FAXAnchor is IFAXAnchor {
    struct AnchorEntry {
        bytes32 chainHash;
        uint256 timestamp;
    }

    mapping(address => AnchorEntry[]) private _anchors;
    mapping(address => mapping(bytes32 => uint256)) private _hashToTimestamp;

    /// @notice Anchor a single VC chain hash.
    function anchor(bytes32 chainHash) external {
        require(chainHash != bytes32(0), "FAX: zero hash");

        uint64 seq = uint64(_anchors[msg.sender].length);
        _anchors[msg.sender].push(AnchorEntry({chainHash: chainHash, timestamp: block.timestamp}));
        _hashToTimestamp[msg.sender][chainHash] = block.timestamp;

        emit ChainAnchored(msg.sender, chainHash, seq, block.timestamp);
    }

    /// @notice Anchor multiple hashes in one transaction (gas efficient for batch trades).
    function anchorBatch(bytes32[] calldata chainHashes) external {
        uint256 len = chainHashes.length;
        require(len > 0 && len <= 64, "FAX: batch 1-64");

        uint64 baseSeq = uint64(_anchors[msg.sender].length);
        for (uint256 i = 0; i < len; i++) {
            bytes32 h = chainHashes[i];
            require(h != bytes32(0), "FAX: zero hash in batch");
            _anchors[msg.sender].push(AnchorEntry({chainHash: h, timestamp: block.timestamp}));
            _hashToTimestamp[msg.sender][h] = block.timestamp;
            emit ChainAnchored(msg.sender, h, baseSeq + uint64(i), block.timestamp);
        }
    }

    function getLatestAnchor(address agent)
        external
        view
        returns (bytes32 hash, uint64 seq, uint256 ts)
    {
        uint256 len = _anchors[agent].length;
        require(len > 0, "FAX: no anchors");
        AnchorEntry storage entry = _anchors[agent][len - 1];
        return (entry.chainHash, uint64(len - 1), entry.timestamp);
    }

    function getAnchorAt(address agent, uint64 sequenceNum)
        external
        view
        returns (bytes32 hash, uint256 ts)
    {
        require(sequenceNum < _anchors[agent].length, "FAX: seq out of range");
        AnchorEntry storage entry = _anchors[agent][sequenceNum];
        return (entry.chainHash, entry.timestamp);
    }

    function getAnchorCount(address agent) external view returns (uint64) {
        return uint64(_anchors[agent].length);
    }

    function verifyAnchorExisted(address agent, bytes32 chainHash)
        external
        view
        returns (bool existed, uint256 anchoredAt)
    {
        uint256 ts = _hashToTimestamp[agent][chainHash];
        return (ts > 0, ts);
    }
}
