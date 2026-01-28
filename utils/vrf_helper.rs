use anchor_lang::solana_program::program_error::ProgramError;

use crate::constants::{
    PROB_PRECISION, REWARD_STEP, TIER1_MIN_USD, TIER1_STEPS, TIER1_THRESHOLD, TIER2_MIN_USD,
    TIER2_STEPS, TIER2_THRESHOLD, TIER3_MIN_USD, TIER3_STEPS, TIER3_THRESHOLD, TIER4_MIN_USD,
    TIER4_STEPS,
};

// ==================== VRF Helper: 通用随机数处理 ====================

/// 抽奖结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LotteryResult {
    /// 总中奖金额 (USDC, 6 位精度)
    pub total_won_usd: u64,
    /// 选中的奖品池索引
    pub selected_pool_index: u8,
}

/// 处理 VRF 回调结果，计算抽奖奖金
///
/// # 参数
/// - `randomness`: 32 字节 VRF 随机数
/// - `amount_of_cards`: 抽卡数量
/// - `active_pool_count`: 当前活跃池数量
/// - `active_pool_indices`: 活跃池索引列表 (最多 50 个，255 表示空位)
///
/// # 返回值
/// - `LotteryResult`: 包含总中奖金额和选中的奖品池索引
pub fn process_vrf_result(
    randomness: &[u8; 32],
    amount_of_cards: u32,
    active_pool_count: u8,
    active_pool_indices: &[u8; 50],
) -> std::result::Result<LotteryResult, ProgramError> {
    let mut total_won_usd: u64 = 0;

    for i in 0..amount_of_cards {
        let card_random = derive_random_result(randomness, i);
        let won = map_to_tiered_distribution(&card_random);
        total_won_usd = total_won_usd
            .checked_add(won)
            .ok_or(ProgramError::ArithmeticOverflow)?;
    }

    let selected_pool_index = if active_pool_count > 0 {
        select_active_prize_pool(randomness, active_pool_count, active_pool_indices)
    } else {
        0
    };

    Ok(LotteryResult {
        total_won_usd,
        selected_pool_index,
    })
}

/// 计数器法: 从原始随机数派生特定索引的随机数
/// 使用简单的 XOR 和位旋转实现确定性派生
pub fn derive_random_result(raw_seed: &[u8; 32], index: u32) -> [u8; 32] {
    let mut result = *raw_seed;
    let index_bytes = index.to_le_bytes();

    // 使用 index 对每个字节进行变换
    for (i, byte) in result.iter_mut().enumerate() {
        // XOR with index bytes (cycling through index_bytes)
        *byte ^= index_bytes[i % 4];
        // 添加一些位混合
        *byte = byte.wrapping_add((index as u8).wrapping_mul((i + 1) as u8));
    }

    // 进一步混合: 使用前一字节影响后一字节
    for i in 1..32 {
        result[i] ^= result[i - 1].wrapping_mul(31);
    }

    result
}

pub fn compute_pool_index(random_bytes: &[u8; 32], pool_count: u64) -> u64 {
    if pool_count == 0 {
        return 0;
    }
    let random_u64 = u64::from_le_bytes([
        random_bytes[0],
        random_bytes[1],
        random_bytes[2],
        random_bytes[3],
        random_bytes[4],
        random_bytes[5],
        random_bytes[6],
        random_bytes[7],
    ]);
    random_u64 % pool_count
}

/// 选择奖品池索引 (Task 1.23)
/// 使用 VRF 随机数对奖品池数量取模，返回 0 到 pool_count-1 的索引
pub fn select_prize_pool(random_bytes: &[u8; 32], pool_count: u8) -> u8 {
    if pool_count == 0 {
        return 0;
    }
    // 使用随机数的第 8-15 字节（避免与奖金计算使用相同的熵源）
    let random_u64 = u64::from_le_bytes([
        random_bytes[8],
        random_bytes[9],
        random_bytes[10],
        random_bytes[11],
        random_bytes[12],
        random_bytes[13],
        random_bytes[14],
        random_bytes[15],
    ]);
    (random_u64 % (pool_count as u64)) as u8
}

