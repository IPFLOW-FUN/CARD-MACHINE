pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;
pub mod utils;

use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use pyth_solana_receiver_sdk::price_update::PriceUpdateV2;

declare_id!("ALRWyaQkjVGznjAXsxhqXkyYDaETPUN2xj82W8uyji53");

#[program]
pub mod ipflow_v3 {
    use super::*;

    /// 系统初始化
    pub fn initialize(ctx: Context<Initialize>, platform_fee_bps: u16) -> Result<()> {
        instructions::admin::initialize::handler(ctx, platform_fee_bps)
    }

    /// 迁移/扩容全局配置账户 (仅管理员)
    pub fn migrate_config(ctx: Context<MigrateConfig>, prize_pool_count: u8) -> Result<()> {
        instructions::admin::initialize::migrate_config(ctx, prize_pool_count)
    }

    /// 关闭全局配置账户 (仅管理员，用于重新初始化)
    pub fn close_config(ctx: Context<CloseConfig>) -> Result<()> {
        instructions::admin::initialize::close_config(ctx)
    }

    /// 用户发起抽奖请求 (支付 10U/张 并发起 MagicBlock VRF 请求)
    /// payment_mode: SOL 或 USDT 支付方式
    /// client_seed: 用户提供的随机种子 (用于 VRF 请求)
    /// request_slot: 请求发起时的 slot (用于 PDA 派生)
    pub fn request_mint(
        ctx: Context<RequestMint>,
        amount_of_cards: u32,
        payment_mode: PaymentMode,
        client_seed: u8,
        request_slot: u64,
    ) -> Result<()> {
        instructions::user::request_mint::handler(
            ctx,
            amount_of_cards,
            payment_mode,
            client_seed,
            request_slot,
        )
    }

    /// VRF 回调处理 - 由 MagicBlock VRF 程序自动调用
    /// 不应由用户直接调用
    pub fn consume_lottery_randomness(
        ctx: Context<ConsumeLotteryRandomness>,
        randomness: [u8; 32],
    ) -> Result<()> {
        instructions::oracle::consume_randomness::handler(ctx, randomness)
    }

    /// 用户领取奖励 (选择 SOL 或 Token 发放方式)
    /// - payout_mode: SOL 或 Token 发放方式
    /// - swap_router: Token 模式时选择 DEX 路由 (Jupiter/Raydium)，SOL 模式传 None
    /// - expected_token_output: Token 模式必填，前端从 DEX quote 获取的预期输出量
    /// - swap_data: Token 模式必填，从 DEX swap-instructions API 获取的指令数据
    /// - vrf_request_slot: VRF 请求时的 slot (用于 PDA 派生)
    pub fn claim<'info>(
        ctx: Context<'_, '_, 'info, 'info, Claim<'info>>,
        payout_mode: PayoutMode,
        swap_router: Option<SwapRouter>,
        expected_token_output: Option<u64>,
        swap_data: Option<Vec<u8>>,
        _vrf_request_slot: u64,
    ) -> Result<()> {
        instructions::user::claim::handler(
            ctx,
            payout_mode,
            swap_router,
            expected_token_output,
            swap_data,
        )
    }

    /// 超时退款 (Task 2.3)
    /// 当 MintRequest 处于 Pending 状态超过 10 分钟时，用户可申请退款
    /// - vrf_request_slot: VRF 请求时的 slot (用于 PDA 派生)
    pub fn refund(ctx: Context<Refund>, _vrf_request_slot: u64) -> Result<()> {
        instructions::user::refund::handler(ctx)
    }

    // ==================== 管理员指令 (Task 3.1) ====================

    /// 管理员提取 SOL
    /// - amount: 提取金额 (lamports)
    pub fn withdraw_sol(ctx: Context<WithdrawSol>, amount: u64) -> Result<()> {
        instructions::admin::withdraw::withdraw_sol(ctx, amount)
    }

    /// 管理员提取 Token
    /// - amount: 提取金额 (raw token amount)
    pub fn withdraw_token(ctx: Context<WithdrawToken>, amount: u64) -> Result<()> {
        instructions::admin::withdraw::withdraw_token(ctx, amount)
    }

    // ==================== 奖品池管理 (Task 3.3) ====================

    /// 添加奖品池
    /// - swap_pool: 交易对地址 (Raydium Pool / Jupiter Route)
    /// - pool_type: 池子类型
    /// - name: 显示名称 (最长 16 字节)
    pub fn add_prize_pool(
        ctx: Context<AddPrizePool>,
        swap_pool: Pubkey,
        pool_type: PoolType,
        name: String,
    ) -> Result<()> {
        instructions::admin::prize_pool::add_prize_pool(ctx, swap_pool, pool_type, name)
    }

    /// 硬删除奖品池（关闭 PDA，退还租金）
    pub fn remove_prize_pool(ctx: Context<RemovePrizePool>) -> Result<()> {
        instructions::admin::prize_pool::remove_prize_pool(ctx)
    }

    /// 更新奖品池
    /// - swap_pool: 可选，新的交易对地址
    /// - pool_type: 可选，新的池子类型
    /// - name: 可选，新的显示名称
    pub fn update_prize_pool(
        ctx: Context<UpdatePrizePool>,
        swap_pool: Option<Pubkey>,
        pool_type: Option<PoolType>,
        name: Option<String>,
    ) -> Result<()> {
        instructions::admin::prize_pool::update_prize_pool(ctx, swap_pool, pool_type, name)
    }
}

