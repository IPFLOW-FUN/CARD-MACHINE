use anchor_lang::prelude::*;

#[error_code]
pub enum IPFlowError {
    #[msg("Invalid Pyth price feed")]
    PythError,
    #[msg("Pyth price is stale (older than 60 seconds)")]
    PythPriceStale,
    #[msg("Pyth price is invalid (non-positive)")]
    PythPriceInvalid,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Program is paused")]
    ProgramPaused,
    #[msg("Invalid card amount, must be between 1 and 100")]
    InvalidCardAmount,
    #[msg("Invalid randomness account")]
    InvalidRandomnessAccount,
    #[msg("Switchboard commit failed")]
    SwitchboardCommitFailed,
    #[msg("Invalid reward distribution choice")]
    InvalidChoice,
    #[msg("Unauthorized access")]
    Unauthorized,
    #[msg("Invalid Switchboard account or program")]
    InvalidSwitchboardAccount,
    #[msg("Invalid USDT mint address")]
    InvalidUsdtMint,
    #[msg("Invalid token account")]
    InvalidTokenAccount,
    #[msg("Missing required accounts for USDT payment")]
    MissingUsdtAccounts,
    #[msg("Invalid request status for this operation")]
    InvalidRequestStatus,
    #[msg("Claim period has expired")]
    ClaimExpired,
    #[msg("Missing required accounts for Token swap")]
    MissingSwapAccounts,
    #[msg("Raydium swap failed")]
    SwapFailed,
    #[msg("Slippage exceeded")]
    SlippageExceeded,
    // ==================== Jupiter 相关错误码 (Task 1.16) ====================
    #[msg("Invalid Jupiter program ID")]
    InvalidJupiterProgram,
    #[msg("Missing expected token output for slippage protection")]
    MissingExpectedOutput,
    #[msg("Jupiter swap failed")]
    JupiterSwapFailed,
    #[msg("Invalid swap data: malformed or unsupported instruction")]
    InvalidSwapData,
    // ==================== Raydium 相关错误码 (Task 1.20) ====================
    #[msg("Invalid Raydium program ID")]
    InvalidRaydiumProgram,
    #[msg("Raydium swap failed")]
    RaydiumSwapFailed,
    // ==================== WSOL 相关错误码 ====================
    #[msg("WSOL wrap failed")]
    WsolWrapFailed,
    // ==================== Refund 相关错误码 (Task 2.3) ====================
    #[msg("Refund not allowed: request not timed out or already processed")]
    RefundNotAllowed,
    #[msg("Insufficient vault balance for refund")]
    InsufficientVaultBalance,
    // ==================== Prize Pool 相关错误码 (Task 3.3) ====================
    #[msg("Maximum prize pools reached")]
    MaxPrizePoolsReached,
    #[msg("Invalid prize pool index")]
    InvalidPrizePoolIndex,
    #[msg("No prize pool to remove")]
    NoPrizePoolToRemove,
    // ==================== MagicBlock VRF 相关错误码 ====================
    #[msg("Invalid slot: request_slot does not match current slot")]
    InvalidSlot,
    #[msg("Invalid VRF callback: unauthorized caller")]
    InvalidVrfCallback,
    #[msg("Invalid slot_hashes sysvar address")]
    InvalidSlotHashesSysvar,
    #[msg("Invalid oracle queue account")]
    InvalidOracleQueue,
    #[msg("Jupiter swap input exceeded maximum allowed amount")]
    ExcessiveSwapInput,
}
