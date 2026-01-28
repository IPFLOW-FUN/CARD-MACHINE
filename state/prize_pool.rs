// ==================== 奖品池状态定义 (Task 3.3) ====================

use anchor_lang::prelude::*;

/// 池子类型枚举
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Default, InitSpace)]
#[repr(u8)]
pub enum PoolType {
    #[default]
    RaydiumCPMM = 0,
    RaydiumAMM = 1,
    Jupiter = 2,
    Orca = 3,
}

/// 独立奖品池 PDA（无 is_active，采用硬删除）
///
/// Seeds: [b"prize_pool", index]
/// 每个奖品池对应一个独立的 PDA 账户
#[account]
#[derive(InitSpace)]
pub struct PrizePoolAccount {
    /// 池子索引（永久分配，不重用）
    pub index: u8,
    /// 交易对地址 (Raydium Pool / Jupiter Route)
    pub swap_pool: Pubkey,
    /// 池子类型
    pub pool_type: PoolType,
    /// 显示名称 (最长 16 字节，如 "USDT", "BONK", "WIF")
    #[max_len(16)]
    pub name: String,
    /// PDA bump
    pub bump: u8,
}

// 空间: 8 (discriminator) + 1 (index) + 32 (swap_pool) + 1 (pool_type)
//       + 4 (String len prefix) + 16 (name max) + 1 (bump) = 63 bytes
// 租金: ~0.00089 SOL
