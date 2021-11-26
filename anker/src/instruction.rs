// TODO(#449): Remove this once Anker functions are all complete.
#![allow(dead_code)]

use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use lido::{accounts_struct, accounts_struct_meta, error::LidoError, token::StLamports};
use solana_program::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    pubkey::Pubkey,
    system_program, sysvar,
};

use crate::token::BLamports;

#[repr(C)]
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, BorshSchema)]
pub enum AnkerInstruction {
    Initialize,

    /// Deposit a given amount of StSOL, gets bSOL in return.
    ///
    /// This can be called by anybody.
    Deposit {
        #[allow(dead_code)] // but it's not
        amount: StLamports,
    },

    /// Withdraw a given amount of bSOL.
    ///
    /// Caller provides some `amount` of bLamports that are to be burned in
    /// order to withdraw stSOL.
    Withdraw {
        #[allow(dead_code)] // but it's not
        amount: BLamports,
    },

    /// Sell rewards to the UST reserve.
    SellRewards,
}

impl AnkerInstruction {
    pub fn to_vec(&self) -> Vec<u8> {
        // `BorshSerialize::try_to_vec` returns a Result, because it uses
        // `Borsh::serialize`, which takes an arbitrary writer, and which can
        // therefore return an IoError. But when serializing to a vec, there
        // is no IO, so for this particular writer, it should never fail.
        self.try_to_vec()
            .expect("Serializing an Instruction to Vec<u8> does not fail.")
    }
}

accounts_struct! {
    InitializeAccountsMeta, InitializeAccountsInfo {
        pub fund_rent_from {
            is_signer: true,
            is_writable: true, // It pays for the rent of the new accounts.
        },
        pub anker {
            is_signer: false,
            is_writable: true, // Writable because we need to initialize it.
        },
        pub solido {
            is_signer: false,
            is_writable: false,
        },
        pub solido_program {
            is_signer: false,
            is_writable: false,
        },
        pub st_sol_mint {
            is_signer: false,
            is_writable: false,
        },
        pub b_sol_mint {
            is_signer: false,
            is_writable: false,
        },
        pub st_sol_reserve_account {
            is_signer: false,
            is_writable: true, // Writable because we need to initialize it.
        },
        pub ust_reserve_account {
            is_signer: false,
            is_writable: true, // Writable because we need to initialize it.
        },
        pub reserve_authority {
            is_signer: false,
            is_writable: false,
        },
        // Instance of the token swap data used for trading StSOL for UST.
        pub token_swap_pool {
            is_signer: false,
            is_writable: false,
        },
        pub terra_rewards_destination {
            is_signer: false,
            is_writable: false,
        },
        pub ust_mint {
            is_signer: false,
            is_writable: false,
        },
        const sysvar_rent = sysvar::rent::id(),
        const system_program = system_program::id(),
        const spl_token = spl_token::id(),
    }
}

pub fn initialize(program_id: &Pubkey, accounts: &InitializeAccountsMeta) -> Instruction {
    let data = AnkerInstruction::Initialize;
    Instruction {
        program_id: *program_id,
        accounts: accounts.to_vec(),
        data: data.to_vec(),
    }
}

accounts_struct! {
    DepositAccountsMeta, DepositAccountsInfo {
        pub anker {
            is_signer: false,
            is_writable: false,
        },
        // For reading the stSOL/SOL exchange rate.
        pub solido {
            is_signer: false,
            is_writable: false,
        },
        pub from_account {
            is_signer: false,
            is_writable: true, // We will reduce its balance.
        },
        // Owner of `from_account` SPL token account.
        // Must sign the transaction in order to move tokens.
        pub user_authority {
            is_signer: true,
            is_writable: false,
        },
        pub to_reserve_account {
            is_signer: false,
            is_writable: true, // Needs to be writable to update the account's state.
        },
        // User account that will receive the bSOL tokens, needs to be writable
        // to update the account's state.
        pub b_sol_user_account {
            is_signer: false,
            is_writable: true,
        },
        pub b_sol_mint {
            is_signer: false,
            is_writable: true, // Minting changes the supply, which is stored in the mint.
        },
        pub b_sol_mint_authority {
            is_signer: false,
            is_writable: false,
        },
        const spl_token = spl_token::id(),
    }
}

