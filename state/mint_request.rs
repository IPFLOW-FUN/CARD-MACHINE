use anchor_lang::prelude::*;

// ==================== VRF 请求状态 ====================

#[account]
#[derive(InitSpace)]
pub struct MintRequest {
    /// 发起请求的用户
    pub user: Pubkey, // 32 bytes

    /// 关联的 Randomness 账户 (用于 PDA 派生和 VRF 校验)
    pub randomness_account: Pubkey, // 32 bytes

    /// 购买的周卡数量 (5U/张)
    pub amount_of_cards: u32, // 4 bytes

    /// 请求状态 (Pending → Revealed → Claimed)
    pub status: RequestStatus, // 1 byte

    /// 用户选择的支付方式 (SOL 或 USDT)
    pub payment_mode: PaymentMode, // 1 byte

    /// 中奖总额 (micro-USD, 精度 10^6)
    pub total_won_usd: u64, // 8 bytes

    /// 实际支付的金额 (如果是 SOL 则是 lamports, 如果是 Token 则是 token amount)
    pub paid_amount: u64, // 8 bytes

    /// 请求创建时间戳 (用于超时退款)
    pub created_at: i64, // 8 bytes

    /// 随机数揭示时间戳 (用于领取超时校验)
    pub revealed_at: i64, // 8 bytes

    /// VRF 随机选中的奖品池索引 (0-4, Task 1.23)
    pub selected_pool_index: u8, // 1 byte

    /// VRF Commit 阶段锁定的 slot (用于防重放校验)
    pub commit_slot: u64, // 8 bytes

    /// VRF Reveal 阶段的 slot (用于审计)
    pub reveal_slot: u64, // 8 bytes

    /// VRF 请求发起时的 slot (用于防重放校验和审计，兼容旧字段)
    pub vrf_request_slot: u64, // 8 bytes
}

#[derive(
    AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Default, Debug,
)]
pub enum RequestStatus {
    /// 等待随机数揭示
    #[default]
    Pending,
    /// 已揭示随机数，等待用户领取
    Revealed,
    /// 已完成领取
    Claimed,
    /// 失败 (可退款)
    Failed,
}

#[derive(
    AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Default, Debug,
)]
pub enum PayoutMode {
    /// 95% SOL 兑付
    #[default]
    SOL,
    /// IP 代币回购发放
    Token,
}

// ==================== 支付方式 ====================

#[derive(
    AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Default, Debug,
)]
pub enum PaymentMode {
    /// SOL 支付 (按 Pyth 实时汇率换算)
    #[default]
    SOL,
    /// USDT 直接支付 (固定 5U/张)
    USDT,
}

// ==================== Swap 路由选择 (Task 1.20) ====================

#[derive(
    AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Default, Debug,
)]
pub enum SwapRouter {
    /// Jupiter DEX 聚合器 (推荐，最优路由)
    #[default]
    Jupiter,
    /// Raydium CPMM 直连 (备选)
    Raydium,
}
