use anchor_lang::prelude::*;

/// 奖品池最大数量
pub const MAX_PRIZE_POOLS: usize = 50;

#[account]
pub struct IPFlowState {
    pub admin: Pubkey,
    pub vault_bump: u8,
    pub total_collected: u64,
    pub platform_fee_bps: u16, // 平台利润比例，例如 500 表示 5%
    pub is_paused: bool,
    pub pool_count: u8,
    /// 下一个可用索引（只增不减，用于创建新池）(Task 3.3)
    pub prize_pool_count: u8,
    /// 当前活跃池子数量（VRF 取模基数）(Task 3.3)
    pub active_pool_count: u8,
    /// 活跃池子索引列表（有序，无空洞）(Task 3.3)
    /// 255 表示空位
    pub active_pool_indices: [u8; MAX_PRIZE_POOLS],
    /// VRF Oracle Queue 白名单
    pub oracle_queue: Pubkey,
    /// 退款超时时间（秒）
    pub request_timeout_seconds: i64,
}

impl IPFlowState {
    // 32 (admin) + 1 (vault_bump) + 8 (total_collected) + 2 (platform_fee_bps)
    // + 1 (is_paused) + 1 (pool_count) + 1 (prize_pool_count)
    // + 1 (active_pool_count) + 50 (active_pool_indices) + 32 (oracle_queue)
    // + 8 (request_timeout_seconds)
    pub const INIT_SPACE: usize =
        32 + 1 + 8 + 2 + 1 + 1 + 1 + 1 + MAX_PRIZE_POOLS + 32 + 8;
}
