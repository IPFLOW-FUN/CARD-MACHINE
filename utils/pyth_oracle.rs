use crate::constants::{PYTH_SOL_USD_FEED_ID, SOL_DECIMALS, USD_PRECISION};
use crate::errors::IPFlowError;
use anchor_lang::prelude::*;
use pyth_solana_receiver_sdk::price_update::{Price, PriceUpdateV2};

/// 价格最大有效期（秒）- 超过此时间的价格视为陈旧
/// NOTE: Devnet 上 Pyth 更新频率较低，设置为 1 小时
/// 生产环境应改回 60 秒
const MAX_PRICE_AGE_SECONDS: u64 = 3600;

/// 将 micro-USD (10^6) 换算为 Lamports (10^9)
///
/// 计算公式:
/// lamports = (micro_usd / 10^6) * (1 / price_usd) * 10^9
/// 为了防止精度丢失，先乘后除:
/// lamports = (micro_usd * 10^9 * 10^price_expo) / (price * 10^6)
pub fn get_lamports_for_micro_usd(
    price_update: &PriceUpdateV2,
    micro_usd_amount: u64,
) -> Result<u64> {
    let clock = Clock::get()?;

    // 使用带时效校验的价格获取方法，防止陈旧价格攻击
    let current_price: Price = price_update
        .get_price_no_older_than(&clock, MAX_PRICE_AGE_SECONDS, &PYTH_SOL_USD_FEED_ID)
        .map_err(|_| error!(IPFlowError::PythPriceStale))?;

    // 校验价格为正数，防止无效价格
    require!(current_price.price > 0, IPFlowError::PythPriceInvalid);

    let price = current_price.price as u128;
    let expo = current_price.exponent.unsigned_abs();

    // 10^expo
    let scale_factor = 10u128
        .checked_pow(expo)
        .ok_or(error!(IPFlowError::MathOverflow))?;

    // 10^9 (SOL Decimals)
    let sol_scale = 10u128
        .checked_pow(SOL_DECIMALS)
        .ok_or(error!(IPFlowError::MathOverflow))?;

    // 分子: micro_usd * 10^9 * 10^expo
    let numerator = (micro_usd_amount as u128)
        .checked_mul(sol_scale)
        .ok_or(error!(IPFlowError::MathOverflow))?
        .checked_mul(scale_factor)
        .ok_or(error!(IPFlowError::MathOverflow))?;

    // 分母: price * 10^6 (USD_PRECISION)
    let denominator = price
        .checked_mul(USD_PRECISION as u128)
        .ok_or(error!(IPFlowError::MathOverflow))?;

    let lamports = numerator
        .checked_div(denominator)
        .ok_or(error!(IPFlowError::MathOverflow))?;

    Ok(lamports as u64)
}

/// 保留旧接口供 request_mint 使用 (5U 支付逻辑)
pub fn get_lamports_for_usd(price_update: &PriceUpdateV2, usd_amount: u64) -> Result<u64> {
    get_lamports_for_micro_usd(price_update, usd_amount * USD_PRECISION)
}
