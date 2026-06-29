use soroban_sdk::{symbol_short, Address, Env};

#[allow(deprecated)]
pub fn publish_deploy(env: &Env, tag: soroban_sdk::Symbol, creator: &Address, address: &Address) {
    env.events().publish(
        (symbol_short!("deploy"), tag),
        (creator.clone(), address.clone()),
    );
}

/// Emitted when a deployment fee is successfully transferred to the treasury.
///
/// Topics: ("fee_coll", creator, treasury)
/// Data:   (amount: i128, currency: Address)
#[allow(deprecated)]
pub fn publish_deployment_fee_collected(
    env: &Env,
    creator: &Address,
    treasury: &Address,
    amount: i128,
    currency: &Address,
) {
    env.events().publish(
        (symbol_short!("fee_coll"), creator.clone(), treasury.clone()),
        (amount, currency.clone()),
    );
}
