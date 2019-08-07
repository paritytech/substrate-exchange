//! A shim for the substrate api providing a simplified interface for wallet
//! applications.
//!
#![deny(missing_docs)]
#![deny(warnings)]

use jsonrpc_core::IoHandler;
use jsonrpc_http_server::ServerBuilder;
use sr_primitives::generic::Era;
use substrate_subxt as subxt;
use substrate_exchange::{Exchange, Rpc, RpcImpl};

#[derive(Clone, PartialEq, Eq)]
struct Runtime;

impl srml_system::Trait for Runtime {
    type Origin = <node_runtime::Runtime as srml_system::Trait>::Origin;
    type Index = <node_runtime::Runtime as srml_system::Trait>::Index;
    type BlockNumber = <node_runtime::Runtime as srml_system::Trait>::BlockNumber;
    type Hash = <node_runtime::Runtime as srml_system::Trait>::Hash;
    type Hashing = <node_runtime::Runtime as srml_system::Trait>::Hashing;
    type AccountId = <node_runtime::Runtime as srml_system::Trait>::AccountId;
    type Lookup = <node_runtime::Runtime as srml_system::Trait>::Lookup;
    type WeightMultiplierUpdate =
        <node_runtime::Runtime as srml_system::Trait>::WeightMultiplierUpdate;
    type Header = <node_runtime::Runtime as srml_system::Trait>::Header;
    type Event = <node_runtime::Runtime as srml_system::Trait>::Event;
    type BlockHashCount = <node_runtime::Runtime as srml_system::Trait>::BlockHashCount;
    type MaximumBlockWeight = <node_runtime::Runtime as srml_system::Trait>::MaximumBlockWeight;
    type MaximumBlockLength = <node_runtime::Runtime as srml_system::Trait>::MaximumBlockLength;
    type AvailableBlockRatio = <node_runtime::Runtime as srml_system::Trait>::AvailableBlockRatio;
}

impl srml_balances::Trait for Runtime {
    type Balance = <node_runtime::Runtime as srml_balances::Trait>::Balance;
    type OnFreeBalanceZero = ();
    type OnNewAccount = ();
    type TransactionPayment = ();
    type TransferPayment = <node_runtime::Runtime as srml_balances::Trait>::TransferPayment;
    type DustRemoval = <node_runtime::Runtime as srml_balances::Trait>::DustRemoval;
    type Event = <node_runtime::Runtime as srml_balances::Trait>::Event;
    type ExistentialDeposit = <node_runtime::Runtime as srml_balances::Trait>::ExistentialDeposit;
    type TransferFee = <node_runtime::Runtime as srml_balances::Trait>::TransferFee;
    type CreationFee = <node_runtime::Runtime as srml_balances::Trait>::CreationFee;
    type TransactionBaseFee = <node_runtime::Runtime as srml_balances::Trait>::TransactionBaseFee;
    type TransactionByteFee = <node_runtime::Runtime as srml_balances::Trait>::TransactionByteFee;
    type WeightToFee = <node_runtime::Runtime as srml_balances::Trait>::WeightToFee;
}

impl Exchange for Runtime {
    type Pair = substrate_primitives::sr25519::Pair;
    type SignedExtra = (
        srml_system::CheckGenesis<Self>,
        srml_system::CheckEra<Self>,
        srml_system::CheckNonce<Self>,
        srml_system::CheckWeight<Self>,
        srml_balances::TakeFees<Self>,
    );

    fn extra(nonce: <Self as srml_system::Trait>::Index) -> Self::SignedExtra {
        (
            srml_system::CheckGenesis::<Runtime>::new(),
            srml_system::CheckEra::<Runtime>::from(Era::Immortal),
            srml_system::CheckNonce::<Runtime>::from(nonce),
            srml_system::CheckWeight::<Runtime>::new(),
            srml_balances::TakeFees::<Runtime>::from(0),
        )
    }
}

#[derive(Debug)]
enum Error {
    Io(std::io::Error),
    AddrParse(std::net::AddrParseError),
    UrlParse(url::ParseError),
    Subxt(subxt::Error),
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

impl From<subxt::Error> for Error {
    fn from(error: subxt::Error) -> Self {
        Error::Subxt(error)
    }
}

/// Check alices account balance with curl:
/// curl -X POST -H "content-type: application/json" -d
/// '{"jsonrpc":"2.0","id":0,"method":"account_balance","params":["5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"]}'
/// http://127.0.0.1:3030
/// Transfer 10 from alice to bob with curl:
/// curl -X POST -H "content-type: application/json" -d
/// '{"jsonrpc":"2.0","id":0,"method":"transfer_balance","params":["//Alice", "5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty", "0xa"]}'
/// http://127.0.0.1:3030
fn main() -> Result<(), Error> {
    env_logger::init();

    let addr = "127.0.0.1:3030".parse()?;
    log::info!("starting server at {}", addr);

    let url = "ws://127.0.0.1:9944".parse()?;
    log::info!("connecting to substrate at {}", url);

    let client = {
        let mut rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(
            subxt::ClientBuilder::<Runtime, <Runtime as Exchange>::SignedExtra>::new()
                .set_url(url)
                .build(),
        )?
    };
    let rpc = RpcImpl(client);
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
    use substrate_primitives::crypto::{Pair, Ss58Codec};

    fn key(keyring: Keyring) -> String {
        format!("//{}", <&'static str>::from(keyring))
    }

    fn pubkey(keyring: Keyring) -> String {
        keyring.pair().public().to_ss58check()
    }

    fn balance(amount: u128) -> String {
        amount.to_string()
    }

    fn rpc() -> RpcImpl<Runtime> {
        let client = {
            let mut rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(
                subxt::ClientBuilder::<Runtime, <Runtime as Exchange>::SignedExtra>::new().build(),
            )
            .unwrap()
        };
        RpcImpl(client)
    }

    #[test]
    fn test_account_balance() {
        let mut rt = tokio::runtime::Runtime::new().unwrap();
        let _: u128 = rt
            .block_on(rpc().account_balance(pubkey(Keyring::Alice)))
            .unwrap()
            .parse()
            .unwrap();
    }

    #[test]
    fn test_transfer_balance() {
        let mut rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(rpc().transfer_balance(key(Keyring::Alice), pubkey(Keyring::Bob), balance(10)))
            .unwrap();
    }
}
