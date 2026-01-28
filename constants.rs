use anchor_lang::prelude::*;

#[constant]
pub const SEED_GLOBAL_CONFIG: &[u8] = b"global_config";

#[constant]
pub const SEED_MINT_REQUEST: &[u8] = b"mint_request";

// ==================== Pyth Network Price Feeds ====================

/// Pyth SOL/USD Price Feed ID
pub const PYTH_SOL_USD_FEED_ID: [u8; 32] =
    hex_literal::hex!("ef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d");

/// Pyth SOL/USD Price Feed Update Account on Devnet
pub const PYTH_SOL_USD_DEVNET: Pubkey = pubkey!("7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE");

/// Switchboard On-Demand Program ID on Devnet
pub const SWITCHBOARD_ON_DEMAND_DEVNET: Pubkey =
    pubkey!("Aio4gaXjXzJNVLtzwtNVmSqGKpANtXhybbkhtAC94ji2");

// ==================== MagicBlock VRF Constants ====================

/// MagicBlock VRF Program ID (Devnet/Mainnet)
/// 参考: ephemeral-vrf-sdk/src/consts.rs
pub const VRF_PROGRAM_ID: Pubkey = pubkey!("Vrf1RNUjXmQGjmQrQLvJHs9SNkvDJEsRVFPkfSQUwGz");

/// MagicBlock VRF Program Identity PDA
/// 这是 VRF 程序签名回调的身份 PDA，用于验证回调来源
/// 参考: ephemeral-vrf-sdk/src/consts.rs
pub const VRF_PROGRAM_IDENTITY: Pubkey = pubkey!("9irBy75QS2BN81FUgXuHcjqceJJRuc9oDkAe8TKVvvAw");

/// MagicBlock VRF Default Queue
/// 默认的 VRF 请求队列 (所有网络通用)
/// 参考: ephemeral-vrf-sdk/src/consts.rs
pub const ORACLE_QUEUE_DEVNET: Pubkey = pubkey!("Cuj97ggrhhidhbu39TijNVqE74xvKJ69gDervRUXAxGh");

// ==================== USDT Token Constants ====================

/// USDT Mint Address on Devnet (使用官方 Mock USDT)
/// 注意: Devnet 上可能需要使用测试代币，此地址需根据实际情况更新
pub const USDT_MINT_DEVNET: Pubkey = pubkey!("CHLJch4C3fnh2jdE3DhNJRHmqpjaWzDzBKg7UH1FqRLT");

/// USDT Decimals (6 位精度)
pub const USDT_DECIMALS: u32 = 6;

// ==================== Business Logic Constants ====================

/// Target USD amount for one mint (10 USD)
pub const TARGET_USD_AMOUNT: u64 = 10;

/// SOL Decimals
pub const SOL_DECIMALS: u32 = 9;

/// USD Precision (10^6)
pub const USD_PRECISION: u64 = 1_000_000;

// ==================== Claim Timeout Constants ====================

/// Claim timeout in seconds (24 hours)
pub const CLAIM_TIMEOUT_SECONDS: i64 = 24 * 60 * 60;

/// Request timeout for refund in seconds (45 seconds for testing)
/// 用户在 Pending 状态超过此时间后可申请退款
/// NOTE: 生产环境应改回 10 * 60 (10 分钟)
pub const REQUEST_TIMEOUT_SECONDS: i64 = 45;

// ==================== WSOL (Wrapped SOL) Constants ====================

/// Native SOL Mint Address (WSOL)
/// This is the canonical address for wrapped SOL on all Solana clusters
pub const NATIVE_SOL_MINT: Pubkey = pubkey!("So11111111111111111111111111111111111111112");

// ==================== Jupiter DEX Aggregator Constants ====================

/// Jupiter V6 Program ID (Mainnet & Devnet)
pub const JUPITER_PROGRAM_ID: Pubkey = pubkey!("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4");

/// 默认滑点保护：3% (300 basis points)
pub const DEFAULT_SLIPPAGE_BPS: u64 = 300;

