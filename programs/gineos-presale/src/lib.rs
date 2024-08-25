use anchor_lang::prelude::*;
use anchor_spl::token::{ self, Token, TokenAccount, Transfer };
use anchor_spl::associated_token::AssociatedToken;

declare_id!("5fafMD9vnGUFY9oSwJaavHWWq5hovW1MNih1xTnvWGq8");

#[program]
pub mod gineos_presale {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, phases: [Phase; 10]) -> Result<()> {
        let presale_account = &mut ctx.accounts.presale_account;
        presale_account.phases = phases;
        presale_account.current_phase = 0;
        presale_account.total_tokens_sold = 0;
        Ok(())
    }

    pub fn buy_tokens(
        ctx: Context<BuyTokens>,
        amount: u64,
        payment_method: PaymentMethod
    ) -> Result<()> {
        let presale_account = &mut ctx.accounts.presale_account;
        let current_phase_index = presale_account.current_phase as usize;

        if current_phase_index >= presale_account.phases.len() {
            return Err(ErrorCode::InvalidPhaseIndex.into());
        }

        let current_phase = &presale_account.phases[current_phase_index];
        let price_per_token = current_phase.price_per_token;
        let total_cost = amount.checked_mul(price_per_token).ok_or(ErrorCode::ArithmeticOverflow)?;

        // Handle payments based on the payment method
        match payment_method {
            PaymentMethod::SOL => {
                let payer = &ctx.accounts.payer;
                let payer_lamports = payer.lamports();
                if payer_lamports < total_cost {
                    return Err(ErrorCode::InsufficientFunds.into());
                }
                // Transfer SOL from payer to presale_account
                **payer.try_borrow_mut_lamports()? -= total_cost;
                **presale_account.to_account_info().try_borrow_mut_lamports()? += total_cost;
            }
            PaymentMethod::USDT | PaymentMethod::USDC => {
                let transfer_amount = total_cost
                    .checked_div(price_per_token)
                    .ok_or(ErrorCode::ArithmeticOverflow)?;

                let token_account = match payment_method {
                    PaymentMethod::USDT => ctx.accounts.usdt_account.to_account_info(),
                    PaymentMethod::USDC => ctx.accounts.usdc_account.to_account_info(),
                    _ => {
                        return Err(ErrorCode::UnsupportedPaymentMethod.into());
                    }
                };

                let cpi_ctx = CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: token_account,
                        to: presale_account.to_account_info(),
                        authority: ctx.accounts.payer.to_account_info(),
                    }
                );

                token::transfer(cpi_ctx, transfer_amount)?;
            }
        }

        // Perform the token transfer to the payer
        let token_transfer_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.token_account.to_account_info(),
                to: ctx.accounts.payer.to_account_info(),
                authority: presale_account.to_account_info(), // Ensure this authority is correct
            }
        );
        token::transfer(token_transfer_ctx, amount)?;

        presale_account.total_tokens_sold = presale_account.total_tokens_sold
            .checked_add(amount)
            .ok_or(ErrorCode::ArithmeticOverflow)?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = initializer, space = 8 + 32 + 1 + 4 + Phase::SIZE * 10)]
    pub presale_account: Account<'info, PresaleAccount>,
    #[account(mut)]
    pub initializer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct BuyTokens<'info> {
    #[account(mut)]
    pub presale_account: Account<'info, PresaleAccount>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token>,
    #[account(mut)]
    pub token_account: Account<'info, TokenAccount>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    #[account(mut)]
    pub usdt_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub usdc_account: Account<'info, TokenAccount>,
}

#[account]
pub struct PresaleAccount {
    pub phases: [Phase; 10],
    pub current_phase: u8,
    pub total_tokens_sold: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Phase {
    pub price_per_token: u64,
    pub token_amount: u64,
}

impl Phase {
    const SIZE: usize = 16 + 8; // size of `price_per_token` and `token_amount`
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub enum PaymentMethod {
    SOL,
    USDT,
    USDC,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid phase index.")]
    InvalidPhaseIndex,
    #[msg("Arithmetic overflow.")]
    ArithmeticOverflow,
    #[msg("Insufficient funds.")]
    InsufficientFunds,
    #[msg("Unsupported payment method.")]
    UnsupportedPaymentMethod,
    #[msg("Invalid mint.")]
    InvalidMint,
}