// ==================== Context Definitions (Moved to lib.rs for Macro Visibility) ====================

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = 8 + IPFlowState::INIT_SPACE,
        seeds = [constants::SEED_GLOBAL_CONFIG],
        bump
    )]
    pub config: Account<'info, IPFlowState>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct MigrateConfig<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    /// CHECK: 迁移过程中需要兼容旧版结构，手动校验 admin
    #[account(
        mut,
        seeds = [constants::SEED_GLOBAL_CONFIG],
        bump
    )]
    pub config: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CloseConfig<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    /// CHECK: 关闭过程中需要兼容旧版结构，手动校验 admin
    #[account(
        mut,
        seeds = [constants::SEED_GLOBAL_CONFIG],
        bump
    )]
    pub config: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(amount_of_cards: u32, payment_mode: PaymentMode, client_seed: u8, request_slot: u64)]
pub struct RequestMint<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [constants::SEED_GLOBAL_CONFIG],
        bump,
        constraint = !config.is_paused @ errors::IPFlowError::ProgramPaused
    )]
    pub config: Account<'info, IPFlowState>,

    /// 程序金库，用于接收 SOL
    /// CHECK: PDA used as vault
    #[account(
        mut,
        seeds = [b"vault"],
        bump = config.vault_bump
    )]
    pub vault: AccountInfo<'info>,

    /// MintRequest PDA - 使用 request_slot 作为种子的一部分
    /// request_slot 由前端传入，合约内验证是否为当前 slot
    #[account(
        init,
        payer = user,
        space = 8 + MintRequest::INIT_SPACE,
        seeds = [constants::SEED_MINT_REQUEST, user.key().as_ref(), &request_slot.to_le_bytes()],
        bump
    )]
    pub mint_request: Account<'info, MintRequest>,

    /// MagicBlock VRF Oracle Queue
    /// CHECK: 由前端传入正确的 Queue 地址
    #[account(mut)]
    pub oracle_queue: AccountInfo<'info>,

    /// 程序身份 PDA - 用于 VRF 请求签名
    /// CHECK: Seeds 验证
    #[account(seeds = [b"identity"], bump)]
    pub program_identity: AccountInfo<'info>,

    /// VRF 程序
    /// CHECK: 地址验证确保是 MagicBlock VRF 程序
    #[account(address = ephemeral_vrf_sdk::consts::VRF_PROGRAM_ID)]
    pub vrf_program: AccountInfo<'info>,

    /// Slot Hashes Sysvar
    /// CHECK: 地址验证确保是 SlotHashes sysvar
    #[account(address = anchor_lang::solana_program::sysvar::slot_hashes::ID)]
    pub slot_hashes: AccountInfo<'info>,

    /// Pyth 价格数据账户 (SOL 支付时必需)
    pub pyth_price_update: Account<'info, PriceUpdateV2>,

    pub system_program: Program<'info, System>,

    // ==================== USDT 支付相关账户 (可选) ====================
    /// Token Program (USDT 支付时必需)
    pub token_program: Option<Program<'info, Token>>,

    /// USDT Mint 账户 (USDT 支付时必需，用于校验)
    pub usdt_mint: Option<Account<'info, Mint>>,

    /// 用户的 USDT Token 账户 (USDT 支付时必需)
    #[account(mut)]
    pub user_token_account: Option<Account<'info, TokenAccount>>,

    /// 协议的 USDT Token 账户 (USDT 支付时必需)
    #[account(mut)]
    pub vault_token_account: Option<Account<'info, TokenAccount>>,
}

