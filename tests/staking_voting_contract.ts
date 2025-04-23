import * as anchor from "@coral-xyz/anchor";
import { Program, BN, web3 } from "@coral-xyz/anchor";
import { assert } from "chai";

const { SystemProgram } = web3;

describe("staking_voting_contract", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
 
  const program = anchor.workspace.StakingVotingContract as Program<any>;
  const wallet = provider.wallet;

  let stakeAccountKeypair: web3.Keypair;
  let stakeAccountPubkey: web3.PublicKey;
  let proposalAccount: web3.Keypair;
  let voteRecordPda: web3.PublicKey;

  it("Stakes SOL successfully", async () => {
    stakeAccountKeypair = web3.Keypair.generate();
    stakeAccountPubkey = stakeAccountKeypair.publicKey;
    const stakeAmount = new BN(1_000_000_000);

    await program.rpc.stakeSol(stakeAmount, {
      accounts: {
        stakeAccount: stakeAccountPubkey,
        user: wallet.publicKey,
        systemProgram: SystemProgram.programId,
      },
      signers: [stakeAccountKeypair],
    });

    const stakeAccount = await (program.account as any)["stakeAccount"].fetch(stakeAccountPubkey);
    assert.ok(new BN(stakeAccount.stakedAmount).eq(stakeAmount), "Stake amount should match");
  });

  it("Requests unstake successfully", async () => {
    await program.rpc.requestUnstake({
      accounts: {
        stakeAccount: stakeAccountPubkey,
        owner: wallet.publicKey,
      },
    });

    const stakeAccount = await (program.account as any)["stakeAccount"].fetch(stakeAccountPubkey);
    assert.ok(stakeAccount.unstakeRequested, "The unstakeRequested flag should be true");
    assert.ok(stakeAccount.unstakeTimestamp !== null, "An unstakeTimestamp should be set");
  });

  it("Fails to claim unstake before cooldown", async () => {
    try {
      await program.rpc.claimUnstake({
        accounts: {
          stakeAccount: stakeAccountPubkey,
          owner: wallet.publicKey,
        },
      });
      assert.fail("Claim unstake should have failed due to cooldown not passed");
    } catch (err) {
      assert.ok(err.toString().includes("Unstake cooldown period not passed"), "Expected cooldown error");
    }
  });

  it("Claims unstake after cooldown", async () => {
    await new Promise((resolve) => setTimeout(resolve, 20000));

    await program.rpc.claimUnstake({
      accounts: {
        stakeAccount: stakeAccountPubkey,
        owner: wallet.publicKey,
      },
    });

    const stakeAccount = await (program.account as any)["stakeAccount"].fetch(stakeAccountPubkey);
    assert.ok(new BN(stakeAccount.stakedAmount).eq(new BN(0)), "Staked amount should be zero after claim");
    assert.ok(!stakeAccount.unstakeRequested, "Unstake flag should be cleared");
  });

  it("Creates proposal successfully", async () => {
    stakeAccountKeypair = web3.Keypair.generate();
    stakeAccountPubkey = stakeAccountKeypair.publicKey;
    const stakeAmount = new BN(1_000_000_000);
    await program.rpc.stakeSol(stakeAmount, {
      accounts: {
        stakeAccount: stakeAccountPubkey,
        user: wallet.publicKey,
        systemProgram: SystemProgram.programId,
      },
      signers: [stakeAccountKeypair],
    });

    proposalAccount = web3.Keypair.generate();
    const metadataUri = "https://example.com/proposal.json";

    await program.rpc.initializeProposal(metadataUri, {
      accounts: {
        proposal: proposalAccount.publicKey,
        stakeAccount: stakeAccountPubkey,
        owner: wallet.publicKey,
        user: wallet.publicKey,
        systemProgram: SystemProgram.programId,
      },
      signers: [proposalAccount],
    });

    const proposal = await (program.account as any)["proposal"].fetch(proposalAccount.publicKey);
    assert.ok(proposal.metadataUri === metadataUri, "Proposal metadata should match");
  });

  it("Casts vote successfully", async () => {
    [voteRecordPda] = await web3.PublicKey.findProgramAddress(
      [
        Buffer.from("vote"),
        proposalAccount.publicKey.toBuffer(),
        wallet.publicKey.toBuffer()
      ],
      program.programId
    );

    await program.rpc.castVote({ yes: {} }, {
      accounts: {
        proposal: proposalAccount.publicKey,
        voteRecord: voteRecordPda,
        stakeAccount: stakeAccountPubkey,
        owner: wallet.publicKey,
        user: wallet.publicKey,
        systemProgram: SystemProgram.programId,
      },
    });

    const voteRecord = await (program.account as any)["voteRecord"].fetch(voteRecordPda);
    assert.ok(voteRecord.voteChoice.yes !== undefined, "Vote should be recorded as Yes");
  });

  it("Finalizes proposal after end time", async () => {
    await new Promise((resolve) => setTimeout(resolve, 10000));

    await program.rpc.finalizeProposal({
      accounts: {
        proposal: proposalAccount.publicKey,
      },
    });

    const proposal = await (program.account as any)["proposal"].fetch(proposalAccount.publicKey);
    console.log("Fetched proposal.status:", proposal.status);
  
    assert.ok(
      proposal.status.finalized !== undefined,
      "Proposal should be finalized"
    );
  });
});