/// 选择活跃奖品池索引 (Task 2.11.1)
///
/// 使用 VRF 随机数从活跃池列表中选择一个池子。
/// 与 `select_prize_pool` 不同，此函数正确处理池子删除后的间隙。
///
/// # 参数
/// - `random_bytes`: 32 字节 VRF 随机数
/// - `active_pool_count`: 当前活跃池数量 (VRF 取模基数)
/// - `active_pool_indices`: 活跃池索引列表 (最多 50 个，255 表示空位)
///
/// # 返回值
/// 实际的池子索引 (从 `active_pool_indices` 中取出)
///
/// # 逻辑
/// 1. 使用 VRF 随机数对 `active_pool_count` 取模，得到位置 (position)
/// 2. 返回 `active_pool_indices[position]` 作为实际池子索引
pub fn select_active_prize_pool(
    random_bytes: &[u8; 32],
    active_pool_count: u8,
    active_pool_indices: &[u8; 50],
) -> u8 {
    // 没有活跃池时返回 0 (默认值)
    if active_pool_count == 0 {
        return 0;
    }

    // 使用随机数的第 8-15 字节 (与 select_prize_pool 保持一致)
    let random_u64 = u64::from_le_bytes([
        random_bytes[8],
        random_bytes[9],
        random_bytes[10],
        random_bytes[11],
        random_bytes[12],
        random_bytes[13],
        random_bytes[14],
        random_bytes[15],
    ]);

    // 对活跃池数量取模，得到位置
    let position = (random_u64 % (active_pool_count as u64)) as usize;

    // 从活跃池列表中取出实际索引
    active_pool_indices[position]
}

/// [已废弃] 原平方根反演算法，保留用于回退
/// 核心算法: 线性概率映射 (1-400U)
#[allow(dead_code)]
pub fn map_to_linear_curve(random_bytes: &[u8; 32], min_usd: u64, max_usd: u64) -> u64 {
    let mut entropy_bytes = [0u8; 16];
    entropy_bytes.copy_from_slice(&random_bytes[0..16]);
    let entropy = u128::from_le_bytes(entropy_bytes);
    let range = (max_usd as u128).saturating_sub(min_usd as u128);
    let range_sq = range.checked_mul(range).unwrap_or(0);
    if range_sq == 0 {
        return min_usd;
    }
    let random_val_sq = entropy % range_sq;
    let s = integer_sqrt(random_val_sq);
    max_usd.checked_sub(s as u64).unwrap_or(min_usd)
}

/// 分层概率映射：将 VRF 随机数映射为分层奖金
///
/// 32 字节 VRF 随机数熵分配：
/// - 字节 0-7:   选择 Tier (取模 1000000)
/// - 字节 8-15:  Tier 内离散步进选择
/// - 字节 16-23: 选择奖品池 (保持现有逻辑)
///
/// 分布设计 (单抽 10U):
/// - Tier 1 (15%): 5.0 - 7.0 USDC,   21 个离散值
/// - Tier 2 (50%): 7.0 - 14.0 USDC,  71 个离散值
/// - Tier 3 (30%): 14.0 - 49.9 USDC, 360 个离散值
/// - Tier 4 (5%):  50.0 - 99.9 USDC, 500 个离散值
///
/// 精度: 0.1 USDC (100,000 micro-USDC)
pub fn map_to_tiered_distribution(random_bytes: &[u8; 32]) -> u64 {
    // 1. 提取熵源选择 Tier (字节 0-7)
    let tier_entropy = u64::from_le_bytes(random_bytes[0..8].try_into().unwrap());
    let tier_roll = tier_entropy % PROB_PRECISION;

    // 2. 提取 Tier 内步进熵源 (字节 8-15)
    let step_entropy = u64::from_le_bytes(random_bytes[8..16].try_into().unwrap());

    // 3. 确定 Tier 及计算奖金
    let (min_usd, steps) = if tier_roll < TIER1_THRESHOLD {
        (TIER1_MIN_USD, TIER1_STEPS) // 15%: 5.0-7.0 USDC
    } else if tier_roll < TIER2_THRESHOLD {
        (TIER2_MIN_USD, TIER2_STEPS) // 50%: 7.0-14.0 USDC
    } else if tier_roll < TIER3_THRESHOLD {
        (TIER3_MIN_USD, TIER3_STEPS) // 30%: 14.0-49.9 USDC
    } else {
        (TIER4_MIN_USD, TIER4_STEPS) // 5%: 50.0-99.9 USDC
    };

    // 4. 计算离散步进索引并生成奖金
    let idx = step_entropy % steps;
    min_usd.saturating_add(idx.saturating_mul(REWARD_STEP))
}