/// ConsumeLotteryRandomness: VRF 回调处理
/// 由 MagicBlock VRF 程序自动调用，不应由用户直接调用
#[derive(Accounts)]
pub struct ConsumeLotteryRandomness<'info> {
    /// VRF 程序身份 PDA - 验证调用来源
    /// 只有 MagicBlock VRF 程序可以调用此指令
    /// CHECK: 通过 address constraint 验证是 VRF_PROGRAM_IDENTITY
    #[account(address = ephemeral_vrf_sdk::consts::VRF_PROGRAM_IDENTITY)]
    pub vrf_program_identity: Signer<'info>,

    /// MintRequest 账户 - 通过 callback_accounts_metas 传入
    #[account(
        mut,
        constraint = mint_request.status == RequestStatus::Pending @ errors::IPFlowError::InvalidRequestStatus
    )]
    pub mint_request: Account<'info, MintRequest>,

    /// 全局配置 - 获取活跃奖品池信息
    #[account(seeds = [constants::SEED_GLOBAL_CONFIG], bump)]
    pub config: Account<'info, IPFlowState>,
}

/// Claim: 用户领取奖励 (选择 SOL 或 Token)
/// Task 1.14: claim 完成后自动关闭 MintRequest PDA，退还租金给用户
#[derive(Accounts)]
#[instruction(payout_mode: PayoutMode, swap_router: Option<SwapRouter>, expected_token_output: Option<u64>, swap_data: Option<Vec<u8>>, vrf_request_slot: u64)]
pub struct Claim<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        close = user,  // Task 1.14: 关闭账户，租金退还给 user
        seeds = [constants::SEED_MINT_REQUEST, user.key().as_ref(), &vrf_request_slot.to_le_bytes()],
        bump,
        has_one = user @ errors::IPFlowError::Unauthorized,
        constraint = mint_request.status == RequestStatus::Revealed @ errors::IPFlowError::InvalidRequestStatus,
        constraint = mint_request.vrf_request_slot == vrf_request_slot @ errors::IPFlowError::InvalidRequestStatus
    )]
    pub mint_request: Account<'info, MintRequest>,

    #[account(
        seeds = [constants::SEED_GLOBAL_CONFIG],
        bump,
    )]
    pub config: Account<'info, IPFlowState>,

    /// 程序金库，用于支付 SOL 奖金
    /// CHECK: PDA
    #[account(
        mut,
        seeds = [b"vault"],
        bump = config.vault_bump
    )]
    pub vault: AccountInfo<'info>,

    /// Pyth 价格数据账户 (SOL 模式需要)
    pub pyth_price_update: Account<'info, PriceUpdateV2>,

    pub system_program: Program<'info, System>,
    // TODO: Task 1.10 添加 Raydium CPI 所需的 remaining_accounts
}

/// Refund: 超时退款 (Task 2.3)
/// 当 MintRequest 处于 Pending 状态超过 10 分钟时，用户可申请退款
/// - SOL 退款: 仅需基础账户
/// - USDT 退款: 需要额外传入 Token 账户
#[derive(Accounts)]
#[instruction(vrf_request_slot: u64)]
pub struct Refund<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        close = user,  // 关闭账户，租金退还给 user
        seeds = [constants::SEED_MINT_REQUEST, user.key().as_ref(), &vrf_request_slot.to_le_bytes()],
        bump,
        has_one = user @ errors::IPFlowError::Unauthorized,
        constraint = mint_request.vrf_request_slot == vrf_request_slot @ errors::IPFlowError::InvalidRequestStatus
    )]
    pub mint_request: Account<'info, MintRequest>,

    #[account(
        seeds = [constants::SEED_GLOBAL_CONFIG],
        bump,
    )]
    pub config: Account<'info, IPFlowState>,

    /// 程序金库，用于退还 SOL
    /// CHECK: PDA
    #[account(
        mut,
        seeds = [b"vault"],
        bump = config.vault_bump
    )]
    pub vault: AccountInfo<'info>,

    pub system_program: Program<'info, System>,

    // ==================== USDT 退款专用账户（可选）====================

    /// Token Program (USDT 退款时必需)
    pub token_program: Option<Program<'info, Token>>,

    /// Vault 的 USDT Token 账户 (USDT 退款时必需)
    #[account(mut)]
    pub vault_token_account: Option<Account<'info, TokenAccount>>,

    /// 用户的 USDT Token 账户 (USDT 退款时必需)
    #[account(mut)]
    pub user_token_account: Option<Account<'info, TokenAccount>>,
}

