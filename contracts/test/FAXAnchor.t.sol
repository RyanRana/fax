// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "forge-std/Test.sol";
import "../src/FAXAnchor.sol";

contract FAXAnchorTest is Test {
    FAXAnchor public anchor;
    address public agent = address(0xA1);

    function setUp() public {
        anchor = new FAXAnchor();
    }

    function test_anchor_single() public {
        bytes32 hash = sha256("vc-chain-tip-1");
        vm.prank(agent);
        anchor.anchor(hash);

        (bytes32 h, uint64 seq, uint256 ts) = anchor.getLatestAnchor(agent);
        assertEq(h, hash);
        assertEq(seq, 0);
        assertGt(ts, 0);
        assertEq(anchor.getAnchorCount(agent), 1);
    }

    function test_anchor_sequence() public {
        bytes32 h1 = sha256("chain-1");
        bytes32 h2 = sha256("chain-2");
        bytes32 h3 = sha256("chain-3");

        vm.startPrank(agent);
        anchor.anchor(h1);
        anchor.anchor(h2);
        anchor.anchor(h3);
        vm.stopPrank();

        assertEq(anchor.getAnchorCount(agent), 3);
        (bytes32 latest,,) = anchor.getLatestAnchor(agent);
        assertEq(latest, h3);

        (bytes32 first,) = anchor.getAnchorAt(agent, 0);
        assertEq(first, h1);
    }

    function test_anchor_batch() public {
        bytes32[] memory hashes = new bytes32[](3);
        hashes[0] = sha256("batch-1");
        hashes[1] = sha256("batch-2");
        hashes[2] = sha256("batch-3");

        vm.prank(agent);
        anchor.anchorBatch(hashes);

        assertEq(anchor.getAnchorCount(agent), 3);
    }

    function test_verify_anchor_existed() public {
        bytes32 hash = sha256("verify-me");
        vm.prank(agent);
        anchor.anchor(hash);

        (bool existed, uint256 anchoredAt) = anchor.verifyAnchorExisted(agent, hash);
        assertTrue(existed);
        assertGt(anchoredAt, 0);

        (bool notExist,) = anchor.verifyAnchorExisted(agent, sha256("unknown"));
        assertFalse(notExist);
    }

    function test_revert_zero_hash() public {
        vm.expectRevert("FAX: zero hash");
        vm.prank(agent);
        anchor.anchor(bytes32(0));
    }

    function test_revert_empty_batch() public {
        bytes32[] memory empty = new bytes32[](0);
        vm.expectRevert("FAX: batch 1-64");
        vm.prank(agent);
        anchor.anchorBatch(empty);
    }

    function test_separate_agents_independent() public {
        address agent2 = address(0xA2);
        bytes32 h1 = sha256("agent1");
        bytes32 h2 = sha256("agent2");

        vm.prank(agent);
        anchor.anchor(h1);
        vm.prank(agent2);
        anchor.anchor(h2);

        assertEq(anchor.getAnchorCount(agent), 1);
        assertEq(anchor.getAnchorCount(agent2), 1);

        (bytes32 a1Latest,,) = anchor.getLatestAnchor(agent);
        (bytes32 a2Latest,,) = anchor.getLatestAnchor(agent2);
        assertEq(a1Latest, h1);
        assertEq(a2Latest, h2);
    }
}