fn integer_sqrt(n: u128) -> u128 {
    if n < 2 {
        return n;
    }
    let mut x = n / 2 + 1;
    let mut y = (x + n / x) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{TIER1_MAX_USD, TIER2_MAX_USD, TIER3_MAX_USD, TIER4_MAX_USD};
    use proptest::prelude::*;

    fn tier_roll(random_bytes: &[u8; 32]) -> u64 {
        let tier_entropy = u64::from_le_bytes(random_bytes[0..8].try_into().unwrap());
        tier_entropy % PROB_PRECISION
    }

    fn tier_id_from_roll(tier_roll: u64) -> u8 {
        if tier_roll < TIER1_THRESHOLD {
            1
        } else if tier_roll < TIER2_THRESHOLD {
            2
        } else if tier_roll < TIER3_THRESHOLD {
            3
        } else {
            4
        }
    }

    fn tier_id(random_bytes: &[u8; 32]) -> u8 {
        tier_id_from_roll(tier_roll(random_bytes))
    }

    fn build_random_bytes(
        tier_entropy: u64,
        amount_entropy: u64,
        tail: [u8; 16],
    ) -> [u8; 32] {
        let mut random_bytes = [0u8; 32];
        random_bytes[0..8].copy_from_slice(&tier_entropy.to_le_bytes());
        random_bytes[8..16].copy_from_slice(&amount_entropy.to_le_bytes());
        random_bytes[16..32].copy_from_slice(&tail);
        random_bytes
    }

    proptest! {
        #[test]
        fn tiered_distribution_in_range(random_bytes in any::<[u8; 32]>()) {
            let amount = map_to_tiered_distribution(&random_bytes);
            prop_assert!(amount >= TIER1_MIN_USD); // 最小值: 5.0 USDC
            prop_assert!(amount <= TIER4_MAX_USD); // 最大值: 99.9 USDC
        }

        #[test]
        fn compute_pool_index_in_range(random_bytes in any::<[u8; 32]>(), pool_count in 1u64..=255) {
            let index = compute_pool_index(&random_bytes, pool_count);
            prop_assert!(index < pool_count);
        }

        #[test]
        fn select_prize_pool_in_range(random_bytes in any::<[u8; 32]>(), pool_count in 1u8..=255) {
            let index = select_prize_pool(&random_bytes, pool_count);
            prop_assert!(index < pool_count);
        }

        #[test]
        fn integer_sqrt_upper_bound(n in any::<u64>()) {
            let n128 = n as u128;
            let s = integer_sqrt(n128);
            prop_assert!(s * s <= n128);
        }

        #[test]
        fn integer_sqrt_lower_bound(n in any::<u64>()) {
            let n128 = n as u128;
            let s = integer_sqrt(n128);
            prop_assert!((s + 1) * (s + 1) > n128);
        }

        #[test]
        fn tiered_distribution_deterministic(random_bytes in any::<[u8; 32]>()) {
            let a = map_to_tiered_distribution(&random_bytes);
            let b = map_to_tiered_distribution(&random_bytes);
            prop_assert_eq!(a, b);
        }

        #[test]
        fn derive_random_result_deterministic(seed in any::<[u8; 32]>(), index in any::<u32>()) {
            let a = derive_random_result(&seed, index);
            let b = derive_random_result(&seed, index);
            prop_assert_eq!(a, b);
        }

        #[test]
        fn derive_random_result_varies_by_index(seed in any::<[u8; 32]>()) {
            let a = derive_random_result(&seed, 0);
            let b = derive_random_result(&seed, 1);
            prop_assert_ne!(a, b);
        }

        #[test]
        fn tier1_amount_in_range(tier_roll in 0u64..TIER1_THRESHOLD, amount_entropy in any::<u64>(), tail in any::<[u8; 16]>()) {
            let random_bytes = build_random_bytes(tier_roll, amount_entropy, tail);
            let amount = map_to_tiered_distribution(&random_bytes);
            prop_assert!(amount >= TIER1_MIN_USD);
            prop_assert!(amount <= TIER1_MAX_USD); // 5.0-7.0 USDC (包含边界)
        }

        #[test]
        fn tier2_amount_in_range(tier_roll in TIER1_THRESHOLD..TIER2_THRESHOLD, amount_entropy in any::<u64>(), tail in any::<[u8; 16]>()) {
            let random_bytes = build_random_bytes(tier_roll, amount_entropy, tail);
            let amount = map_to_tiered_distribution(&random_bytes);
            prop_assert!(amount >= TIER2_MIN_USD);
            prop_assert!(amount <= TIER2_MAX_USD); // 7.0-14.0 USDC (包含边界)
        }

        #[test]
        fn tier3_amount_in_range(tier_roll in TIER2_THRESHOLD..TIER3_THRESHOLD, amount_entropy in any::<u64>(), tail in any::<[u8; 16]>()) {
            let random_bytes = build_random_bytes(tier_roll, amount_entropy, tail);
            let amount = map_to_tiered_distribution(&random_bytes);
            prop_assert!(amount >= TIER3_MIN_USD);
            prop_assert!(amount <= TIER3_MAX_USD); // 14.0-49.9 USDC (包含边界)
        }

        #[test]
        fn tier4_amount_in_range(tier_roll in TIER3_THRESHOLD..PROB_PRECISION, amount_entropy in any::<u64>(), tail in any::<[u8; 16]>()) {
            let random_bytes = build_random_bytes(tier_roll, amount_entropy, tail);
            let amount = map_to_tiered_distribution(&random_bytes);
            prop_assert!(amount >= TIER4_MIN_USD);
            prop_assert!(amount <= TIER4_MAX_USD); // 50.0-99.9 USDC (包含边界)
        }

        #[test]
        fn integer_sqrt_exact_squares(n in any::<u64>()) {
            let n128 = n as u128;
            let square = n128 * n128;
            prop_assert_eq!(integer_sqrt(square), n128);
        }

        #[test]
        fn integer_sqrt_near_squares(n in 2u64..=u64::MAX) {
            let n128 = n as u128;
            let square = n128 * n128 - 1;
            prop_assert_eq!(integer_sqrt(square), n128 - 1);
        }

        #[test]
        fn integer_sqrt_monotonic(n1 in any::<u64>(), n2 in any::<u64>()) {
            prop_assume!(n1 < n2);
            prop_assert!(integer_sqrt(n1 as u128) <= integer_sqrt(n2 as u128));
        }

        #[test]
        fn tier_and_amount_use_different_bytes(random_bytes in any::<[u8; 32]>()) {
            let mut modified = random_bytes;
            modified[0] = modified[0].wrapping_add(1);
            prop_assume!(tier_id(&random_bytes) == tier_id(&modified));
            let a = map_to_tiered_distribution(&random_bytes);
            let b = map_to_tiered_distribution(&modified);
            prop_assert_eq!(a, b);
        }

        #[test]
        fn pool_selection_uses_bytes_8_15(random_bytes in any::<[u8; 32]>(), pool_count in 1u8..=255) {
            let mut modified = random_bytes;
            modified[0] = modified[0].wrapping_add(1);
            let a = select_prize_pool(&random_bytes, pool_count);
            let b = select_prize_pool(&modified, pool_count);
            prop_assert_eq!(a, b);
        }

        // ==================== Task 2.11.1: 活跃池选择测试 ====================

        #[test]
        fn select_active_prize_pool_returns_valid_index(
            random_bytes in any::<[u8; 32]>(),
            active_pool_count in 1u8..=50,
        ) {
            // 构建 active_pool_indices (模拟有 active_pool_count 个活跃池)
            let mut active_pool_indices = [255u8; 50];
            for i in 0..active_pool_count as usize {
                active_pool_indices[i] = (i * 2) as u8; // 模拟索引: 0, 2, 4, 6, ...
            }

            let result = select_active_prize_pool(&random_bytes, active_pool_count, &active_pool_indices);

            // 结果必须是 active_pool_indices 中的有效值
            prop_assert!(active_pool_indices[0..active_pool_count as usize].contains(&result));
        }

        #[test]
        fn select_active_prize_pool_with_gaps(random_bytes in any::<[u8; 32]>()) {
            // 模拟删除后有间隙的情况: 活跃池索引为 [0, 2, 5]，池 1, 3, 4 已删除
            let mut active_pool_indices = [255u8; 50];
            active_pool_indices[0] = 0;
            active_pool_indices[1] = 2;
            active_pool_indices[2] = 5;
            let active_pool_count = 3u8;

            let result = select_active_prize_pool(&random_bytes, active_pool_count, &active_pool_indices);

            // 结果必须是 0, 2, 或 5
            prop_assert!(result == 0 || result == 2 || result == 5);
        }

        #[test]
        fn select_active_prize_pool_single_pool(random_bytes in any::<[u8; 32]>()) {
            // 只有一个活跃池
            let mut active_pool_indices = [255u8; 50];
            active_pool_indices[0] = 7; // 唯一的活跃池索引是 7
            let active_pool_count = 1u8;

            let result = select_active_prize_pool(&random_bytes, active_pool_count, &active_pool_indices);

            // 必须返回 7
            prop_assert_eq!(result, 7);
        }

        #[test]
        fn select_active_prize_pool_empty_returns_zero(random_bytes in any::<[u8; 32]>()) {
            // 没有活跃池
            let active_pool_indices = [255u8; 50];
            let active_pool_count = 0u8;

            let result = select_active_prize_pool(&random_bytes, active_pool_count, &active_pool_indices);

            // 没有活跃池时返回 0 (或使用默认值)
            prop_assert_eq!(result, 0);
        }

        #[test]
        fn select_active_prize_pool_deterministic(
            random_bytes in any::<[u8; 32]>(),
            active_pool_count in 1u8..=50,
        ) {
            let mut active_pool_indices = [255u8; 50];
            for i in 0..active_pool_count as usize {
                active_pool_indices[i] = i as u8;
            }

            let a = select_active_prize_pool(&random_bytes, active_pool_count, &active_pool_indices);
            let b = select_active_prize_pool(&random_bytes, active_pool_count, &active_pool_indices);

            prop_assert_eq!(a, b);
        }

        #[test]
        fn select_active_prize_pool_distribution_uniform(pool_count in 2u8..=10) {
            // 测试分布均匀性: 大量样本下每个池被选中的次数应接近
            let mut active_pool_indices = [255u8; 50];
            for i in 0..pool_count as usize {
                active_pool_indices[i] = i as u8;
            }

            let mut counts = [0u32; 10];
            let samples = 10000;

            for i in 0..samples {
                let mut random_bytes = [0u8; 32];
                // 使用 i 填充随机数 (模拟不同的 VRF 输出)
                random_bytes[8..16].copy_from_slice(&(i as u64).to_le_bytes());

                let result = select_active_prize_pool(&random_bytes, pool_count, &active_pool_indices);
                counts[result as usize] += 1;
            }

            // 每个池应该被选中约 samples / pool_count 次
            let expected = samples / pool_count as u32;
            let tolerance = expected / 5; // 允许 20% 偏差

            for i in 0..pool_count as usize {
                let diff = if counts[i] > expected {
                    counts[i] - expected
                } else {
                    expected - counts[i]
                };
                prop_assert!(diff <= tolerance, "Pool {} selected {} times, expected ~{}", i, counts[i], expected);
            }
        }
    }
}
