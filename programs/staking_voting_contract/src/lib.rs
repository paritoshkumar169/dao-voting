use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock::Clock;

declare_id!("8vDcMPAPjXDCy7zgNmN9u3JNTWJAvBzuwt9Lhztub82Y");

#[program]
pub mod staking_voting_contract {
    use super::*;

    pub fn stake_sol(ctx: Context<StakeSol>, amount: u64) -> Result<()> {
        let stake_account = &mut ctx.accounts.stake_account;
        let clock = Clock::get().unwrap();
        if amount < 1_000_000_000 {
            return Err(ErrorCode::InsufficientStake.into());
        }
        stake_account.owner = *ctx.accounts.user.key;
        stake_account.staked_amount = stake_account
            .staked_amount
            .checked_add(amount)
            .ok_or(ErrorCode::MathOverflow)?;
        stake_account.stake_timestamp = clock.unix_timestamp;
        Ok(())
    }

    pub fn request_unstake(ctx: Context<RequestUnstake>) -> Result<()> {
        let stake_account = &mut ctx.accounts.stake_account;
        let clock = Clock::get().unwrap();
        stake_account.unstake_requested = true;
        stake_account.unstake_timestamp = Some(clock.unix_timestamp);
        Ok(())
    }

    pub fn claim_unstake(ctx: Context<ClaimUnstake>) -> Result<()> {
        let stake_account = &mut ctx.accounts.stake_account;
        let clock = Clock::get().unwrap();
        if let Some(unstake_ts) = stake_account.unstake_timestamp {
            if clock.unix_timestamp < unstake_ts + 5 * 24 * 3600 {
                return Err(ErrorCode::CooldownNotPassed.into());
            }
        } else {
            return Err(ErrorCode::NoUnstakeRequested.into());
        }
        stake_account.staked_amount = 0;
        stake_account.unstake_requested = false;
        stake_account.unstake_timestamp = None;
        Ok(())
    }

    pub fn initialize_proposal(
        ctx: Context<InitializeProposal>,
        metadata_uri: String,
    ) -> Result<()> {
        let proposal = &mut ctx.accounts.proposal;
        let clock = Clock::get().unwrap();
        if ctx.accounts.stake_account.staked_amount < 1_000_000_000 {
            return Err(ErrorCode::InsufficientStake.into());
        }
        proposal.creator = *ctx.accounts.user.key;
        proposal.metadata_uri = metadata_uri;
        proposal.start_time = clock.unix_timestamp;
        proposal.end_time = clock.unix_timestamp + 3 * 24 * 3600;
        proposal.yes_votes = 0;
        proposal.no_votes = 0;
        proposal.status = ProposalStatus::Active;
        Ok(())
    }

    pub fn cast_vote(ctx: Context<CastVote>, vote_choice: VoteChoice) -> Result<()> {
        let proposal = &mut ctx.accounts.proposal;
        let clock = Clock::get().unwrap();
        if clock.unix_timestamp > proposal.end_time {
            return Err(ErrorCode::VotingPeriodEnded.into());
        }

        let stake_weight = ctx.accounts.stake_account.staked_amount;
        let vote_record = &mut ctx.accounts.vote_record;
        vote_record.voter = *ctx.accounts.user.key;
        vote_record.proposal = proposal.key();
        vote_record.vote_choice = vote_choice.clone();
        vote_record.vote_weight = stake_weight;

        match vote_choice {
            VoteChoice::Yes => {
                proposal.yes_votes = proposal
                    .yes_votes
                    .checked_add(stake_weight)
                    .ok_or(ErrorCode::MathOverflow)?;
            }
            VoteChoice::No => {
                proposal.no_votes = proposal
                    .no_votes
                    .checked_add(stake_weight)
                    .ok_or(ErrorCode::MathOverflow)?;
            }
        }

        Ok(())
    }

    pub fn finalize_proposal(ctx: Context<FinalizeProposal>) -> Result<()> {
        let proposal = &mut ctx.accounts.proposal;
        let clock = Clock::get().unwrap();
        if clock.unix_timestamp < proposal.end_time {
            return Err(ErrorCode::VotingPeriodNotEnded.into());
        }
        proposal.status = ProposalStatus::Finalized;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct StakeSol<'info> {
    #[account(init_if_needed, payer = user, space = 8 + 66)]
    pub stake_account: Account<'info, StakeAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RequestUnstake<'info> {
    #[account(mut, has_one = owner)]
    pub stake_account: Account<'info, StakeAccount>,
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct ClaimUnstake<'info> {
    #[account(mut, has_one = owner)]
    pub stake_account: Account<'info, StakeAccount>,
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct InitializeProposal<'info> {
    #[account(init, payer = user, space = 8 + 256)]
    pub proposal: Account<'info, Proposal>,
    #[account(mut, has_one = owner)]
    pub stake_account: Account<'info, StakeAccount>,
    pub owner: Signer<'info>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CastVote<'info> {
    #[account(mut)]
    pub proposal: Account<'info, Proposal>,
    #[account(
        init,
        payer = user,
        space = 8 + 80,
        seeds = [b"vote", proposal.key().as_ref(), user.key().as_ref()],
        bump
    )]
    pub vote_record: Account<'info, VoteRecord>,
    #[account(mut, has_one = owner)]
    pub stake_account: Account<'info, StakeAccount>,
    pub owner: Signer<'info>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct FinalizeProposal<'info> {
    #[account(mut)]
    pub proposal: Account<'info, Proposal>,
}

#[account]
pub struct StakeAccount {
    pub owner: Pubkey,
    pub staked_amount: u64,
    pub stake_timestamp: i64,
    pub unstake_requested: bool,
    pub unstake_timestamp: Option<i64>,
}

#[account]
pub struct Proposal {
    pub creator: Pubkey,
    pub metadata_uri: String,
    pub start_time: i64,
    pub end_time: i64,
    pub yes_votes: u64,
    pub no_votes: u64,
    pub status: ProposalStatus,
}

#[account]
pub struct VoteRecord {
    pub voter: Pubkey,
    pub proposal: Pubkey,
    pub vote_choice: VoteChoice,
    pub vote_weight: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum ProposalStatus {
    Active,
    Finalized,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum VoteChoice {
    Yes,
    No,
}

#[error_code]
pub enum ErrorCode {
    #[msg("You must stake at least 1 SOL to perform this action.")]
    InsufficientStake,
    #[msg("Math overflow occurred.")]
    MathOverflow,
    #[msg("Unstake cooldown period not passed.")]
    CooldownNotPassed,
    #[msg("No unstake request found.")]
    NoUnstakeRequested,
    #[msg("Voting period has ended.")]
    VotingPeriodEnded,
    #[msg("Voting period has not ended yet.")]
    VotingPeriodNotEnded,
}
