//! A shim for the substrate api providing a simplified interface for wallet
//! applications.
//!
#![deny(missing_docs)]
#![deny(warnings)]

use balances_shim::rpc;
use futures::prelude::*;
use jsonrpc_core::{Error as RpcError, IoHandler, Result as RpcResult};
use jsonrpc_derive::rpc;
use jsonrpc_http_server::ServerBuilder;
use node_runtime::Call;
use srml_balances::Call as BalancesCall;
use std::str::FromStr;
use substrate_primitives::crypto::{Pair, Ss58Codec};
use url::Url;

/// The private key of an account.
pub type Key = substrate_primitives::sr25519::Pair;

/// The public key of an account.
pub type PubKey = substrate_primitives::sr25519::Public;

/// The balance of an account.
pub struct Balance(pub u128);

impl FromStr for Balance {
    type Err = std::num::ParseIntError;

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        let without_prefix = string.trim_start_matches("0x");
        Ok(Self(u128::from_str_radix(without_prefix, 16)?))
    }
}

impl From<Balance> for String {
    fn from(balance: Balance) -> Self {
        format!("0x{:x}", balance.0)
    }
}

impl From<u128> for Balance {
    fn from(balance: u128) -> Self {
        Balance(balance)
    }
}

impl From<Balance> for u128 {
    fn from(balance: Balance) -> Self {
        balance.0
    }
}

/// The rpc interface for wallet applications.
#[rpc]
pub trait Rpc<Balance, Private, Public> {
    /// Query the balance of an account.
    ///
    /// Check alices account balance with curl:
    /// curl -X POST -H "content-type: application/json" -d
    /// '{"jsonrpc":"2.0","id":0,"method":"account_balance","params":["5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"]}'
    /// http://127.0.0.1:3030
    #[rpc(name = "account_balance")]
    fn account_balance(&self, from: Public) -> RpcResult<Balance>;

    /// Transfer the given amount of balance from one account to an other.
    ///
    /// Transfer 10 from alice to bob with curl:
    /// curl -X POST -H "content-type: application/json" -d
    /// '{"jsonrpc":"2.0","id":0,"method":"transfer_balance","params":["//Alice", "5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty", "0xa"]}'
    /// http://127.0.0.1:3030
    #[rpc(name = "transfer_balance", returns = "()")]
    fn transfer_balance(
        &self,
        from: Private,
        to: Public,
        amount: Balance,
    ) -> Box<dyn Future<Item = (), Error = RpcError> + Send>;
}

/// The implementation of the Rpc trait.
pub struct RpcImpl {
    url: Url,
}

impl Rpc<String, String, String> for RpcImpl {
    fn account_balance(&self, of: String) -> RpcResult<String> {
        let _public = PubKey::from_string(&of).map_err(|err| {
            RpcError::invalid_params_with_details(
                "Expected a ss58 encoded public key.",
                format!("{:?}", err),
            )
        })?;
        let balance = Balance(0);
        Ok(balance.into())
    }

    fn transfer_balance(
        &self,
        from: String,
        to: String,
        amount: String,
    ) -> Box<dyn Future<Item = (), Error = RpcError> + Send> {
        let pair = match Key::from_string(&from, None) {
            Ok(pair) => pair,
            Err(err) => {
                return Box::new(futures::future::err(RpcError::invalid_params_with_details(
                    "Expected a suri encoded private key.",
                    format!("{:?}", err),
                )))
            }
        };
        let public = match PubKey::from_string(&to) {
            Ok(public) => public,
            Err(err) => {
                return Box::new(futures::future::err(RpcError::invalid_params_with_details(
                    "Expected a ss58 encoded public key.",
                    format!("{:?}", err),
                )))
            }
        };
        let balance = match Balance::from_str(&amount) {
            Ok(balance) => balance,
            Err(err) => {
                return Box::new(futures::future::err(RpcError::invalid_params_with_details(
                    "Expected a hex encoded balance.",
                    format!("{:?}", err),
                )))
            }
        };
        let call = Call::Balances(BalancesCall::transfer(public.into(), balance.into()));
        Box::new(rpc::submit(&self.url, pair, call).map(|_| ()).map_err(|e| {
            log::error!("{:?}", e);
            RpcError::internal_error()
        }))
    }
}

#[derive(Debug)]
enum Error {
    Io(std::io::Error),
    AddrParse(std::net::AddrParseError),
    UrlParse(url::ParseError),
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::Io(error)
    }
}

impl From<std::net::AddrParseError> for Error {
    fn from(error: std::net::AddrParseError) -> Self {
        Error::AddrParse(error)
    }
}

impl From<url::ParseError> for Error {
    fn from(error: url::ParseError) -> Self {
        Error::UrlParse(error)
    }
}

fn main() -> Result<(), Error> {
    env_logger::init();

    let addr = "127.0.0.1:3030".parse()?;
    log::info!("starting server at {}", addr);

    let url = "ws://127.0.0.1:9944".parse()?;
    log::info!("connecting to substrate at {}", url);

    let rpc = RpcImpl { url };
    let mut io = IoHandler::new();
    io.extend_with(rpc.to_delegate());

    let server = ServerBuilder::new(io).start_http(&addr)?;

    server.wait();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use substrate_keyring::sr25519::Keyring;

    fn key(keyring: Keyring) -> String {
        format!("//{}", <&'static str>::from(keyring))
    }

    fn pubkey(keyring: Keyring) -> String {
        keyring.pair().public().to_ss58check()
    }

    fn balance(amount: u128) -> String {
        Balance(amount).into()
    }

    #[test]
    fn test_account_balance() {
        let hex = RpcImpl.account_balance(pubkey(Keyring::Alice)).unwrap();
        let balance = Balance::from_str(&hex).unwrap();
        assert_eq!(u128::from(balance), 0);
    }

    #[test]
    fn test_transfer_balance() {
        let result =
            RpcImpl.transfer_balance(key(Keyring::Alice), pubkey(Keyring::Bob), balance(10));
        assert!(result.is_ok());
    }
}
