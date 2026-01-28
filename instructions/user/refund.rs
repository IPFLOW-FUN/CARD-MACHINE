// ==================== Task 2.3: 超时退款指令 ====================
//
// 当 MintRequest 处于 Pending 状态超过 10 分钟（VRF 未回调）时，
// 允许用户申请退款，防止资金卡死。
//
// 支持两种退款模式:
// - SOL 退款: Vault → User (System Program transfer)
// - USDT 退款: VaultTokenAccount → UserTokenAccount (SPL Token transfer)

use anchor_lang::prelude::*;
use anchor_spl::token;

use crate::constants::USDT_MINT_DEVNET;
use crate::errors::IPFlowError;
use crate::state::{PaymentMode, RequestStatus};
use crate::Refund;

pub fn handler(ctx: Context<Refund>) -> Result<()> {
    let clock = Clock::get()?;
    let request = &ctx.accounts.mint_request;

    // ==================== 1. 校验退款条件 ====================
    // 条件: Pending 状态且超过 request_timeout_seconds
    let request_timeout_seconds = ctx.accounts.config.request_timeout_seconds;
    let is_pending_timeout = request.status == RequestStatus::Pending
        && clock.unix_timestamp - request.created_at > request_timeout_seconds;

    require!(is_pending_timeout, IPFlowError::RefundNotAllowed);

    msg!(
        "Refund triggered: created_at={}, now={}, timeout={}s",
        request.created_at,
        clock.unix_timestamp,
        request_timeout_seconds
    );

    // ==================== 2. 根据支付方式执行退款 ====================
    match request.payment_mode {
        PaymentMode::SOL => {
            // SOL 退款: Vault → User
            let refund_amount = request.paid_amount;
            let vault = &ctx.accounts.vault;
            let user = &ctx.accounts.user;
            let config = &ctx.accounts.config;

            // Vault 余额检查
            require!(
                vault.lamports() >= refund_amount,
                IPFlowError::InsufficientVaultBalance
            );

            // PDA 签名转账
            let seeds = &[b"vault".as_ref(), &[config.vault_bump]];
            let signer = &[&seeds[..]];

            anchor_lang::solana_program::program::invoke_signed(
                &anchor_lang::solana_program::system_instruction::transfer(
                    vault.key,
                    user.key,
                    refund_amount,
                ),
                &[
                    vault.to_account_info(),
                    user.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                ],
                signer,
            )?;

            msg!("SOL refund completed: {} lamports", refund_amount);
        }
        PaymentMode::USDT => {
            // USDT 退款: VaultTokenAccount → UserTokenAccount
            let refund_amount = request.paid_amount;

            // 校验必需的 Token 账户存在
            let token_program = ctx
                .accounts
                .token_program
                .as_ref()
                .ok_or(IPFlowError::RefundNotAllowed)?;

            let vault_token_account = ctx
                .accounts
                .vault_token_account
                .as_ref()
                .ok_or(IPFlowError::RefundNotAllowed)?;

            let user_token_account = ctx
                .accounts
                .user_token_account
                .as_ref()
                .ok_or(IPFlowError::RefundNotAllowed)?;

            // 校验 Token 账户余额
            require!(
                vault_token_account.amount >= refund_amount,
                IPFlowError::InsufficientVaultBalance
            );

            // 校验用户 Token 账户 owner
            require!(
                user_token_account.owner == ctx.accounts.user.key(),
                IPFlowError::Unauthorized
            );
            require!(
                user_token_account.mint == USDT_MINT_DEVNET,
                IPFlowError::InvalidTokenAccount
            );

            // 校验 Vault Token 账户 mint/owner
            require!(
                vault_token_account.mint == USDT_MINT_DEVNET,
                IPFlowError::InvalidTokenAccount
            );
            require!(
                vault_token_account.owner == ctx.accounts.vault.key(),
                IPFlowError::InvalidTokenAccount
            );

            // Vault PDA 签名
            let config = &ctx.accounts.config;
            let seeds = &[b"vault".as_ref(), &[config.vault_bump]];
            let signer = &[&seeds[..]];

            // SPL Token 转账
            token::transfer(
                CpiContext::new_with_signer(
                    token_program.to_account_info(),
                    token::Transfer {
                        from: vault_token_account.to_account_info(),
                        to: user_token_account.to_account_info(),
                        authority: ctx.accounts.vault.to_account_info(),
                    },
                    signer,
                ),
                refund_amount,
            )?;

            msg!(
                "USDT refund completed: {} (6 decimals)",
                refund_amount
            );
        }
    }

    // 3. 关闭 MintRequest PDA (租金退给用户)
    // 通过 Anchor 的 close = user 自动处理

    msg!(
        "Refund completed for request created at {}",
        request.created_at
    );

    Ok(())
}