// ==================== Raydium CPMM Constants (Task 1.20) ====================

/// Raydium CPMM Swap Program ID (Mainnet)
/// 用于校验 remaining_accounts 中传入的程序地址
pub const RAYDIUM_CP_SWAP_PROGRAM: Pubkey = pubkey!("CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C");

/// Raydium CPMM Swap Program ID (Devnet)
pub const RAYDIUM_CP_SWAP_PROGRAM_DEVNET: Pubkey =
    pubkey!("DRaycpLY18LhpbydsBWbVJtxpNv9oXPgjRSfpF2bWpYb");

/// Raydium CPMM remaining_accounts 固定数量 (13 个账户)
pub const RAYDIUM_SWAP_ACCOUNTS_COUNT: usize = 13;

// ==================== Prize Pool Constants (Task 1.23) ====================

/// 默认奖品池数量 (5 个 Raydium CPMM 池子)
pub const DEFAULT_PRIZE_POOL_COUNT: u8 = 5;

// ==================== Prize Pool Management (Task 3.3) ====================

/// 奖品池 PDA Seed
#[constant]
pub const SEED_PRIZE_POOL: &[u8] = b"prize_pool";

/// 奖品池最大数量
pub const MAX_PRIZE_POOLS: usize = 50;

// ==================== 分层概率配置 ====================
// 目标分布 (单抽 10U):
// - Tier 1 (15%): 5.0 - 7.0 USDC,   期望 6.0,  贡献 0.9
// - Tier 2 (50%): 7.0 - 14.0 USDC,  期望 10.5, 贡献 5.25
// - Tier 3 (30%): 14.0 - 49.9 USDC, 期望 31.95, 贡献 9.585
// - Tier 4 (5%):  50.0 - 99.9 USDC, 期望 74.95, 贡献 3.7475
// 总期望: 19.48 USDC | 本金: 10 USDC | ROI: +94.8%
// 精度: 0.1 USDC (100,000 micro-USDC)

/// 概率精度基数 (1000000 = 100.0000%)
pub const PROB_PRECISION: u64 = 1_000_000;

/// 奖金步进精度 (0.1 USDC = 100,000 micro-USDC)
pub const REWARD_STEP: u64 = 100_000;

/// Tier 1: 15% 概率, 5.0-7.0 USDC (21 个离散值)
pub const TIER1_THRESHOLD: u64 = 150_000; // 累积: 15%
pub const TIER1_MIN_USD: u64 = 5_000_000; // 5.0 USDC (micro)
pub const TIER1_STEPS: u64 = 21; // 5.0, 5.1, ..., 7.0

/// Tier 2: 50% 概率, 7.0-14.0 USDC (71 个离散值)
pub const TIER2_THRESHOLD: u64 = 650_000; // 累积: 65%
pub const TIER2_MIN_USD: u64 = 7_000_000; // 7.0 USDC (micro)
pub const TIER2_STEPS: u64 = 71; // 7.0, 7.1, ..., 14.0

/// Tier 3: 30% 概率, 14.0-49.9 USDC (360 个离散值)
pub const TIER3_THRESHOLD: u64 = 950_000; // 累积: 95%
pub const TIER3_MIN_USD: u64 = 14_000_000; // 14.0 USDC (micro)
pub const TIER3_STEPS: u64 = 360; // 14.0, 14.1, ..., 49.9

/// Tier 4: 5% 概率, 50.0-99.9 USDC (500 个离散值)
pub const TIER4_MIN_USD: u64 = 50_000_000; // 50.0 USDC (micro)
pub const TIER4_STEPS: u64 = 500; // 50.0, 50.1, ..., 99.9

// 保留旧常量用于测试兼容 (将被废弃)
pub const TIER1_MAX_USD: u64 = 7_000_000; // 7.0 USDC
pub const TIER2_MAX_USD: u64 = 14_000_000; // 14.0 USDC
pub const TIER3_MAX_USD: u64 = 49_900_000; // 49.9 USDC
pub const TIER4_MAX_USD: u64 = 99_900_000; // 99.9 USDC
