//! Logic for keeping the stake pool balanced.

use crate::error::LidoError;
use crate::state::Validators;
use crate::token::Lamports;

/// Compute the ideal stake balance for each validator.
///
/// The validator order in `target_balance` is the same as in `current_balance`.
///
/// At the moment, this function targets a uniform distribution over all
/// validators. In the future we could do something more sophisticated (such as
/// allocating more stake to faster validators, or ones with a proven track
/// record).
pub fn get_target_balance(
    undelegated_lamports: Lamports,
    validators: &Validators,
    target_balance: &mut [Lamports],
) -> Result<(), LidoError> {
    assert_eq!(
        validators.len(),
        target_balance.len(),
        "Must have as many target balance outputs as current balance inputs."
    );

    let total_delegated_lamports: Option<Lamports> = validators
        .iter_entries()
        .map(|v| v.stake_accounts_balance)
        .sum();

    let total_lamports = total_delegated_lamports
        .and_then(|t| t + undelegated_lamports)
        .ok_or(LidoError::CalculationFailure)?;

    // We only want to target validators that are not in the process of being
    // removed. For now, those are all the validators. Once we add validator
    // removal, we need to take the removal flag into account here.
    let num_active_validators = validators.len() as u64;

    // We simply target a uniform distribution. If this causes division by
    // zero, that means there are no active validators.
    let target_balance_per_active_validator =
        (total_lamports / num_active_validators).ok_or(LidoError::NoActiveValidators)?;

    for target in target_balance.iter_mut() {
        *target = target_balance_per_active_validator;
    }

    // The total lamports to distribute may be slightly larger than the total
    // lamports we distributed so far, because we round down.
    let total_lamports_distributed = target_balance
        .iter()
        .cloned()
        .sum::<Option<Lamports>>()
        .expect("Does not overflow, is at most total_lamports.");

    let mut remainder = (total_lamports - total_lamports_distributed)
        .expect("Does not underflow because we distribute at most total_lamports.");

    assert!(remainder.0 < num_active_validators);

    // Distribute the remainder among the first few validators, give them one
    // Lamport each.
    for target in target_balance.iter_mut() {
        if remainder == Lamports(0) {
            break;
        }
        *target = (*target + Lamports(1))
            .expect("Does not overflow because per-validator balance is at most total_lamports.");
        remainder = (remainder - Lamports(1)).expect("Does not underflow due to loop condition.");
    }

    // Sanity check: now we should have distributed all inputs.
    let total_lamports_distributed = target_balance
        .iter()
        .cloned()
        .sum::<Option<Lamports>>()
        .expect("Does not overflow, is at most total_lamports.");
    assert_eq!(total_lamports_distributed, total_lamports);

    Ok(())
}

/// Given a list of validators and their target balance, return the index of the
/// one furthest below its target, and the amount by which it is below.
pub fn get_validator_furthest_below_target(
    validators: &Validators,
    target_balance: &[Lamports],
) -> (usize, Lamports) {
    assert_eq!(
        validators.len(),
        target_balance.len(),
        "Must have as many target balances as current balances."
    );
    let mut index = 0;
    let mut amount = Lamports(0);

    for (i, (validator, target)) in validators.iter_entries().zip(target_balance).enumerate() {
        let amount_below = Lamports(target.0.saturating_sub(validator.stake_accounts_balance.0));
        if amount_below > amount {
            amount = amount_below;
            index = i;
        }
    }

    (index, amount)
}

#[cfg(test)]
mod test {
    use super::{get_target_balance, get_validator_furthest_below_target};
    use crate::state::Validators;
    use crate::token::Lamports;

    #[test]
    fn get_target_balance_works_for_single_validator() {
        // 100 Lamports delegated + 50 undelegated => 150 per validator target.
        let mut validators = Validators::new_fill_default(1);
        validators.entries[0].entry.stake_accounts_balance = Lamports(100);
        let mut targets = [Lamports(0); 1];
        let undelegated_stake = Lamports(50);
        let result = get_target_balance(undelegated_stake, &validators, &mut targets[..]);
        assert!(result.is_ok());
        assert_eq!(targets[0], Lamports(150));

        // With only one validator, that one is the least balanced. It is
        // missing the 50 undelegated Lamports.
        assert_eq!(
            get_validator_furthest_below_target(&validators, &targets[..]),
            (0, Lamports(50))
        );
    }

    #[test]
    fn get_target_balance_works_for_integer_multiple() {
        // 200 Lamports delegated + 50 undelegated => 125 per validator target.
        let mut validators = Validators::new_fill_default(2);
        validators.entries[0].entry.stake_accounts_balance = Lamports(101);
        validators.entries[1].entry.stake_accounts_balance = Lamports(99);

        let mut targets = [Lamports(0); 2];
        let undelegated_stake = Lamports(50);
        let result = get_target_balance(undelegated_stake, &validators, &mut targets[..]);
        assert!(result.is_ok());
        assert_eq!(targets, [Lamports(125), Lamports(125)]);

        // The second validator is further away from its target.
        assert_eq!(
            get_validator_furthest_below_target(&validators, &targets[..]),
            (1, Lamports(26))
        );
    }

    #[test]
    fn get_target_balance_works_for_non_integer_multiple() {
        // 200 Lamports delegated + 51 undelegated => 125 per validator target,
        // and one validator gets 1 more.
        let mut validators = Validators::new_fill_default(2);
        validators.entries[0].entry.stake_accounts_balance = Lamports(101);
        validators.entries[1].entry.stake_accounts_balance = Lamports(99);

        let mut targets = [Lamports(0); 2];
        let undelegated_stake = Lamports(51);
        let result = get_target_balance(undelegated_stake, &validators, &mut targets[..]);
        assert!(result.is_ok());
        assert_eq!(targets, [Lamports(126), Lamports(125)]);

        // The second validator is further from its target, by one Lamport.
        assert_eq!(
            get_validator_furthest_below_target(&validators, &targets[..]),
            (1, Lamports(26))
        );
    }

    #[test]
    fn get_target_balance_already_balanced() {
        // 200 Lamports delegated, but only one active validator,
        // so all of the target should be with that one validator.
        let mut validators = Validators::new_fill_default(2);
        validators.entries[0].entry.stake_accounts_balance = Lamports(50);
        validators.entries[1].entry.stake_accounts_balance = Lamports(50);

        let mut targets = [Lamports(0); 2];
        let undelegated_stake = Lamports(0);
        let result = get_target_balance(undelegated_stake, &validators, &mut targets[..]);
        assert!(result.is_ok());
        assert_eq!(targets, [Lamports(50), Lamports(50)]);

        assert_eq!(
            get_validator_furthest_below_target(&validators, &targets[..]),
            (0, Lamports(0))
        );
    }
}
