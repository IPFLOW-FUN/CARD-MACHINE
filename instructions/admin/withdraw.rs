use anchor_lang::prelude::*;
use anchor_spl::token::{self, Transfer};

use crate::errors::IPFlowError;
use crate::WithdrawSol;
use crate::WithdrawToken;

// ==================== SOL 提取 ====================

/// 提取 SOL 到指定接收地址
pub fn withdraw_sol(ctx: Context<WithdrawSol>, amount: u64) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let recipient = &ctx.accounts.recipient;
    let config = &ctx.accounts.config;

    // 保留最小租金，防止账户被关闭
    let min_rent = Rent::get()?.minimum_balance(0);
    let available = vault.lamports().saturating_sub(min_rent);
    require!(amount <= available, IPFlowError::InsufficientVaultBalance);

    // PDA 签名转账
    let seeds = &[b"vault".as_ref(), &[config.vault_bump]];
    let signer = &[&seeds[..]];

    anchor_lang::system_program::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: vault.to_account_info(),
                to: recipient.to_account_info(),
            },
            signer,
        ),
        amount,
    )?;

    msg!(
        "Admin withdrew {} lamports from Vault to {}",
        amount,
        recipient.key()
    );
    Ok(())
}

// ==================== Token 提取 ====================

/// 提取 Token 到指定接收地址
pub fn withdraw_token(ctx: Context<WithdrawToken>, amount: u64) -> Result<()> {
    let config = &ctx.accounts.config;

    // 检查 Token 余额
    require!(
        ctx.accounts.vault_token_account.amount >= amount,
        IPFlowError::InsufficientVaultBalance
    );

    // PDA 签名
    let seeds = &[b"vault".as_ref(), &[config.vault_bump]];
    let signer = &[&seeds[..]];

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.vault_token_account.to_account_info(),
                to: ctx.accounts.recipient_token_account.to_account_info(),
                authority: ctx.accounts.vault.to_account_info(),
            },
            signer,
        ),
        amount,
    )?;

    msg!(
        "Admin withdrew {} tokens from Vault to {}",
        amount,
        ctx.accounts.recipient_token_account.key()
    );
    Ok(())
}
