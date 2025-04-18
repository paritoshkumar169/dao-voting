use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock::Clock;

declare_id!("8vDcMPAPjXDCy7zgNmN9u3JNTWJAvBzuwt9Lhztub82Y");

const MINIMUM_STAKE: u64 = 1_000_000_000; // 1 SOL
const UNSTAKE_COOLDOWN: i64 = 300; // 5 mins

#[program]
pub mod solana_staking {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }

    pub fn stake(ctx: Context<Stake>, amount: u64) -> Result<()> {
        require!(amount >= MINIMUM_STAKE, StakingError::BelowMinimumStake);

        let clock = Clock::get()?;
        let user_stake = &mut ctx.accounts.user_stake;

        require!(user_stake.status == StakeStatus::Unstaked, StakingError::AlreadyStaked);

        // Validate vault PDA
        let (expected_vault, _) = Pubkey::find_program_address(
            &[b"vault", ctx.accounts.user.key.as_ref()],
            ctx.program_id,
        );
        require_keys_eq!(ctx.accounts.vault.key(), expected_vault, StakingError::InvalidVault);

        // Transfer SOL to vault
        let ix = anchor_lang::solana_program::system_instruction::transfer(
            ctx.accounts.user.key,
            ctx.accounts.vault.key,
            amount,
        );
        anchor_lang::solana_program::program::invoke(
            &ix,
            &[
                ctx.accounts.user.to_account_info(),
                ctx.accounts.vault.to_account_info(),
            ],
        )?;

        user_stake.amount = amount;
        user_stake.stake_time = clock.unix_timestamp;
        user_stake.status = StakeStatus::Staked;
        user_stake.cooldown_start = 0;

        Ok(())
    }

    pub fn start_unstake(ctx: Context<StartUnstake>) -> Result<()> {
        let clock = Clock::get()?;
        let user_stake = &mut ctx.accounts.user_stake;

        require!(user_stake.status == StakeStatus::Staked, StakingError::NotStaked);

        user_stake.status = StakeStatus::Cooldown;
        user_stake.cooldown_start = clock.unix_timestamp;

        Ok(())
    }

    pub fn claim_unstake(ctx: Context<ClaimUnstake>, vault_bump: u8) -> Result<()> {
        let clock = Clock::get()?;
        let user_stake = &mut ctx.accounts.user_stake;

        require!(user_stake.status == StakeStatus::Cooldown, StakingError::NotInCooldown);
        require!(
            clock.unix_timestamp >= user_stake.cooldown_start + UNSTAKE_COOLDOWN,
            StakingError::CooldownNotElapsed
        );

        // Validate vault PDA
        let (expected_vault, _) = Pubkey::find_program_address(
            &[b"vault", ctx.accounts.user.key.as_ref()],
            ctx.program_id,
        );
        require_keys_eq!(ctx.accounts.vault.key(), expected_vault, StakingError::InvalidVault);

        let seeds = &[b"vault", ctx.accounts.user.key.as_ref(), &[vault_bump]];
        let signer = &[&seeds[..]];

        let ix = anchor_lang::solana_program::system_instruction::transfer(
            ctx.accounts.vault.key,
            ctx.accounts.user.key,
            user_stake.amount,
        );

        anchor_lang::solana_program::program::invoke_signed(
            &ix,
            &[
                ctx.accounts.vault.to_account_info(),
                ctx.accounts.user.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            signer,
        )?;

        user_stake.amount = 0;
        user_stake.status = StakeStatus::Unstaked;
        user_stake.cooldown_start = 0;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [b"vault", user.key().as_ref()],
        bump,
    )]
    pub vault: SystemAccount<'info>,

    #[account(
        init_if_needed,
        payer = user,
        seeds = [b"user-stake", user.key().as_ref()],
        bump,
        space = 8 + std::mem::size_of::<UserStake>(),
    )]
    pub user_stake: Account<'info, UserStake>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct StartUnstake<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [b"vault", user.key().as_ref()],
        bump,
    )]
    pub vault: SystemAccount<'info>,

    #[account(
        mut,
        seeds = [b"user-stake", user.key().as_ref()],
        bump,
    )]
    pub user_stake: Account<'info, UserStake>,
}

#[derive(Accounts)]
pub struct ClaimUnstake<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [b"vault", user.key().as_ref()],
        bump,
    )]
    pub vault: SystemAccount<'info>,

    #[account(
        mut,
        seeds = [b"user-stake", user.key().as_ref()],
        bump,
    )]
    pub user_stake: Account<'info, UserStake>,

    pub system_program: Program<'info, System>,
}

#[account]
pub struct UserStake {
    pub amount: u64,
    pub stake_time: i64,
    pub cooldown_start: i64,
    pub status: StakeStatus,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum StakeStatus {
    Unstaked,
    Staked,
    Cooldown,
}

impl Default for StakeStatus {
    fn default() -> Self {
        StakeStatus::Unstaked
    }
}

#[error_code]
pub enum StakingError {
    #[msg("Stake amount is below the minimum of 1 SOL.")]
    BelowMinimumStake,
    #[msg("Already staked.")]
    AlreadyStaked,
    #[msg("Not currently staked.")]
    NotStaked,
    #[msg("Not in cooldown.")]
    NotInCooldown,
    #[msg("Cooldown period not elapsed.")]
    CooldownNotElapsed,
    #[msg("Vault PDA is incorrect.")]
    InvalidVault,
}
