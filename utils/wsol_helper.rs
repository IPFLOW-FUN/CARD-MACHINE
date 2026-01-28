// ==================== WSOL (Wrapped SOL) Helper Module ====================
//
// 封装 WSOL 包装相关的 CPI 调用：
// 1. 从 native SOL 转账到 WSOL Account
// 2. 调用 sync_native 同步 WSOL 余额
// 3. 关闭 WSOL Account 回收 rent

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{program::invoke_signed, system_instruction};
use anchor_spl::token::spl_token;

/// 从 Vault PDA 向目标 WSOL Token Account 包装指定数量的 SOL
///
/// # 流程
/// 1. system_program::transfer: Vault SOL -> WSOL Token Account
/// 2. spl_token::sync_native: 同步 WSOL 余额
///
/// # 参数
/// - `vault`: Vault PDA (SOL 来源)
/// - `wsol_token_account`: 目标 WSOL Token Account
/// - `system_program`: System Program
/// - `token_program`: SPL Token Program
/// - `amount`: 包装的 lamports 数量
/// - `signer_seeds`: Vault PDA 签名种子
pub fn wrap_sol<'info>(
    vault: &AccountInfo<'info>,
    wsol_token_account: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    amount: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    // 1. 从 Vault 转账 SOL 到 WSOL Token Account
    let transfer_ix = system_instruction::transfer(vault.key, wsol_token_account.key, amount);

    invoke_signed(
        &transfer_ix,
        &[
            vault.clone(),
            wsol_token_account.clone(),
            system_program.clone(),
        ],
        signer_seeds,
    )?;

    msg!("WSOL Wrap: Transferred {} lamports to WSOL account", amount);

    // 2. 调用 sync_native 同步 WSOL 余额
    let sync_ix = spl_token::instruction::sync_native(token_program.key, wsol_token_account.key)?;

    invoke_signed(
        &sync_ix,
        std::slice::from_ref(wsol_token_account),
        &[], // sync_native 不需要 PDA 签名
    )?;

    msg!("WSOL Wrap: Synced native balance");

    Ok(())
}

/// 关闭 WSOL Token Account，回收 rent 到指定目标
///
/// # 参数
/// - `wsol_token_account`: 要关闭的 WSOL Token Account
/// - `destination`: 接收 rent 的目标账户
/// - `authority`: Token Account 的 owner (通常是 Vault PDA)
/// - `token_program`: SPL Token Program
/// - `signer_seeds`: Authority PDA 签名种子
pub fn close_wsol_account<'info>(
    wsol_token_account: &AccountInfo<'info>,
    destination: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let close_ix = spl_token::instruction::close_account(
        token_program.key,
        wsol_token_account.key,
        destination.key,
        authority.key,
        &[],
    )?;

    invoke_signed(
        &close_ix,
        &[
            wsol_token_account.clone(),
            destination.clone(),
            authority.clone(),
        ],
        signer_seeds,
    )?;

    msg!("WSOL: Closed account, rent returned to destination");

    Ok(())
}
