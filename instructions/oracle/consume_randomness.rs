// ==================== MagicBlock VRF 回调指令 ====================
//
// 处理 MagicBlock Ephemeral VRF 回调，计算抽奖结果
// 状态转换：Pending -> Revealed
// 用户后续调用 claim 选择发放方式

use anchor_lang::prelude::*;

use crate::errors::IPFlowError;
use crate::state::RequestStatus;
use crate::utils::vrf_helper::process_vrf_result;
use crate::ConsumeLotteryRandomness;

/// MagicBlock VRF 回调事件
#[event]
pub struct LotteryRevealed {
    /// 用户地址
    pub user: Pubkey,
    /// MintRequest PDA 地址
    pub mint_request: Pubkey,
    /// 总中奖金额 (micro-USD, 精度 10^6)
    pub total_won_usd: u64,
    /// 选中的奖品池索引
    pub selected_pool_index: u8,
    /// 揭示时间戳
    pub revealed_at: i64,
}

/// 处理 MagicBlock VRF 回调 (handler 入口)
///
/// # 参数
/// - `ctx`: 账户上下文 (ConsumeLotteryRandomness，定义于 lib.rs)
/// - `randomness`: 32 字节 VRF 随机数
///
/// # 状态转换
/// - MintRequest.status: Pending -> Revealed
///
/// # 安全考虑
/// - 仅允许 VRF 程序身份 PDA 调用 (由 lib.rs 中 address constraint 保证)
/// - 防重放：仅处理 Pending 状态的请求
/// - 幂等性：已 Revealed 的请求直接返回 Ok
pub fn handler(ctx: Context<ConsumeLotteryRandomness>, randomness: [u8; 32]) -> Result<()> {
    let mint_request = &mut ctx.accounts.mint_request;
    let config = &ctx.accounts.config;
    let clock = Clock::get()?;

    // 1. 幂等性检查：已揭示则直接返回成功
    // 防止因网络抖动导致的重复调用
    if mint_request.status == RequestStatus::Revealed {
        msg!("Request already revealed, returning Ok (idempotent).");
        return Ok(());
    }

    // 2. 状态校验 (由 Accounts constraint 保证，这里做双重校验)
    require!(
        mint_request.status == RequestStatus::Pending,
        IPFlowError::InvalidRequestStatus
    );

    // 3. 处理 VRF 结果，计算奖金和选择奖品池
    let result = process_vrf_result(
        &randomness,
        mint_request.amount_of_cards,
        config.active_pool_count,
        &config.active_pool_indices,
    )
    .map_err(|_| IPFlowError::MathOverflow)?;

    // 4. 更新 MintRequest 状态
    mint_request.status = RequestStatus::Revealed;
    mint_request.total_won_usd = result.total_won_usd;
    mint_request.selected_pool_index = result.selected_pool_index;
    mint_request.revealed_at = clock.unix_timestamp;
    mint_request.reveal_slot = clock.slot;

    // 5. 发射事件 (供链下索引)
    emit!(LotteryRevealed {
        user: mint_request.user,
        mint_request: mint_request.key(),
        total_won_usd: result.total_won_usd,
        selected_pool_index: result.selected_pool_index,
        revealed_at: clock.unix_timestamp,
    });

    msg!(
        "Lottery Revealed: User={}, Cards={}, Total Won USD={} (micro), Pool Index={}",
        mint_request.user,
        mint_request.amount_of_cards,
        result.total_won_usd,
        result.selected_pool_index
    );

    Ok(())
}

