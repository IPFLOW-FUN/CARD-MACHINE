// ==================== Raydium CPMM CPI 集成模块 (独立实现版) ====================
use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::solana_program::program::invoke_signed;

use crate::constants::{RAYDIUM_CP_SWAP_PROGRAM, RAYDIUM_CP_SWAP_PROGRAM_DEVNET};
use crate::errors::IPFlowError;

/// SwapBaseInput 指令参数
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SwapBaseInputArgs {
    pub amount_in: u64,
    pub minimum_amount_out: u64,
}

#[allow(clippy::too_many_arguments)]
pub fn swap_base_input<'info>(
    cp_swap_program: AccountInfo<'info>,
    payer: AccountInfo<'info>,
    authority: AccountInfo<'info>,
    amm_config: AccountInfo<'info>,
    pool_state: AccountInfo<'info>,
    input_token_account: AccountInfo<'info>,
    output_token_account: AccountInfo<'info>,
    input_vault: AccountInfo<'info>,
    output_vault: AccountInfo<'info>,
    input_token_program: AccountInfo<'info>,
    output_token_program: AccountInfo<'info>,
    input_token_mint: AccountInfo<'info>,
    output_token_mint: AccountInfo<'info>,
    observation_state: AccountInfo<'info>,
    amount_in: u64,
    minimum_amount_out: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    // 0. 验证 Raydium CPMM Program ID (安全检查)
    require!(
        cp_swap_program.key() == RAYDIUM_CP_SWAP_PROGRAM
            || cp_swap_program.key() == RAYDIUM_CP_SWAP_PROGRAM_DEVNET,
        IPFlowError::InvalidRaydiumProgram
    );

    // 1. 构建指令数据
    // swap_base_input discriminator (Raydium CPMM Anchor IDL)
    let mut data = vec![143, 190, 90, 218, 196, 30, 51, 222];
    let args = SwapBaseInputArgs {
        amount_in,
        minimum_amount_out,
    };
    args.serialize(&mut data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    // 2. 构建账户列表
    let accounts = vec![
        AccountMeta::new(payer.key(), true),
        AccountMeta::new_readonly(authority.key(), false),
        AccountMeta::new_readonly(amm_config.key(), false),
        AccountMeta::new(pool_state.key(), false),
        AccountMeta::new(input_token_account.key(), false),
        AccountMeta::new(output_token_account.key(), false),
        AccountMeta::new(input_vault.key(), false),
        AccountMeta::new(output_vault.key(), false),
        AccountMeta::new_readonly(input_token_program.key(), false),
        AccountMeta::new_readonly(output_token_program.key(), false),
        AccountMeta::new_readonly(input_token_mint.key(), false),
        AccountMeta::new_readonly(output_token_mint.key(), false),
        AccountMeta::new(observation_state.key(), false),
    ];

    // 3. 构建指令
    let ix = Instruction {
        program_id: cp_swap_program.key(),
        accounts,
        data,
    };

    // 4. 执行调用
    invoke_signed(
        &ix,
        &[
            payer,
            authority,
            amm_config,
            pool_state,
            input_token_account,
            output_token_account,
            input_vault,
            output_vault,
            input_token_program,
            output_token_program,
            input_token_mint,
            output_token_mint,
            observation_state,
        ],
        signer_seeds,
    )
    .map_err(Into::into)
}
