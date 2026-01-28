use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::IPFlowError;
use crate::events::ClaimCompleted;
use crate::state::*;
use crate::utils::{jupiter_cpi, pyth_oracle, raydium_cpi, wsol_helper};
use crate::Claim;

// ==================== Token Claim 账户说明 ====================
//
// Task 1.20: 双路由架构 (Jupiter / Raydium)
//
// **Jupiter 路由** (推荐):
//   - remaining_accounts 由前端从 Jupiter /v6/swap-instructions API 获取
//   - 第一个账户必须是 Jupiter Program (JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4)
//   - 账户数量取决于路由路径 (通常 10-50 个)
//   - swap_data 为 Jupiter 返回的指令 data
//
// **Raydium 路由** (备选):
//   - remaining_accounts 为固定 13 个账户:
//     [0] cp_swap_program - Raydium CPMM 程序
//     [1] authority - Pool Authority PDA
//     [2] amm_config - AMM 配置
//     [3] pool_state - 池子状态
//     [4] input_token_account - Vault 的 WSOL ATA (输入，合约自动 wrap SOL)
//     [5] output_token_account - 用户的目标 Token Account (输出)
//     [6] input_vault - Pool WSOL Vault
//     [7] output_vault - Pool Token Vault
//     [8] input_token_program (SPL Token，同时用于 wrap_sol)
//     [9] output_token_program
//     [10] input_token_mint (WSOL)
//     [11] output_token_mint
//     [12] observation_state
//   - swap_data 不使用 (Raydium 参数通过 expected_token_output 传入)
//   - **自动 WSOL 包装**: 合约在 swap 前自动将 Vault SOL 包装到 WSOL ATA
//
// 客户端工作流:
//   Jupiter: quote → swap-instructions → claim(Jupiter, ...)
//   Raydium: getSwapQuote → claim(Raydium, ...)

