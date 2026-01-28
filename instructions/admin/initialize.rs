use crate::Initialize;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::{program::invoke, system_instruction};
use anchor_lang::{AccountDeserialize, AccountSerialize};

use crate::constants::{ORACLE_QUEUE_DEVNET, REQUEST_TIMEOUT_SECONDS};
use crate::errors::IPFlowError;
use crate::state::global_config::MAX_PRIZE_POOLS;
use crate::state::IPFlowState;
use crate::MigrateConfig;
use crate::CloseConfig;

pub fn handler(ctx: Context<Initialize>, platform_fee_bps: u16) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.admin = ctx.accounts.admin.key(); // 管理员的公钥
    config.platform_fee_bps = platform_fee_bps; // 平台手续费，单位为 basis points (bps)
    config.is_paused = false; // 初始化时不暂停
    config.pool_count = 0; // 初始池数量为 0
    config.prize_pool_count = 0; // Task 3.3: 初始为 0，表示下一个可用索引
    config.active_pool_count = 0; // Task 3.3: 初始无活跃池
    config.active_pool_indices = [255u8; MAX_PRIZE_POOLS]; // Task 3.3: 255 表示空位
    config.total_collected = 0; // 初始总收集金额为 0
    config.oracle_queue = ORACLE_QUEUE_DEVNET; // 默认 VRF Queue 白名单
    config.request_timeout_seconds = REQUEST_TIMEOUT_SECONDS; // 默认退款超时

    // 获取 vault 的 bump
    let (_, vault_bump) = Pubkey::find_program_address(&[b"vault"], ctx.program_id);
    config.vault_bump = vault_bump; // 设置 vault 的 bump, 用于 PDA 生成,vlault 是资金归集账户

    Ok(())
}

/// 迁移/扩容全局配置账户
/// Task 3.3: 增加 active_pool_count 和 active_pool_indices 字段的初始化
/// CRITICAL FIX: 保留现有活跃池状态，避免迁移时丢失数据
pub fn migrate_config(ctx: Context<MigrateConfig>, prize_pool_count: u8) -> Result<()> {
    let config_info = ctx.accounts.config.to_account_info();
    let data = config_info.try_borrow_data()?;

    if data.len() < 40 {
        return Err(IPFlowError::Unauthorized.into());
    }

    let admin_bytes: [u8; 32] = data[8..40]
        .try_into()
        .map_err(|_| IPFlowError::Unauthorized)?;
    let admin_key = Pubkey::new_from_array(admin_bytes);
    require!(
        admin_key == ctx.accounts.admin.key(),
        IPFlowError::Unauthorized
    );

    drop(data);

    let new_space = 8 + IPFlowState::INIT_SPACE;
    let rent = Rent::get()?;
    let required_lamports = rent.minimum_balance(new_space);
    let current_lamports = **config_info.lamports.borrow();

    if current_lamports < required_lamports {
        let diff = required_lamports - current_lamports;
        invoke(
            &system_instruction::transfer(ctx.accounts.admin.key, config_info.key, diff),
            &[
                ctx.accounts.admin.to_account_info(),
                config_info.clone(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;
    }

    #[allow(deprecated)] // realloc 是当前唯一的账户扩容方式
    config_info.realloc(new_space, false)?;

    let mut data_mut = config_info.try_borrow_mut_data()?;
    let mut cursor: &[u8] = &data_mut;
    let mut config_state = IPFlowState::try_deserialize(&mut cursor)?;

    // 保存迁移前的活跃池状态
    let prev_active_count = config_state.active_pool_count;
    let prev_active_indices = config_state.active_pool_indices;

    // 更新 prize_pool_count
    config_state.prize_pool_count = prize_pool_count;

    // CRITICAL FIX: 保留现有活跃池配置
    // 只在首次迁移（字段为默认值）时初始化，否则保留原值
    // 检测条件：active_pool_count > 0 表示已有活跃池数据
    if prev_active_count > 0 {
        // 保留现有数据，不重置
        config_state.active_pool_count = prev_active_count;
        config_state.active_pool_indices = prev_active_indices;
        msg!(
            "Migrate config: prize_pool_count={}, preserved active_pool_count={}",
            prize_pool_count,
            prev_active_count
        );
    } else {
        // 首次迁移或无活跃池，初始化为默认值
        config_state.active_pool_count = 0;
        config_state.active_pool_indices = [255u8; MAX_PRIZE_POOLS];
        msg!(
            "Migrate config: prize_pool_count={}, initialized active_pool_count=0",
            prize_pool_count
        );
    }

    // 初始化新增配置字段（仅当为空时设置默认值）
    if config_state.oracle_queue == Pubkey::default() {
        config_state.oracle_queue = ORACLE_QUEUE_DEVNET;
    }
    if config_state.request_timeout_seconds == 0 {
        config_state.request_timeout_seconds = REQUEST_TIMEOUT_SECONDS;
    }

    let mut dst: &mut [u8] = &mut data_mut;
    config_state.try_serialize(&mut dst)?;

    Ok(())
}

/// 关闭全局配置账户（用于重新初始化）
/// 将账户 lamports 转回 admin，并清零数据
pub fn close_config(ctx: Context<CloseConfig>) -> Result<()> {
    let config_info = ctx.accounts.config.to_account_info();
    let data = config_info.try_borrow_data()?;

    // 校验 admin 权限（从原始字节读取）
    if data.len() < 40 {
        return Err(IPFlowError::Unauthorized.into());
    }

    let admin_bytes: [u8; 32] = data[8..40]
        .try_into()
        .map_err(|_| IPFlowError::Unauthorized)?;
    let admin_key = Pubkey::new_from_array(admin_bytes);
    require!(
        admin_key == ctx.accounts.admin.key(),
        IPFlowError::Unauthorized
    );

    drop(data);

    // 将 lamports 转回 admin
    let dest_starting_lamports = ctx.accounts.admin.lamports();
    **ctx.accounts.admin.lamports.borrow_mut() = dest_starting_lamports
        .checked_add(config_info.lamports())
        .ok_or(IPFlowError::MathOverflow)?;
    **config_info.lamports.borrow_mut() = 0;

    // 清零数据并设置 owner 为 system program
    let mut data_mut = config_info.try_borrow_mut_data()?;
    data_mut.fill(0);

    msg!("Config account closed, lamports returned to admin");
    Ok(())
}
