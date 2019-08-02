//! A shim for the substrate api providing a simplified interface for wallet
//! applications.
//!
#![deny(missing_docs)]
#![deny(warnings)]

use jsonrpc_core::{Error as RpcError, IoHandler, Result as RpcResult};
use jsonrpc_derive::rpc;
use jsonrpc_http_server::ServerBuilder;
use std::str::FromStr;
use substrate_primitives::crypto::{Pair, Ss58Codec};

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
    #[rpc(name = "transfer_balance")]
    fn transfer_balance(&self, from: Private, to: Public, amount: Balance) -> RpcResult<()>;
}

/// The implementation of the Rpc trait.
pub struct RpcImpl;

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

    fn transfer_balance(&self, from: String, to: String, amount: String) -> RpcResult<()> {
        let _pair = Key::from_string(&from, None).map_err(|err| {
            RpcError::invalid_params_with_details(
                "Expected a suri encoded private key.",
                format!("{:?}", err),
            )
        })?;
        let _public = PubKey::from_string(&to).map_err(|err| {
            RpcError::invalid_params_with_details(
                "Expected a ss58 encoded public key.",
                format!("{:?}", err),
            )
        })?;
        let _balance = Balance::from_str(&amount).map_err(|err| {
            RpcError::invalid_params_with_details(
                "Expected a hex encoded balance.",
                format!("{:?}", err),
            )
        })?;
        Ok(())
    }
}

#[derive(Debug)]
enum Error {
    Io(std::io::Error),
    AddrParse(std::net::AddrParseError),
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

fn main() -> Result<(), Error> {
    let mut io = IoHandler::new();
    io.extend_with(RpcImpl.to_delegate());

    let addr = "127.0.0.1:3030".parse()?;
    println!("starting server at {}", addr);

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
        let result = RpcImpl
            .transfer_balance(key(Keyring::Alice), pubkey(Keyring::Bob), balance(10))
            .unwrap();
        assert_eq!(result, ());
    }
}
