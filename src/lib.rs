//! A shim for the substrate api providing a simplified interface for exchanges.
#![deny(missing_docs)]
#![deny(warnings)]

use futures::prelude::*;
use jsonrpc_core::Error;
use jsonrpc_derive::rpc;
use parity_scale_codec::Codec;
use sr_primitives::traits::{
    SignedExtension,
    StaticLookup,
};
use srml_balances::Call;
use substrate_primitives::crypto::{
    Pair,
    Ss58Codec,
};
use substrate_subxt as subxt;

/// Trait defining all chain specific data.
pub trait Exchange: srml_system::Trait + srml_balances::Trait {
    /// Pair to use for signing.
    type Pair: Pair;
    /// Signed extra to include in transactions.
    type SignedExtra: SignedExtension;

    /// Constructs signed extra.
    fn extra(nonce: <Self as srml_system::Trait>::Index) -> Self::SignedExtra;
}

/// The rpc interface for wallet applications.
#[rpc(server)]
pub trait Rpc<T: Exchange> {
    /// Query the balance of an account.
    #[rpc(name = "account_balance", returns = "String")]
    fn account_balance(
        &self,
        from: String,
    ) -> Box<dyn Future<Item = String, Error = Error> + Send>;

    /// Transfer the given amount of balance from one account to an other.
    #[rpc(name = "transfer_balance", returns = "()")]
    fn transfer_balance(
        &self,
        from: String,
        to: String,
        amount: String,
    ) -> Box<dyn Future<Item = (), Error = Error> + Send>;
}

/// The implementation of the Rpc trait.
pub struct RpcImpl<T: Exchange>(pub subxt::Client<T, T::SignedExtra>);

impl<T: Exchange> Rpc<T> for RpcImpl<T>
where
    <T::Pair as Pair>::Public:
        Ss58Codec
            + Into<<T as srml_system::Trait>::AccountId>
            + Into<<<T as srml_system::Trait>::Lookup as StaticLookup>::Source>,
    <T::Pair as Pair>::Signature: Codec,
    <T as srml_balances::Trait>::Balance: std::fmt::Display + std::str::FromStr,
    <<T as srml_balances::Trait>::Balance as std::str::FromStr>::Err: std::fmt::Debug,
{
    fn account_balance(
        &self,
        of: String,
    ) -> Box<dyn Future<Item = String, Error = Error> + Send> {
        let public = match <T::Pair as Pair>::Public::from_string(&of) {
            Ok(public) => public,
            Err(err) => {
                return Box::new(futures::future::err(Error::invalid_params_with_details(
                    "Expected a ss58 encoded public key.",
                    format!("{:?}", err),
                )))
            }
        };
        let account: <T as srml_system::Trait>::AccountId = public.into();
        let account_balance_key = self
            .0
            .metadata()
            .module("Balances")
            .expect("runtime has srml_balances module")
            .storage("FreeBalance")
            .expect("srml_balances has a free balance")
            .map()
            .expect("free balance is a map")
            .key(&account);
        Box::new(
            self.0
                .fetch_or_default::<<T as srml_balances::Trait>::Balance>(
                    account_balance_key,
                )
                .map(|balance| format!("{}", balance))
                .map_err(|e| {
                    log::error!("{:?}", e);
                    Error::internal_error()
                }),
        )
    }

    fn transfer_balance(
        &self,
        from: String,
        to: String,
        amount: String,
    ) -> Box<dyn Future<Item = (), Error = Error> + Send> {
        let pair = match T::Pair::from_string(&from, None) {
            Ok(pair) => pair,
            Err(err) => {
                return Box::new(futures::future::err(Error::invalid_params_with_details(
                    "Expected a suri encoded private key.",
                    format!("{:?}", err),
                )))
            }
        };
        let public = match <T::Pair as Pair>::Public::from_string(&to) {
            Ok(public) => public,
            Err(err) => {
                return Box::new(futures::future::err(Error::invalid_params_with_details(
                    "Expected a ss58 encoded public key.",
                    format!("{:?}", err),
                )))
            }
        };
        let balance: Result<<T as srml_balances::Trait>::Balance, _> = amount.parse();
        let balance = match balance {
            Ok(balance) => balance,
            Err(err) => {
                return Box::new(futures::future::err(Error::invalid_params_with_details(
                    "Expected a hex encoded balance.",
                    format!("{:?}", err),
                )))
            }
        };
        let xt = self.0.xt(pair, T::extra);

        let transfer = Call::transfer::<T>(public.into(), balance.into());
        let call = self
            .0
            .metadata()
            .module("Balances")
            .expect("runtime has srml_balances module")
            .call(transfer);

        Box::new(
            xt.and_then(|xt| xt.submit(call))
            .map(|hash| log::info!("{:?}", hash))
            .map_err(|e| {
            log::error!("{:?}", e);
            Error::internal_error()
        }))
    }
}