pub fn deposit(
    program_id: &Pubkey,
    accounts: &DepositAccountsMeta,
    amount: StLamports,
) -> Instruction {
    let data = AnkerInstruction::Deposit { amount };
    Instruction {
        program_id: *program_id,
        accounts: accounts.to_vec(),
        data: data.to_vec(),
    }
}

accounts_struct! {
    WithdrawAccountsMeta, WithdrawAccountsInfo {
        pub anker {
            is_signer: false,
            is_writable: false,
        },
        // For reading the stSOL/SOL exchange rate.
        pub solido {
            is_signer: false,
            is_writable: false,
        },
        // SPL token account that holds the bSOL to return.
        pub from_b_sol_account {
            is_signer: false,
            is_writable: true, // We will decrease its balance.
        },
        // Owner of `from_b_sol_account` SPL token account.
        // Must sign the transaction in order to move tokens.
        pub from_b_sol_authority {
            is_signer: true,
            is_writable: false,
        },
        // Recipient of the proceeds, must be an SPL token account that holds stSOL.
        pub to_st_sol_account {
            is_signer: false,
            is_writable: true, // We will increase its balance.
        },
        // Anker's reserve, where the stSOL move out of.
        pub reserve_account {
            is_signer: false,
            is_writable: true, // We will decrease its balance.
        },
        // Owner of Anker's reserve, a program-derived address.
        pub reserve_authority {
            is_signer: false,
            is_writable: false,
        },
        pub b_sol_mint {
            is_signer: false,
            is_writable: true, // Burning bSOL changes the supply, which is stored in the mint.
        },
        const spl_token = spl_token::id(),
    }
}

pub fn withdraw(
    program_id: &Pubkey,
    accounts: &WithdrawAccountsMeta,
    amount: BLamports,
) -> Instruction {
    let data = AnkerInstruction::Withdraw { amount };
    Instruction {
        program_id: *program_id,
        accounts: accounts.to_vec(),
        data: data.to_vec(),
    }
}

accounts_struct! {
    SellRewardsAccountsMeta, SellRewardsAccountsInfo {
        pub anker {
            is_signer: false,
            is_writable: false,
        },
        pub solido {
            is_signer: false,
            is_writable: false,
        },
        // Needs to be writable so we can sell stSOL.
        pub st_sol_reserve_account {
            is_signer: false,
            is_writable: true, // Needed to swap tokens.
        },
        pub b_sol_mint {
            is_signer: false,
            is_writable: false,
        },

        // Accounts for token swap
        pub token_swap_pool {
            is_signer: false,
            is_writable: false,
        },
        pub st_sol_token {
            is_signer: false,
            is_writable: true, // Needed to swap tokens.
        },
        pub ust_token {
            is_signer: false,
            is_writable: true, // Needed to swap tokens.
        },
        pub pool_mint {
            is_signer: false,
            is_writable: true, // Needed to swap tokens.
        },
        pub st_sol_mint {
            is_signer: false,
            is_writable: false,
        },
        pub ust_mint {
            is_signer: false,
            is_writable: false,
        },
        pub pool_fee_account {
            is_signer: false,
            is_writable: true, // Needed to swap tokens.
        },
        pub token_pool_authority {
            is_signer: false,
            is_writable: false,
        },
        pub reserve_authority {
            is_signer: false,
            is_writable: false,
        },
        pub ust_reserve {
            is_signer: false,
            is_writable: true, // Needed to swap tokens.
        },

        const spl_token = spl_token::id(),
        const spl_token_swap = spl_token_swap::id(),
    }
}

pub fn sell_rewards(program_id: &Pubkey, accounts: &SellRewardsAccountsMeta) -> Instruction {
    let data = AnkerInstruction::SellRewards;
    Instruction {
        program_id: *program_id,
        accounts: accounts.to_vec(),
        data: data.to_vec(),
    }
}