/// 用户领取奖励
/// - SOL 模式：直接从 Vault 转账 (95% 发放)
/// - Token 模式：通过 Jupiter/Raydium CPI Swap (100% 发放，用户承担滑点)
///
/// # 参数
/// - `payout_mode`: SOL 或 Token 发放方式
/// - `swap_router`: Token 模式时选择 DEX 路由 (Jupiter/Raydium)，SOL 模式传 None
/// - `expected_token_output`: Token 模式必填，前端从 DEX quote 获取的预期输出量
/// - `swap_data`: Token 模式 Jupiter 路由必填；Raydium 路由不需要
pub fn handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, Claim<'info>>,
    payout_mode: PayoutMode,
    swap_router: Option<SwapRouter>,
    expected_token_output: Option<u64>,
    swap_data: Option<Vec<u8>>,
) -> Result<()> {
    let clock = Clock::get()?;
    let request = &mut ctx.accounts.mint_request;

    // 1. 校验领取超时 (24 小时)
    let claim_timeout = CLAIM_TIMEOUT_SECONDS;
    require!(
        clock.unix_timestamp - request.revealed_at < claim_timeout,
        IPFlowError::ClaimExpired
    );

    // 2. 根据 payout_mode 执行发放
    let final_paid_amount: u64;
    let used_router: Option<SwapRouter>;

    match payout_mode {
        PayoutMode::SOL => {
            // ==================== SOL 发放路径 ====================
            // 计算 95% 发放金额
            let payout_usd = request
                .total_won_usd
                .checked_mul(95)
                .ok_or(IPFlowError::MathOverflow)?
                / 100;

            let total_lamports = pyth_oracle::get_lamports_for_micro_usd(
                &ctx.accounts.pyth_price_update,
                payout_usd,
            )?;

            // Vault 余额校验：保留最小租金，确保可用余额足够
            let min_rent = Rent::get()?.minimum_balance(0);
            let available = ctx.accounts.vault.lamports().saturating_sub(min_rent);
            require!(
                total_lamports <= available,
                IPFlowError::InsufficientVaultBalance
            );

            // PDA 签名转账
            let seeds = &[b"vault".as_ref(), &[ctx.accounts.config.vault_bump]];
            let signer = &[&seeds[..]];

            // ==================== 重入保护: 先更新状态 (Effects before Interactions) ====================
            // 遵循 Checks-Effects-Interactions 模式，在 CPI 调用前先标记状态为 Claimed
            request.status = RequestStatus::Claimed;

            anchor_lang::solana_program::program::invoke_signed(
                &anchor_lang::solana_program::system_instruction::transfer(
                    ctx.accounts.vault.key,
                    ctx.accounts.user.key,
                    total_lamports,
                ),
                &[
                    ctx.accounts.vault.to_account_info(),
                    ctx.accounts.user.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                ],
                signer,
            )?;

            final_paid_amount = total_lamports;
            used_router = None;
            msg!("SOL Claim: {} lamports to user", total_lamports);
        }
        PayoutMode::Token => {
            // ==================== Token 发放路径 (双路由调度) ====================
            //
            // Task 1.20: 根据 swap_router 参数选择 Jupiter 或 Raydium 路由

            // Step 1: 校验必需参数
            let expected_output =
                expected_token_output.ok_or(IPFlowError::MissingExpectedOutput)?;
            let router = swap_router.ok_or(IPFlowError::InvalidChoice)?;

            // 校验 remaining_accounts 数量
            require!(
                !ctx.remaining_accounts.is_empty(),
                IPFlowError::MissingSwapAccounts
            );

            // Step 2: 计算发放金额和滑点保护
            // Token 模式：100% 发放 (用户承担滑点风险)
            let payout_usd = request.total_won_usd;

            let amount_in = pyth_oracle::get_lamports_for_micro_usd(
                &ctx.accounts.pyth_price_update,
                payout_usd,
            )?;

            // 计算最小输出 (3% 滑点保护)
            let minimum_amount_out =
                jupiter_cpi::calculate_min_output(expected_output, DEFAULT_SLIPPAGE_BPS)?;

            msg!(
                "Token Claim: amount_in={} lamports, expected_out={}, min_out={} ({}bps slippage), router={:?}",
                amount_in,
                expected_output,
                minimum_amount_out,
                DEFAULT_SLIPPAGE_BPS,
                router
            );

            // Step 3: 根据路由执行 Swap
            let vault_bump = ctx.accounts.config.vault_bump;
            let remaining = &ctx.remaining_accounts;

            // ==================== 重入保护: 先更新状态 (Effects before Interactions) ====================
            // 遵循 Checks-Effects-Interactions 模式，在 CPI 调用前先标记状态为 Claimed
            // 防止恶意合约在 CPI 回调中重入 claim 指令
            request.status = RequestStatus::Claimed;

            match router {
                SwapRouter::Jupiter => {
                    // ==================== Jupiter 路由 ====================
                    let swap_instruction_data =
                        swap_data.ok_or(IPFlowError::MissingExpectedOutput)?;

                    // 从 remaining_accounts 获取用户输出 token 账户
                    // Jupiter swap-instructions 返回的账户列表中，用户输出账户通常在固定位置
                    // 前端需要确保 remaining_accounts[2] 是用户的输出 token 账户
                    // 账户布局: [0]=Jupiter Program, [1]=..., [2]=user_output_token_account, ...
                    require!(
                        remaining.len() >= 3,
                        IPFlowError::MissingSwapAccounts
                    );
                    let user_output_token_account = &remaining[2];

                    // 执行 Jupiter swap 并验证滑点保护
                    jupiter_cpi::swap_via_jupiter(
                        remaining,
                        swap_instruction_data,
                        &ctx.accounts.vault.to_account_info(),
                        vault_bump,
                        user_output_token_account,
                        minimum_amount_out,
                        amount_in,
                    )
                    .map_err(|e| {
                        msg!("Jupiter swap failed: {:?}", e);
                        error!(IPFlowError::JupiterSwapFailed)
                    })?;

                    msg!("Jupiter Swap executed successfully with slippage protection");
                }
                SwapRouter::Raydium => {
                    // ==================== Raydium 路由 ====================
                    // 校验账户数量
                    require!(
                        remaining.len() >= RAYDIUM_SWAP_ACCOUNTS_COUNT,
                        IPFlowError::MissingSwapAccounts
                    );

                    // 校验 Raydium Program ID
                    let cp_swap_program = remaining[0].key();
                    require!(
                        cp_swap_program == RAYDIUM_CP_SWAP_PROGRAM
                            || cp_swap_program == RAYDIUM_CP_SWAP_PROGRAM_DEVNET,
                        IPFlowError::InvalidRaydiumProgram
                    );

                    // 构建 Vault PDA 签名
                    let seeds: &[&[u8]] = &[b"vault".as_ref(), &[vault_bump]];
                    let signer_seeds = &[seeds];

                    // ==================== Step 3.1: 包装 SOL -> WSOL ====================
                    // 从 Vault SOL 余额包装到 Vault WSOL ATA
                    // remaining[4] = input_token_account (Vault WSOL ATA)
                    // remaining[8] = input_token_program (SPL Token)
                    wsol_helper::wrap_sol(
                        &ctx.accounts.vault.to_account_info(),
                        &remaining[4], // wsol_token_account (Vault WSOL ATA)
                        &ctx.accounts.system_program.to_account_info(),
                        &remaining[8], // token_program
                        amount_in,
                        signer_seeds,
                    )
                    .map_err(|e| {
                        msg!("WSOL wrap failed: {:?}", e);
                        error!(IPFlowError::WsolWrapFailed)
                    })?;

                    msg!("WSOL Wrap: {} lamports wrapped to WSOL", amount_in);

                    // ==================== Step 3.2: 执行 Raydium CPMM Swap ====================
                    raydium_cpi::swap_base_input(
                        remaining[0].clone(),                 // cp_swap_program
                        ctx.accounts.vault.to_account_info(), // payer (Vault PDA)
                        remaining[1].clone(),                 // authority
                        remaining[2].clone(),                 // amm_config
                        remaining[3].clone(),                 // pool_state
                        remaining[4].clone(), // input_token_account (Vault WSOL ATA)
                        remaining[5].clone(), // output_token_account (User Token ATA)
                        remaining[6].clone(), // input_vault
                        remaining[7].clone(), // output_vault
                        remaining[8].clone(), // input_token_program
                        remaining[9].clone(), // output_token_program
                        remaining[10].clone(), // input_token_mint
                        remaining[11].clone(), // output_token_mint
                        remaining[12].clone(), // observation_state
                        amount_in,
                        minimum_amount_out,
                        signer_seeds,
                    )
                    .map_err(|e| {
                        msg!("Raydium swap failed: {:?}", e);
                        error!(IPFlowError::RaydiumSwapFailed)
                    })?;

                    msg!("Raydium Swap executed successfully");
                }
            }

            final_paid_amount = amount_in;
            used_router = Some(router);
            msg!(
                "Token Claim: Swapped {} lamports via {:?}",
                amount_in,
                router
            );
        }
    }

    // 3. 更新支付金额 (状态已在各分支的 CPI 前更新，此处仅更新金额)
    request.paid_amount = final_paid_amount;

    // 4. Emit 事件 (Task 1.14: PDA 关闭前记录完整信息供链下索引)
    emit!(ClaimCompleted {
        user: ctx.accounts.user.key(),
        total_won_usd: request.total_won_usd,
        payout_mode,
        payment_mode: request.payment_mode,
        swap_router: used_router,
        paid_amount: final_paid_amount,
        amount_of_cards: request.amount_of_cards,
        timestamp: clock.unix_timestamp,
    });

    msg!(
        "Claim Success: User={}, Mode={:?}, Router={:?}, Paid={}, PDA will be closed",
        ctx.accounts.user.key(),
        payout_mode,
        used_router,
        final_paid_amount
    );

    // 注意: MintRequest PDA 将在指令结束时自动关闭 (close = user)
    // 租金将退还给用户

    Ok(())
}
