// ==================== Jupiter DEX 聚合器 CPI 集成模块 ====================
//
// Task 1.16: 替换 Raydium 为 Jupiter，实现最优路由 Swap
//
// 设计说明:
//   - 采用"透传模式"：前端获取 Jupiter swap-instructions 后直接传入
//   - 合约校验 Jupiter Program ID 和指令 discriminator
//   - Vault PDA 作为 signer 执行 swap
//
// 安全考虑 (CRITICAL):
//   - swap_data 必须验证长度 (至少 8 字节 discriminator)
//   - 必须验证 Jupiter Route 指令 discriminator
//   - 防止恶意构造的 swap_data 执行非预期操作
//
// remaining_accounts 说明:
//   - 由前端从 Jupiter API /v6/swap-instructions 获取
//   - 第一个账户必须是 Jupiter Program
//   - 账户数量取决于路由路径 (通常 10-50 个)

use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::solana_program::program::invoke_signed;
use anchor_spl::token::TokenAccount;

use crate::constants::{JUPITER_PROGRAM_ID, NATIVE_SOL_MINT};
use crate::errors::IPFlowError;

/// Jupiter Route 指令的 discriminator (8 字节)
/// 来源: Jupiter V6 Program IDL
/// 指令: route (最常用的 swap 指令)
const JUPITER_ROUTE_DISCRIMINATOR: [u8; 8] = [229, 100, 247, 91, 30, 192, 179, 237];

/// Jupiter SharedAccountsRoute 指令的 discriminator
/// 用于共享账户的路由优化
const JUPITER_SHARED_ACCOUNTS_ROUTE_DISCRIMINATOR: [u8; 8] = [193, 32, 155, 51, 65, 214, 156, 129];

/// Jupiter ExactOutRoute 指令的 discriminator
/// 用于精确输出金额的 swap
const JUPITER_EXACT_OUT_ROUTE_DISCRIMINATOR: [u8; 8] = [208, 51, 239, 151, 123, 43, 237, 92];

/// 通过 Jupiter 执行 Swap (带滑点保护)
///
/// # 安全说明
/// - 验证 swap_data 长度至少 8 字节
/// - 验证 Jupiter 指令 discriminator (route/sharedAccountsRoute/exactOutRoute)
/// - 验证 Jupiter Program ID
/// - **CRITICAL**: swap 后验证输出金额 >= minimum_amount_out
///
/// # 参数
/// - `remaining_accounts`: 从 Jupiter swap-instructions API 获取的账户列表
///   - 第一个账户必须是 Jupiter Program
///   - 后续账户为路由所需的各种账户
/// - `swap_data`: Jupiter swap 指令的 data 字段 (由前端透传，需验证 discriminator)
/// - `vault`: Vault PDA 账户 (作为 token 持有者)
/// - `vault_bump`: Vault PDA bump seed
/// - `user_output_token_account`: 用户输出 token 账户 (用于验证余额变化)
/// - `minimum_amount_out`: 最小输出金额 (滑点保护)
/// - `max_input_amount`: 允许的最大输入金额 (限制 Vault 支出)
///
/// # 返回
/// - `Ok(())`: Swap 成功且输出满足最小要求
/// - `Err(IPFlowError)`: Swap 失败、校验不通过或滑点超限
pub fn swap_via_jupiter<'info>(
    remaining_accounts: &[AccountInfo<'info>],
    swap_data: Vec<u8>,
    vault: &AccountInfo<'info>,
    vault_bump: u8,
    user_output_token_account: &AccountInfo<'info>,
    minimum_amount_out: u64,
    max_input_amount: u64,
) -> Result<()> {
    // ==================== 校验 swap_data 安全性 (CRITICAL) ====================

    // 1. 长度校验：至少需要 8 字节 discriminator
    require!(
        swap_data.len() >= 8,
        IPFlowError::InvalidSwapData
    );

    // 2. Discriminator 校验：必须是已知的 Jupiter 指令
    let discriminator: [u8; 8] = swap_data[0..8]
        .try_into()
        .map_err(|_| error!(IPFlowError::InvalidSwapData))?;

    let is_valid_discriminator = discriminator == JUPITER_ROUTE_DISCRIMINATOR
        || discriminator == JUPITER_SHARED_ACCOUNTS_ROUTE_DISCRIMINATOR
        || discriminator == JUPITER_EXACT_OUT_ROUTE_DISCRIMINATOR;

    require!(
        is_valid_discriminator,
        IPFlowError::InvalidSwapData
    );

    // ==================== 记录 swap 前余额 (CRITICAL: 滑点保护) ====================
    let balance_before = get_token_amount(user_output_token_account)?;

    // ==================== 校验输入账户 (CRITICAL: 限制 Vault 支出) ====================
    let vault_input_token_account = find_vault_wsol_account(remaining_accounts, vault)?;
    require!(
        vault_input_token_account.key() != user_output_token_account.key(),
        IPFlowError::InvalidTokenAccount
    );
    let input_balance_before = get_token_amount(&vault_input_token_account)?;

    msg!(
        "Jupiter swap_data validated: len={}, discriminator={:?}, user_output={}, balance_before={}, min_out={}",
        swap_data.len(),
        discriminator,
        user_output_token_account.key(),
        balance_before,
        minimum_amount_out
    );

    // ==================== 校验 remaining_accounts ====================

    // 至少需要 Jupiter Program + 若干路由账户
    require!(
        remaining_accounts.len() >= 2,
        IPFlowError::MissingSwapAccounts
    );

    // 第一个账户必须是 Jupiter Program
    let jupiter_program = &remaining_accounts[0];
    require!(
        jupiter_program.key() == JUPITER_PROGRAM_ID,
        IPFlowError::InvalidJupiterProgram
    );

    // ==================== 构建账户列表 ====================

    // 从 remaining_accounts[1..] 构建 AccountMeta 列表
    // 将 vault 匹配的账户标记为 signer
    let accounts: Vec<AccountMeta> = remaining_accounts[1..]
        .iter()
        .map(|acc| {
            let is_signer = acc.key == vault.key;
            if acc.is_writable {
                AccountMeta::new(*acc.key, is_signer)
            } else {
                AccountMeta::new_readonly(*acc.key, is_signer)
            }
        })
        .collect();

    // 收集 AccountInfo 用于 invoke_signed
    let account_infos: Vec<AccountInfo> = remaining_accounts[1..].to_vec();

    // ==================== 构建并执行指令 ====================

    let ix = Instruction {
        program_id: jupiter_program.key(),
        accounts,
        data: swap_data,
    };

    // Vault PDA 签名种子
    let seeds = &[b"vault".as_ref(), &[vault_bump]];
    let signer_seeds = &[&seeds[..]];

    invoke_signed(&ix, &account_infos, signer_seeds)?;

    // ==================== 验证 swap 后余额 (CRITICAL: 滑点保护) ====================
    // CPI 后账户数据已更新，直接重新读取即可
    let balance_after = get_token_amount(user_output_token_account)?;

    let actual_output = balance_after
        .checked_sub(balance_before)
        .ok_or(error!(IPFlowError::MathOverflow))?;

    msg!(
        "Jupiter swap completed: balance_after={}, actual_output={}, minimum_required={}",
        balance_after,
        actual_output,
        minimum_amount_out
    );

    // 验证实际输出 >= 最小输出要求
    require!(
        actual_output >= minimum_amount_out,
        IPFlowError::SlippageExceeded
    );

    // ==================== 验证 Vault 输入不超过上限 ====================
    let input_balance_after = get_token_amount(&vault_input_token_account)?;
    let input_spent = input_balance_before.saturating_sub(input_balance_after);
    require!(
        input_spent <= max_input_amount,
        IPFlowError::ExcessiveSwapInput
    );

    msg!("Jupiter swap executed successfully with slippage protection verified");

    Ok(())
}