// ==================== 单元测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 LotteryRevealed 事件结构
    #[test]
    fn test_lottery_revealed_event_structure() {
        // 编译测试，确保 LotteryRevealed 事件结构正确
        let event = LotteryRevealed {
            user: Pubkey::default(),
            mint_request: Pubkey::default(),
            total_won_usd: 100_000_000, // 100 USD
            selected_pool_index: 2,
            revealed_at: 1700000000,
        };

        assert_eq!(event.total_won_usd, 100_000_000);
        assert_eq!(event.selected_pool_index, 2);
    }

    /// 测试随机数处理边界条件 - 全零
    #[test]
    fn test_randomness_boundary_zero() {
        let zero_randomness = [0u8; 32];
        let indices = create_active_pool_indices(&[0, 1, 2, 3, 4]);
        let result = process_vrf_result(&zero_randomness, 1, 5, &indices);
        assert!(result.is_ok());
    }

    /// 测试随机数处理边界条件 - 全 0xFF
    #[test]
    fn test_randomness_boundary_max() {
        let max_randomness = [0xFF; 32];
        let indices = create_active_pool_indices(&[0, 1, 2, 3, 4]);
        let result = process_vrf_result(&max_randomness, 1, 5, &indices);
        assert!(result.is_ok());
    }

    /// 测试无活跃池时返回默认索引
    #[test]
    fn test_no_active_pools() {
        let randomness = [42u8; 32];
        let empty_indices = [255u8; 50];
        let result = process_vrf_result(&randomness, 1, 0, &empty_indices);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().selected_pool_index, 0);
    }

    /// 测试多张卡的奖金累加
    #[test]
    fn test_multiple_cards() {
        let randomness = [123u8; 32];
        let indices = create_active_pool_indices(&[0, 1, 2]);

        let result_1 = process_vrf_result(&randomness, 1, 3, &indices).unwrap();
        let result_10 = process_vrf_result(&randomness, 10, 3, &indices).unwrap();

        // 多张卡的总奖金应该大于或等于单张
        assert!(result_10.total_won_usd >= result_1.total_won_usd);
    }

    /// 测试奖金范围合理性
    #[test]
    fn test_prize_range() {
        use crate::constants::{TIER1_MAX_USD, TIER4_MIN_USD};

        let indices = create_active_pool_indices(&[0, 1, 2]);

        // 多组随机数测试
        for seed in 0..10u8 {
            let mut randomness = [0u8; 32];
            randomness[0] = seed;

            let result = process_vrf_result(&randomness, 1, 3, &indices).unwrap();

            // 单张卡奖金应在 [TIER4_MIN_USD, TIER1_MAX_USD) 范围内
            assert!(result.total_won_usd >= TIER4_MIN_USD);
            assert!(result.total_won_usd < TIER1_MAX_USD);
        }
    }

    /// 测试池选择在有效范围内
    #[test]
    fn test_pool_selection_valid() {
        let active_values = [0, 2, 4, 6, 8];
        let indices = create_active_pool_indices(&active_values);

        for seed in 0..20u8 {
            let mut randomness = [0u8; 32];
            randomness[8] = seed; // 池选择使用字节 8-15

            let result = process_vrf_result(&randomness, 1, 5, &indices).unwrap();

            // 选中的池索引必须是活跃池之一 (0, 2, 4, 6, 8)
            assert!(active_values.contains(&result.selected_pool_index));
        }
    }

    /// 测试确定性：相同输入产生相同输出
    #[test]
    fn test_deterministic() {
        let randomness = [99u8; 32];
        let indices = create_active_pool_indices(&[0, 1, 2]);

        let result_a = process_vrf_result(&randomness, 5, 3, &indices).unwrap();
        let result_b = process_vrf_result(&randomness, 5, 3, &indices).unwrap();

        assert_eq!(result_a.total_won_usd, result_b.total_won_usd);
        assert_eq!(result_a.selected_pool_index, result_b.selected_pool_index);
    }

    /// 辅助函数：创建活跃池索引数组
    fn create_active_pool_indices(active: &[u8]) -> [u8; 50] {
        let mut indices = [255u8; 50];
        for (i, &idx) in active.iter().enumerate() {
            if i < 50 {
                indices[i] = idx;
            }
        }
        indices
    }
}
