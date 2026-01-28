use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke_signed;
use anchor_lang::system_program::{transfer, Transfer};
use anchor_spl::token::{transfer as token_transfer, Transfer as TokenTransfer};
use ephemeral_vrf_sdk::consts::IDENTITY;
use ephemeral_vrf_sdk::instructions::{create_request_randomness_ix, RequestRandomnessParams};
use ephemeral_vrf_sdk::types::SerializableAccountMeta;
use solana_program::hash::hash;

use crate::constants::*;
use crate::errors::IPFlowError;
use crate::state::*;
use crate::utils::pyth_oracle;
use crate::RequestMint;

/// Request Mint Handler - MagicBlock VRF 版本
///
/// 用户发起抽奖请求的处理逻辑:
/// 1. 验证卡片数量
/// 2. 验证 request_slot 是否为当前 slot
/// 3. 处理支付 (SOL 或 USDT)
/// 4. 初始化 MintRequest 状态
/// 5. VRF 请求由前端单独发起 (简化版实现)
///
/// 注意: 完整的 VRF CPI 调用需要在集成 ephemeral-vrf-sdk 后实现
pub fn handler(
    ctx: Context<RequestMint>,
    amount_of_cards: u32,
    payment_mode: PaymentMode,
    client_seed: u8, // VRF 客户端随机种子
    request_slot: u64, // 前端传入的请求 slot
) -> Result<()> {
    // 1. 基础校验
    require!(
        amount_of_cards > 0 && amount_of_cards <= 100,
        IPFlowError::InvalidCardAmount
    );

    // 2. 验证 request_slot 是否在当前 slot 的合理范围内 (允许 10 slot 的容差)
    // Solana 每 ~400ms 出一个 slot，10 slots ≈ 4 秒，足够覆盖网络延迟和交易确认
    let current_slot = Clock::get()?.slot;
    require!(
        current_slot.saturating_sub(10) <= request_slot && request_slot <= current_slot,
        IPFlowError::InvalidSlot
    );

    // 2.1 验证 Oracle Queue 是否为白名单
    require!(
        ctx.accounts.oracle_queue.key() == ctx.accounts.config.oracle_queue,
        IPFlowError::InvalidOracleQueue
    );

    // 3. 根据支付方式执行不同的支付逻辑
    let paid_amount: u64;

    match payment_mode {
        PaymentMode::SOL => {
            // ==================== SOL 支付路径 ====================
            // 1. 价格校验与换算 (10U/张)
            let total_usd = (amount_of_cards as u64)
                .checked_mul(TARGET_USD_AMOUNT)
                .ok_or(IPFlowError::MathOverflow)?;

            let total_lamports =
                pyth_oracle::get_lamports_for_usd(&ctx.accounts.pyth_price_update, total_usd)?;

            // 2. 执行支付 (User -> Vault)
            transfer(
                CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.user.to_account_info(),
                        to: ctx.accounts.vault.to_account_info(),
                    },
                ),
                total_lamports,
            )?;

            // 记录支付金额 (lamports)
            paid_amount = total_lamports;

            msg!(
                "SOL Payment: {} lamports for {} cards",
                total_lamports,
                amount_of_cards
            );
        }
        PaymentMode::USDT => {
            // ==================== USDT 支付路径 ====================
            // 1. 校验必需的 USDT 账户是否存在
            let token_program = ctx
                .accounts
                .token_program
                .as_ref()
                .ok_or(IPFlowError::MissingUsdtAccounts)?;
            let usdt_mint = ctx
                .accounts
                .usdt_mint
                .as_ref()
                .ok_or(IPFlowError::MissingUsdtAccounts)?;
            let user_token_account = ctx
                .accounts
                .user_token_account
                .as_ref()
                .ok_or(IPFlowError::MissingUsdtAccounts)?;
            let vault_token_account = ctx
                .accounts
                .vault_token_account
                .as_ref()
                .ok_or(IPFlowError::MissingUsdtAccounts)?;

            // 2. 运行时校验 USDT Mint 地址
            require!(
                usdt_mint.key() == USDT_MINT_DEVNET,
                IPFlowError::InvalidUsdtMint
            );

            // 3. 运行时校验用户 Token 账户
            require!(
                user_token_account.owner == ctx.accounts.user.key(),
                IPFlowError::InvalidTokenAccount
            );
            require!(
                user_token_account.mint == USDT_MINT_DEVNET,
                IPFlowError::InvalidTokenAccount
            );

            // 4. 运行时校验 Vault Token 账户
            require!(
                vault_token_account.mint == USDT_MINT_DEVNET,
                IPFlowError::InvalidTokenAccount
            );
            require!(
                vault_token_account.owner == ctx.accounts.vault.key(),
                IPFlowError::InvalidTokenAccount
            );
            require!(
                vault_token_account.key() != user_token_account.key(),
                IPFlowError::InvalidTokenAccount
            );

            // 5. 计算 USDT 金额 (10U/张, USDT 精度 6 位)
            let total_usdt = (amount_of_cards as u64)
                .checked_mul(TARGET_USD_AMOUNT)
                .ok_or(IPFlowError::MathOverflow)?
                .checked_mul(10u64.pow(USDT_DECIMALS))
                .ok_or(IPFlowError::MathOverflow)?;

            // 6. 执行 USDT 转账 (User -> Vault)
            token_transfer(
                CpiContext::new(
                    token_program.to_account_info(),
                    TokenTransfer {
                        from: user_token_account.to_account_info(),
                        to: vault_token_account.to_account_info(),
                        authority: ctx.accounts.user.to_account_info(),
                    },
                ),
                total_usdt,
            )?;

            // 记录支付金额 (USDT raw amount, 6 decimals)
            paid_amount = total_usdt;

            msg!(
                "USDT Payment: {} USDT (raw) for {} cards",
                total_usdt,
                amount_of_cards
            );
        }
    }

    // 4. 获取 mint_request PDA key (在可变借用之前)
    let mint_request_key = ctx.accounts.mint_request.key();

    // 5. 初始化 MintRequest 状态
    let mint_request = &mut ctx.accounts.mint_request;
    mint_request.user = ctx.accounts.user.key();
    mint_request.randomness_account = Pubkey::default(); // MagicBlock VRF 不需要此字段
    mint_request.amount_of_cards = amount_of_cards;
    mint_request.status = RequestStatus::Pending;
    mint_request.payment_mode = payment_mode;
    mint_request.total_won_usd = 0;
    mint_request.paid_amount = paid_amount;
    mint_request.created_at = Clock::get()?.unix_timestamp;
    mint_request.revealed_at = 0;
    mint_request.selected_pool_index = 0;
    mint_request.commit_slot = request_slot; // 使用 request_slot 作为 commit slot
    mint_request.reveal_slot = 0;
    mint_request.vrf_request_slot = request_slot;

    // 6. 日志输出
    msg!(
        "MintRequest created: user={}, cards={}, vrf_request_slot={}, mint_request_pda={}",
        mint_request.user,
        amount_of_cards,
        request_slot,
        mint_request_key
    );

    // ==================== VRF CPI 调用 ====================
    // 7. 构建 VRF 请求参数
    let vrf_params = RequestRandomnessParams {
        payer: ctx.accounts.user.key(),
        oracle_queue: ctx.accounts.oracle_queue.key(),
        callback_program_id: crate::ID,
        callback_discriminator: crate::instruction::ConsumeLotteryRandomness::DISCRIMINATOR.to_vec(),
        caller_seed: hash(&[client_seed]).to_bytes(),
        // Phase 4.3: 回调账户列表
        // 顺序必须与 ConsumeLotteryRandomness Context 一致
        // vrf_program_identity 由 VRF 程序自动添加，无需在此指定
        accounts_metas: Some(vec![
            SerializableAccountMeta {
                pubkey: mint_request_key, // 需要更新状态 (writable)
                is_signer: false,
                is_writable: true,
            },
            SerializableAccountMeta {
                pubkey: ctx.accounts.config.key(), // 读取奖品池信息 (readonly)
                is_signer: false,
                is_writable: false,
            },
        ]),
        callback_args: None,
    };

    msg!("VRF params prepared: oracle_queue={}", ctx.accounts.oracle_queue.key());

    // 8. 创建 VRF 请求指令
    let vrf_ix = create_request_randomness_ix(vrf_params);

    // 9. 执行 CPI 调用
    // 使用 program_identity PDA 作为签名者
    // 注意：回调账户 (mint_request, config) 已通过 accounts_metas 编码在指令数据中
    // VRF 程序会在回调时自动附加这些账户，这里只需要传入 VRF 请求所需的 5 个账户
    invoke_signed(
        &vrf_ix,
        &[
            ctx.accounts.user.to_account_info(),
            ctx.accounts.program_identity.to_account_info(),
            ctx.accounts.oracle_queue.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.slot_hashes.to_account_info(),
        ],
        &[&[IDENTITY, &[ctx.bumps.program_identity]]],
    )?;

    msg!("VRF request sent successfully");

    Ok(())
}