// ==================== 管理员指令 Context (Task 3.1) ====================

/// WithdrawSol: 管理员提取 SOL
#[derive(Accounts)]
pub struct WithdrawSol<'info> {
    /// 管理员签名者
    #[account(
        mut,
        constraint = config.admin == admin.key() @ errors::IPFlowError::Unauthorized
    )]
    pub admin: Signer<'info>,

    /// 全局配置
    #[account(
        seeds = [constants::SEED_GLOBAL_CONFIG],
        bump,
    )]
    pub config: Account<'info, IPFlowState>,

    /// 程序金库 PDA
    /// CHECK: PDA used as vault
    #[account(
        mut,
        seeds = [b"vault"],
        bump = config.vault_bump
    )]
    pub vault: AccountInfo<'info>,

    /// 接收 SOL 的地址
    /// CHECK: 任意地址均可接收
    #[account(mut)]
    pub recipient: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

/// WithdrawToken: 管理员提取 Token
#[derive(Accounts)]
pub struct WithdrawToken<'info> {
    /// 管理员签名者
    #[account(
        constraint = config.admin == admin.key() @ errors::IPFlowError::Unauthorized
    )]
    pub admin: Signer<'info>,

    /// 全局配置
    #[account(
        seeds = [constants::SEED_GLOBAL_CONFIG],
        bump,
    )]
    pub config: Account<'info, IPFlowState>,

    /// 程序金库 PDA (作为 Token 转账 authority)
    /// CHECK: PDA used as vault
    #[account(
        seeds = [b"vault"],
        bump = config.vault_bump
    )]
    pub vault: AccountInfo<'info>,

    /// Vault 的 Token ATA
    #[account(mut)]
    pub vault_token_account: Account<'info, TokenAccount>,

    /// 接收 Token 的 ATA
    #[account(mut)]
    pub recipient_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

// ==================== 奖品池管理 Context (Task 3.3) ====================

/// AddPrizePool: 添加奖品池
#[derive(Accounts)]
#[instruction(swap_pool: Pubkey, pool_type: PoolType, name: String)]
pub struct AddPrizePool<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [constants::SEED_GLOBAL_CONFIG],
        bump,
        constraint = config.admin == admin.key() @ errors::IPFlowError::Unauthorized
    )]
    pub config: Account<'info, IPFlowState>,

    #[account(
        init,
        payer = admin,
        space = 8 + PrizePoolAccount::INIT_SPACE,
        seeds = [constants::SEED_PRIZE_POOL, &[config.prize_pool_count]],
        bump
    )]
    pub prize_pool: Account<'info, PrizePoolAccount>,

    pub system_program: Program<'info, System>,
}

/// RemovePrizePool: 硬删除奖品池（关闭 PDA，退还租金）
#[derive(Accounts)]
pub struct RemovePrizePool<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [constants::SEED_GLOBAL_CONFIG],
        bump,
        constraint = config.admin == admin.key() @ errors::IPFlowError::Unauthorized
    )]
    pub config: Account<'info, IPFlowState>,

    #[account(
        mut,
        close = admin,  // 硬删除：关闭 PDA，租金退给 admin
        seeds = [constants::SEED_PRIZE_POOL, &[prize_pool.index]],
        bump = prize_pool.bump
    )]
    pub prize_pool: Account<'info, PrizePoolAccount>,
}

/// UpdatePrizePool: 更新奖品池
#[derive(Accounts)]
pub struct UpdatePrizePool<'info> {
    pub admin: Signer<'info>,

    #[account(
        seeds = [constants::SEED_GLOBAL_CONFIG],
        bump,
        constraint = config.admin == admin.key() @ errors::IPFlowError::Unauthorized
    )]
    pub config: Account<'info, IPFlowState>,

    #[account(
        mut,
        seeds = [constants::SEED_PRIZE_POOL, &[prize_pool.index]],
        bump = prize_pool.bump
    )]
    pub prize_pool: Account<'info, PrizePoolAccount>,
}
