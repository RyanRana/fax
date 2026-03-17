// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "forge-std/Test.sol";
import "../src/FAXEscrow.sol";
import "../src/FAXReputation.sol";

contract FAXEscrowTest is Test {
    FAXEscrow public escrow;
    FAXReputation public reputation;

    address public arbitrator = address(0xAA);
    address public alice = address(0xA1);
    address public bob = address(0xB2);

    bytes32 public secretA = bytes32("alice-secret-value-32bytes!!!!!");
    bytes32 public secretB = bytes32("bob-secret-value-32bytes!!!!!!!");
    bytes32 public hashLockA;
    bytes32 public hashLockB;
    bytes32 public tradeId = sha256("trade-001");

    function setUp() public {
        escrow = new FAXEscrow(arbitrator);
        reputation = new FAXReputation();
        escrow.setReputationRegistry(address(reputation));
        reputation.setEscrowContract(address(escrow));

        hashLockA = sha256(abi.encodePacked(secretA));
        hashLockB = sha256(abi.encodePacked(secretB));
    }

    function test_lock_trade() public {
        vm.prank(alice);
        escrow.lockTrade(tradeId, bob, hashLockA, hashLockB, 100, 3600);

        IFAXEscrow.Trade memory trade = escrow.getTrade(tradeId);
        assertEq(trade.partyA, alice);
        assertEq(trade.partyB, bob);
        assertEq(uint8(trade.state), uint8(IFAXEscrow.TradeState.Locked));
        assertEq(trade.rcuValue, 100);
    }

    function test_full_swap_lifecycle() public {
        vm.prank(alice);
        escrow.lockTrade(tradeId, bob, hashLockA, hashLockB, 100, 3600);

        // Alice reveals her secret (proves she delivered)
        vm.prank(alice);
        escrow.confirmDelivery(tradeId, secretA);
        assertEq(uint8(escrow.getTrade(tradeId).state), uint8(IFAXEscrow.TradeState.ADelivered));

        // Bob reveals his secret (proves he delivered)
        vm.prank(bob);
        escrow.confirmDelivery(tradeId, secretB);
        assertEq(uint8(escrow.getTrade(tradeId).state), uint8(IFAXEscrow.TradeState.Complete));
    }

    function test_reverse_delivery_order() public {
        vm.prank(alice);
        escrow.lockTrade(tradeId, bob, hashLockA, hashLockB, 50, 3600);

        // Bob delivers first
        vm.prank(bob);
        escrow.confirmDelivery(tradeId, secretB);
        assertEq(uint8(escrow.getTrade(tradeId).state), uint8(IFAXEscrow.TradeState.BDelivered));

        // Alice delivers second — completes the trade
        vm.prank(alice);
        escrow.confirmDelivery(tradeId, secretA);
        assertEq(uint8(escrow.getTrade(tradeId).state), uint8(IFAXEscrow.TradeState.Complete));
    }

    function test_wrong_secret_reverts() public {
        vm.prank(alice);
        escrow.lockTrade(tradeId, bob, hashLockA, hashLockB, 100, 3600);

        vm.expectRevert("FAX: bad secret A");
        vm.prank(alice);
        escrow.confirmDelivery(tradeId, bytes32("wrong-secret-xxxxxxxxxxxxxxx"));
    }

    function test_expiry() public {
        vm.prank(alice);
        escrow.lockTrade(tradeId, bob, hashLockA, hashLockB, 100, 600);

        // Cannot expire before time
        vm.expectRevert("FAX: not expired yet");
        escrow.claimExpired(tradeId);

        // Fast-forward past expiry
        vm.warp(block.timestamp + 601);
        escrow.claimExpired(tradeId);
        assertEq(uint8(escrow.getTrade(tradeId).state), uint8(IFAXEscrow.TradeState.Expired));
    }

    function test_dispute_and_resolution() public {
        vm.prank(alice);
        escrow.lockTrade(tradeId, bob, hashLockA, hashLockB, 100, 3600);

        // Alice initiates dispute
        vm.prank(alice);
        escrow.initDispute(tradeId, sha256("evidence-hash"));
        assertEq(uint8(escrow.getTrade(tradeId).state), uint8(IFAXEscrow.TradeState.Disputed));

        // Arbitrator resolves in favor of Alice
        vm.prank(arbitrator);
        escrow.resolveDispute(tradeId, true);
        assertEq(uint8(escrow.getTrade(tradeId).state), uint8(IFAXEscrow.TradeState.Resolved));
    }

    function test_non_arbitrator_cannot_resolve() public {
        vm.prank(alice);
        escrow.lockTrade(tradeId, bob, hashLockA, hashLockB, 100, 3600);

        vm.prank(alice);
        escrow.initDispute(tradeId, sha256("evidence"));

        vm.expectRevert("FAX: not arbitrator");
        vm.prank(alice);
        escrow.resolveDispute(tradeId, true);
    }

    function test_duplicate_trade_id_reverts() public {
        vm.prank(alice);
        escrow.lockTrade(tradeId, bob, hashLockA, hashLockB, 100, 3600);

        vm.expectRevert("FAX: trade exists");
        vm.prank(alice);
        escrow.lockTrade(tradeId, bob, hashLockA, hashLockB, 100, 3600);
    }

    function test_self_trade_reverts() public {
        vm.expectRevert("FAX: invalid counterparty");
        vm.prank(alice);
        escrow.lockTrade(tradeId, alice, hashLockA, hashLockB, 100, 3600);
    }
}