fn get_token_amount(account: &AccountInfo) -> Result<u64> {
    let data = account.try_borrow_data()?;
    let token_account = TokenAccount::try_deserialize(&mut &data[..])?;
    Ok(token_account.amount)
}

fn find_vault_wsol_account<'info>(
    remaining_accounts: &[AccountInfo<'info>],
    vault: &AccountInfo<'info>,
) -> Result<AccountInfo<'info>> {
    let mut found: Option<AccountInfo<'info>> = None;

    for acc in remaining_accounts.iter().skip(1) {
        let data = match acc.try_borrow_data() {
            Ok(data) => data,
            Err(_) => continue,
        };
        let token_account = match TokenAccount::try_deserialize(&mut &data[..]) {
            Ok(token_account) => token_account,
            Err(_) => continue,
        };

        if token_account.owner == *vault.key && token_account.mint == NATIVE_SOL_MINT {
            require!(acc.is_writable, IPFlowError::InvalidTokenAccount);
            if found.is_some() {
                return Err(error!(IPFlowError::InvalidTokenAccount));
            }
            found = Some(acc.clone());
        }
    }

    found.ok_or(error!(IPFlowError::MissingSwapAccounts))
}

/// 计算最小输出量 (滑点保护)
///
/// # 参数
/// - `expected_output`: 前端从 Jupiter quote 获取的预期输出量
/// - `slippage_bps`: 滑点容忍度 (basis points, 100 = 1%)
///
/// # 返回
/// - 最小接受的输出量
#[inline]
pub fn calculate_min_output(expected_output: u64, slippage_bps: u64) -> Result<u64> {
    // min_output = expected * (10000 - slippage_bps) / 10000
    expected_output
        .checked_mul(10000 - slippage_bps)
        .ok_or(error!(IPFlowError::MathOverflow))?
        .checked_div(10000)
        .ok_or(error!(IPFlowError::MathOverflow))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_min_output() {
        // 3% slippage (300 bps)
        assert_eq!(calculate_min_output(1000, 300).unwrap(), 970);

        // 1% slippage (100 bps)
        assert_eq!(calculate_min_output(1000, 100).unwrap(), 990);

        // 0% slippage
        assert_eq!(calculate_min_output(1000, 0).unwrap(), 1000);

        // Edge case: large number
        assert_eq!(
            calculate_min_output(1_000_000_000, 300).unwrap(),
            970_000_000
        );
    }
}
