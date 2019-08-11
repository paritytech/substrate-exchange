//! A shim for the substrate api providing a simplified interface for exchanges.
#![deny(missing_docs)]
#![deny(warnings)]

use futures::future::{self, Future};
use jsonrpc_core::Error as RpcError;
use jsonrpc_derive::rpc;
use parity_scale_codec::Codec;
use sr_primitives::traits::StaticLookup;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use substrate_primitives::crypto::{
    Pair,
    Ss58Codec,
};
use substrate_subxt::{
    Client,
    srml::{
        balances::{Balances, BalancesCalls, BalancesStore},
        system::System,
    }
};

/// Trait defining all chain specific data.
pub trait Exchange: System + Balances + 'static {
    /// Pair to use for signing.
    type Pair: Pair;
}

/// Error enum
pub enum Error {
    /// Invalid SURI.
    InvalidSURI,
    /// Invalid SS58.
    InvalidSS58,
    /// Invalid balance.
    InvalidBalance,
}

impl From<Error> for RpcError {
    fn from(err: Error) -> Self {
        let msg = match err {
            Error::InvalidSURI => "Expected a suri encoded private key.",
            Error::InvalidSS58 => "Expected a ss58 encoded public key.",
            Error::InvalidBalance => "Expected a numeric string.",
        };
        RpcError::invalid_params(msg)
    }
}

/// The rpc interface for wallet applications.
#[rpc(server)]
pub trait Rpc<T: Exchange> {
    /// Query the balance of an account.
    #[rpc(name = "account_balance", returns = "String")]
    fn account_balance(
        &self,
        from: String,
    ) -> Box<dyn Future<Item = String, Error = RpcError> + Send>;

    /// Transfer the given amount of balance from one account to an other.
    #[rpc(name = "transfer_balance", returns = "()")]
    fn transfer_balance(
        &self,
        from: String,
        to: String,
        amount: String,
    ) -> Box<dyn Future<Item = (), Error = RpcError> + Send>;
}

/// The implementation of the Rpc trait.
pub struct RpcImpl<T: Exchange> {
    client: Client<T>,
    nonces: Arc<Mutex<HashMap<T::AccountId, T::Index>>>,
}

impl<T: Exchange> RpcImpl<T>
where
    <T as System>::AccountId: std::hash::Hash,
{
    /// Creates a new `RpcImpl`.
    pub fn new(client: Client<T>) -> Self {
        Self { client, nonces: Default::default() }
    }
}


impl<T: Exchange> Rpc<T> for RpcImpl<T>
where
    <T as System>::AccountId: std::hash::Hash,
    <T::Pair as Pair>::Public:
        Ss58Codec
            + Into<<T as System>::AccountId>
            + Into<<<T as System>::Lookup as StaticLookup>::Source>,
    <T::Pair as Pair>::Signature: Codec,
    <T as Balances>::Balance: std::fmt::Display + std::str::FromStr,
    <<T as Balances>::Balance as std::str::FromStr>::Err: std::fmt::Debug,
{
    fn account_balance(
        &self,
        of: String,
    ) -> Box<dyn Future<Item = String, Error = RpcError> + Send> {
        let params = || {
            let public = <T::Pair as Pair>::Public::from_string(&of)
              .map_err(|_| Error::InvalidSS58)?;
            let result: Result<_, Error> = Ok(public);
            result
        };
        let public = match params() {
            Ok(params) => params,
            Err(err) => return Box::new(future::err(err.into())),
        };
        let free_balance = self.client.free_balance(public.into())
            .map(|balance| format!("{}", balance))
            .map_err(|e| {
                log::error!("{:?}", e);
                RpcError::internal_error()
            });
        Box::new(free_balance)
    }

    fn transfer_balance(
        &self,
        from: String,
        to: String,
        amount: String,
    ) -> Box<dyn Future<Item = (), Error = RpcError> + Send> {
        let params = || {
            let pair = T::Pair::from_string(&from, None)
              .map_err(|_| Error::InvalidSURI)?;
            let public = <T::Pair as Pair>::Public::from_string(&to)
            .map_err(|_| Error::InvalidSS58)?;
            let balance = amount.parse().map_err(|_| Error::InvalidBalance)?;
            let result: Result<_, Error> = Ok((pair, public, balance));
            result
        };
        let (pair, public, balance) = match params() {
            Ok(params) => params,
            Err(err) => return Box::new(future::err(err.into())),
        };
        let nonce = self.nonces.lock().unwrap().get(&pair.public().into()).cloned();
        let nonces = self.nonces.clone();
        let transfer = self.client.xt(pair.clone(), nonce)
            .and_then(move |mut xt| {
                let fut = xt.transfer(public.into(), balance);
                nonces.lock().unwrap()
                    .insert(pair.public().into(), xt.nonce());
                fut
            })
            .map(|hash| log::info!("{:?}", hash))
            .map_err(|e| {
                log::error!("{:?}", e);
                RpcError::internal_error()
            });
        Box::new(transfer)
    }
}
