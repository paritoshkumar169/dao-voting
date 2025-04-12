import * as anchor from '@project-serum/anchor';
import { assert } from 'chai';
import { PublicKey, SystemProgram } from '@solana/web3.js';
import { Buffer } from 'buffer';

globalThis.Buffer = Buffer;

describe('staking_voting_contract', () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.staking_voting_contract;
  const warpTime = async (seconds: number) => {
    try {
      await (provider.connection as any)._rpcRequest('increaseTime', [seconds]);
    } catch (error) {
      console.log('Time warp failed (ensure local test validator is running)', error);
    }
  };

  const createStakeAccount = async (user: anchor.web3.Keypair, amount: number) => {
    const stakeAccount = anchor.web3.Keypair.generate();
    await program.methods.stakeSol(new anchor.BN(amount))
      .accounts({
        stakeAccount: stakeAccount.publicKey,
        user: user.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([stakeAccount, user])
      .rpc();
    return stakeAccount;
  };

  it('Stakes SOL successfully', async () => {
    const user = anchor.web3.Keypair.generate();
    const stakeAccount = await createStakeAccount(user, 1e9);
    
    const account = await program.account.stakeAccount.fetch(stakeAccount.publicKey);
    assert.equal(account.stakedAmount.toString(), '1000000000');
    assert.isTrue(account.owner.equals(user.publicKey));
  });

  it('Fails to stake with insufficient SOL', async () => {
    try {
      const user = anchor.web3.Keypair.generate();
      await createStakeAccount(user, 0.9e9);
      assert.fail('Should have thrown error');
    } catch (err) {
      assert.include(err.message, 'InsufficientStake');
    }
  });

  it('Requests unstake successfully', async () => {
    const user = anchor.web3.Keypair.generate();
    const stakeAccount = await createStakeAccount(user, 1e9);

    await program.methods.requestUnstake()
      .accounts({
        stakeAccount: stakeAccount.publicKey,
        owner: user.publicKey,
      })
      .rpc();

    const account = await program.account.stakeAccount.fetch(stakeAccount.publicKey);
    assert.isTrue(account.unstakeRequested);
    assert.exists(account.unstakeTimestamp);
  });

  it('Fails to claim unstake before cooldown', async () => {
    const user = anchor.web3.Keypair.generate();
    const stakeAccount = await createStakeAccount(user, 1e9);
    
    await program.methods.requestUnstake()
      .accounts({ stakeAccount: stakeAccount.publicKey, owner: user.publicKey })
      .rpc();

    try {
      await program.methods.claimUnstake()
        .accounts({ stakeAccount: stakeAccount.publicKey, owner: user.publicKey })
        .rpc();
      assert.fail('Should have thrown error');
    } catch (err) {
      assert.include(err.message, 'CooldownNotPassed');
    }
  });

  it('Claims unstake after cooldown', async () => {
    const user = anchor.web3.Keypair.generate();
    const stakeAccount = await createStakeAccount(user, 1e9);

    await program.methods.requestUnstake()
      .accounts({ stakeAccount: stakeAccount.publicKey, owner: user.publicKey })
      .rpc();

    await warpTime(5 * 24 * 3600 + 1);

    await program.methods.claimUnstake()
      .accounts({ stakeAccount: stakeAccount.publicKey, owner: user.publicKey })
      .rpc();

    const account = await program.account.stakeAccount.fetch(stakeAccount.publicKey);
    assert.equal(account.stakedAmount.toString(), '0');
    assert.isFalse(account.unstakeRequested);
  });

  it('Creates proposal successfully', async () => {
    const user = anchor.web3.Keypair.generate();
    const stakeAccount = await createStakeAccount(user, 1e9);
    const proposalAccount = anchor.web3.Keypair.generate();

    await program.methods.initializeProposal("https://example.com/proposal/1")
      .accounts({
        proposal: proposalAccount.publicKey,
        stakeAccount: stakeAccount.publicKey,
        user: user.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([proposalAccount])
      .rpc();

    const proposal = await program.account.proposal.fetch(proposalAccount.publicKey);
    assert.equal(proposal.metadataUri, "https://example.com/proposal/1");
    assert.equal(proposal.status.active, true);
  });

  it('Casts vote successfully', async () => {
    const user = anchor.web3.Keypair.generate();
    const stakeAccount = await createStakeAccount(user, 2e9);
    const proposalAccount = anchor.web3.Keypair.generate();

    await program.methods.initializeProposal("test")
      .accounts({ 
        proposal: proposalAccount.publicKey, 
        stakeAccount: stakeAccount.publicKey,
        user: user.publicKey,
        systemProgram: SystemProgram.programId
      })
      .signers([proposalAccount])
      .rpc();

    const [voteRecord] = await PublicKey.findProgramAddress(
      [Buffer.from('vote'), proposalAccount.publicKey.toBuffer(), user.publicKey.toBuffer()],
      program.programId
    );

    await program.methods.castVote({ yes: {} })
      .accounts({
        proposal: proposalAccount.publicKey,
        voteRecord,
        stakeAccount: stakeAccount.publicKey,
        user: user.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const proposal = await program.account.proposal.fetch(proposalAccount.publicKey);
    assert.equal(proposal.yesVotes.toString(), '2000000000');
    

    const vote = await program.account.voteRecord.fetch(voteRecord);
    assert.equal(vote.voteWeight.toString(), '2000000000');
  });

  it('Finalizes proposal after end time', async () => {
    const user = anchor.web3.Keypair.generate();
    const stakeAccount = await createStakeAccount(user, 1e9);
    const proposalAccount = anchor.web3.Keypair.generate();

    await program.methods.initializeProposal("test")
      .accounts({ 
        proposal: proposalAccount.publicKey, 
        stakeAccount: stakeAccount.publicKey,
        user: user.publicKey,
        systemProgram: SystemProgram.programId
      })
      .signers([proposalAccount])
      .rpc();

   
    const proposal = await program.account.proposal.fetch(proposalAccount.publicKey);
    await warpTime(proposal.endTime.toNumber() - Math.floor(Date.now() / 1000) + 1);

    await program.methods.finalizeProposal()
      .accounts({ proposal: proposalAccount.publicKey })
      .rpc();

    const updatedProposal = await program.account.proposal.fetch(proposalAccount.publicKey);
    assert.equal(updatedProposal.status.finalized, true);
  });
});