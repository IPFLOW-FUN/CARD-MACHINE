// ==================== 奖品池管理指令 (Task 3.3) ====================
//
// 独立 PDA + 硬删除 + 链上活跃索引列表
// VRF 选池流程 (链上):
//   position = vrf_result % active_pool_count
//   actual_index = active_pool_indices[position]
//   读取 PDA[actual_index] → swap_pool

use anchor_lang::prelude::*;

use crate::errors::IPFlowError;
use crate::events::{PrizePoolAdded, PrizePoolRemoved, PrizePoolUpdated};
use crate::state::global_config::MAX_PRIZE_POOLS;
use crate::state::PoolType;

/// 添加奖品池
///
/// 1. 创建 PrizePoolAccount PDA
/// 2. 将新索引追加到 active_pool_indices
/// 3. 更新 active_pool_count 和 prize_pool_count
pub fn add_prize_pool(
    ctx: Context<crate::AddPrizePool>,
    swap_pool: Pubkey,
    pool_type: PoolType,
    name: String,
) -> Result<()> {
    let config = &mut ctx.accounts.config;
    let prize_pool = &mut ctx.accounts.prize_pool;

    // 获取当前索引（下一个可用）
    let index = config.prize_pool_count;

    // 检查是否达到上限
    require!(
        (config.active_pool_count as usize) < MAX_PRIZE_POOLS,
        IPFlowError::MaxPrizePoolsReached
    );

    // 初始化 PrizePoolAccount
    prize_pool.index = index;
    prize_pool.swap_pool = swap_pool;
    prize_pool.pool_type = pool_type;
    prize_pool.name = name.clone();
    prize_pool.bump = ctx.bumps.prize_pool;

    // 更新 Config: 添加到活跃索引列表末尾
    let active_pos = config.active_pool_count as usize;
    config.active_pool_indices[active_pos] = index;
    config.active_pool_count += 1;
    config.prize_pool_count += 1;

    emit!(PrizePoolAdded {
        admin: ctx.accounts.admin.key(),
        index,
        swap_pool,
        pool_type,
        name,
    });

    msg!(
        "Prize pool added: index={}, swap_pool={}, name={}",
        index,
        swap_pool,
        prize_pool.name
    );

    Ok(())
}

/// 硬删除奖品池
///
/// 1. 找到 index 在 active_pool_indices 中的位置
/// 2. 将后续元素前移一位（填补空洞）
/// 3. 更新 active_pool_count
/// 4. PDA 通过 close = admin 自动关闭，租金退给 admin
///
/// 注意: prize_pool_count 不变（只增不减），用于分配新索引
pub fn remove_prize_pool(ctx: Context<crate::RemovePrizePool>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    let prize_pool = &ctx.accounts.prize_pool;
    let index = prize_pool.index;
    let swap_pool = prize_pool.swap_pool;

    require!(
        config.active_pool_count > 0,
        IPFlowError::NoPrizePoolToRemove
    );

    // 1. 找到 index 在 active_pool_indices 中的位置
    let mut found_pos: Option<usize> = None;
    for i in 0..(config.active_pool_count as usize) {
        if config.active_pool_indices[i] == index {
            found_pos = Some(i);
            break;
        }
    }
    let pos = found_pos.ok_or(IPFlowError::InvalidPrizePoolIndex)?;

    // 2. 将 pos 之后的元素前移一位
    let last_active = (config.active_pool_count - 1) as usize;
    for i in pos..last_active {
        config.active_pool_indices[i] = config.active_pool_indices[i + 1];
    }

    // 3. 清空最后一个位置，更新计数
    config.active_pool_indices[last_active] = 255; // 255 表示空位
    config.active_pool_count -= 1;
    // prize_pool_count 不变！只增不减

    emit!(PrizePoolRemoved {
        admin: ctx.accounts.admin.key(),
        index,
        swap_pool,
    });

    msg!(
        "Prize pool removed: index={}, active_pool_count={}",
        index,
        config.active_pool_count
    );

    // PDA 通过 close = admin 自动关闭，租金退给 admin
    Ok(())
}

/// 更新奖品池
///
/// 可选更新: swap_pool, pool_type, name
pub fn update_prize_pool(
    ctx: Context<crate::UpdatePrizePool>,
    swap_pool: Option<Pubkey>,
    pool_type: Option<PoolType>,
    name: Option<String>,
) -> Result<()> {
    let prize_pool = &mut ctx.accounts.prize_pool;
    let old_swap_pool = prize_pool.swap_pool;

    if let Some(sp) = swap_pool {
        prize_pool.swap_pool = sp;
    }
    if let Some(pt) = pool_type {
        prize_pool.pool_type = pt;
    }
    if let Some(n) = name {
        prize_pool.name = n;
    }

    emit!(PrizePoolUpdated {
        admin: ctx.accounts.admin.key(),
        index: prize_pool.index,
        old_swap_pool,
        new_swap_pool: prize_pool.swap_pool,
    });

    msg!(
        "Prize pool updated: index={}, swap_pool={}",
        prize_pool.index,
        prize_pool.swap_pool
    );

    Ok(())
}
