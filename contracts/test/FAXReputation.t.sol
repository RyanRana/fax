// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "forge-std/Test.sol";
import "../src/FAXReputation.sol";

contract FAXReputationTest is Test {
    FAXReputation public reputation;
    address public escrow = address(0xE5);
    address public agent1 = address(0xA1);
    address public agent2 = address(0xA2);

    function setUp() public {
        reputation = new FAXReputation();
        reputation.setEscrowContract(escrow);
    }

    function test_register() public {
        vm.prank(agent1);
        reputation.register();
        assertTrue(reputation.isRegistered(agent1));
    }

    function test_double_register_reverts() public {
        vm.prank(agent1);
        reputation.register();

        vm.expectRevert("FAX: already registered");
        vm.prank(agent1);
        reputation.register();
    }

    function test_new_agent_base_score() public {
        vm.prank(agent1);
        reputation.register();
        uint256 score = reputation.getReliabilityScore(agent1);
        assertEq(score, 100);
    }

    function test_score_after_trades() public {
        // Record 10 successful trades
        vm.startPrank(escrow);
        for (uint256 i = 0; i < 10; i++) {
            reputation.recordTradeCompletion(agent1, agent2, 50, false);
        }
        vm.stopPrank();

        uint256 score1 = reputation.getReliabilityScore(agent1);
        // 10/10 completion = 700, 0 disputes = 200, minimal longevity
        assertGe(score1, 900);
    }

    function test_score_with_disputes() public {
        vm.startPrank(escrow);
        // 8 clean trades + 2 disputed
        for (uint256 i = 0; i < 8; i++) {
            reputation.recordTradeCompletion(agent1, agent2, 50, false);
        }
        reputation.recordTradeCompletion(agent1, agent2, 50, true);
        reputation.recordTradeCompletion(agent1, agent2, 50, true);
        reputation.recordDisputeLoss(agent1);
        vm.stopPrank();

        uint256 score = reputation.getReliabilityScore(agent1);
        // Should be noticeably lower than perfect
        assertLt(score, 900);
        assertGt(score, 500);
    }

    function test_only_escrow_can_record() public {
        vm.prank(agent1);
        reputation.register();

        vm.expectRevert("FAX: not escrow");
        vm.prank(agent1);
        reputation.recordTradeCompletion(agent1, agent2, 50, false);
    }

    function test_total_rcu_tracked() public {
        vm.startPrank(escrow);
        reputation.recordTradeCompletion(agent1, agent2, 100, false);
        reputation.recordTradeCompletion(agent1, agent2, 200, false);
        vm.stopPrank();

        IFAXReputation.AgentReputation memory r = reputation.getReputation(agent1);
        assertEq(r.totalRcuTraded, 300);
        assertEq(r.totalTrades, 2);
        assertEq(r.successfulTrades, 2);
    }

    function test_unregistered_score_is_zero() public {
        uint256 score = reputation.getReliabilityScore(address(0xDEAD));
        assertEq(score, 0);
    }
}
