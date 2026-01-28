// ==================== 程序事件定义 ====================
//
// 事件用于链下索引和历史追溯
// 由于 MintRequest PDA 在 claim 后关闭，事件日志成为唯一的历史记录来源

use crate::state::{PaymentMode, PayoutMode, PoolType, SwapRouter};
use anchor_lang::prelude::*;

/// Claim 完成事件
///
/// 在 MintRequest PDA 关闭前 emit，记录完整的领取信息供链下索引
#[event]
pub struct ClaimCompleted {
    /// 用户地址
    pub user: Pubkey,
    /// 中奖总额 (micro-USD, 精度 10^6)
    pub total_won_usd: u64,
    /// 领取方式 (SOL 或 Token)
    pub payout_mode: PayoutMode,
    /// 支付方式 (SOL 或 USDT)
    pub payment_mode: PaymentMode,
    /// Swap 路由 (Token 模式时使用，SOL 模式为 None)
    /// Task 1.20: 新增字段，记录使用的 DEX 路由
    pub swap_router: Option<SwapRouter>,
    /// 实际支付金额 (lamports 或 token amount)
    pub paid_amount: u64,
    /// 购买的周卡数量
    pub amount_of_cards: u32,
    /// 领取时间戳
    pub timestamp: i64,
}

// ==================== Prize Pool 事件 (Task 3.3) ====================

/// 奖品池添加事件
#[event]
pub struct PrizePoolAdded {
    pub admin: Pubkey,
    pub index: u8,
    pub swap_pool: Pubkey,
    pub pool_type: PoolType,
    pub name: String,
}

/// 奖品池移除事件（硬删除）
#[event]
pub struct PrizePoolRemoved {
    pub admin: Pubkey,
    pub index: u8,
    pub swap_pool: Pubkey,
}

/// 奖品池更新事件
#[event]
pub struct PrizePoolUpdated {
    pub admin: Pubkey,
    pub index: u8,
    pub old_swap_pool: Pubkey,
    pub new_swap_pool: Pubkey,
}
