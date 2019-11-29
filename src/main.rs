//! A shim for the substrate api providing a simplified interface for wallet
//! applications.
#![deny(missing_docs)]
#![deny(warnings)]

use jsonrpc_core::IoHandler;
use jsonrpc_http_server::ServerBuilder;
use sr_primitives::generic::Era;
use substrate_exchange::{
    Exchange,
    Rpc,
    RpcImpl,
};
use substrate_subxt::{
    ClientBuilder,
    Error as SubError,
    balances::Balances,
    system::System,
};

struct Runtime;

impl System for Runtime {
    type Index = <node_runtime::Runtime as frame_system::Trait>::Index;
    type BlockNumber = <node_runtime::Runtime as frame_system::Trait>::BlockNumber;
    type Hash = <node_runtime::Runtime as frame_system::Trait>::Hash;
    type Hashing = <node_runtime::Runtime as frame_system::Trait>::Hashing;
    type AccountId = <node_runtime::Runtime as frame_system::Trait>::AccountId;
    type Lookup = <node_runtime::Runtime as frame_system::Trait>::Lookup;
    type Header = <node_runtime::Runtime as frame_system::Trait>::Header;
    type Event = <node_runtime::Runtime as frame_system::Trait>::Event;

    type SignedExtra = (
        frame_system::CheckGenesis<node_runtime::Runtime>,
        frame_system::CheckEra<node_runtime::Runtime>,
        frame_system::CheckNonce<node_runtime::Runtime>,
        frame_system::CheckWeight<node_runtime::Runtime>,
        pallet_balances::TakeFees<node_runtime::Runtime>,
    );

    fn extra(nonce: <Self as System>::Index) -> Self::SignedExtra {
        (
            frame_system::CheckGenesis::new(),
            frame_system::CheckEra::from(Era::Immortal),
            frame_system::CheckNonce::from(nonce),
            frame_system::CheckWeight::new(),
            pallet_balances::TakeFees::from(0),
        )
    }
}

impl Balances for Runtime {
    type Balance = <node_runtime::Runtime as pallet_balances::Trait>::Balance;
}

impl Exchange for Runtime {
    type Pair = substrate_primitives::sr25519::Pair;
}

#[derive(Debug)]
enum Error {
    Io(std::io::Error),
    AddrParse(std::net::AddrParseError),
    UrlParse(url::ParseError),
    Subxt(SubError),
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

impl From<SubError> for Error {
    fn from(error: SubError) -> Self {
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
        let client = ClientBuilder::<Runtime>::new()
            .set_url(url)
            .build();
        let mut rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(client)?
    };
    let rpc = RpcImpl::new(client);
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
    use substrate_primitives::crypto::{
        Pair,
        Ss58Codec,
    };

    fn key(keyring: Keyring) -> String {
        format!("//{}", <&'static str>::from(keyring))
    }

    fn pubkey(keyring: Keyring) -> String {
        keyring.pair().public().to_ss58check()
    }

    fn balance(amount: u128) -> String {
        amount.to_string()
    }

    fn account_balance(
        rt: &mut tokio::runtime::Runtime,
        rpc: &RpcImpl<Runtime>,
        of: Keyring,
    ) -> u128 {
        let balance = rpc.account_balance(pubkey(of));
        let result = rt.block_on(balance).unwrap();
        result.parse().unwrap()
    }

    fn transfer_balance(
        rt: &mut tokio::runtime::Runtime,
        rpc: &RpcImpl<Runtime>,
        from: Keyring,
        to: Keyring,
        amount: u128,
    ) {
        let transfer = rpc.transfer_balance(key(from), pubkey(to), balance(amount));
        rt.block_on(transfer).unwrap();
    }

    fn test_setup() -> (tokio::runtime::Runtime, RpcImpl<Runtime>) {
        env_logger::try_init().ok();
        let mut rt = tokio::runtime::Runtime::new().unwrap();
        let client = ClientBuilder::<Runtime>::new().build();
        let client = rt.block_on(client).unwrap();
        let rpc = RpcImpl::new(client);
        (rt, rpc)
    }

    #[test]
    fn test_account_balance() {
        let (mut rt, rpc) = test_setup();
        account_balance(&mut rt, &rpc, Keyring::Alice);
    }

    #[test]
    fn test_transfer_balance() {
        let (mut rt, rpc) = test_setup();
        transfer_balance(&mut rt, &rpc, Keyring::Alice, Keyring::Bob, 10_000);
    }

    #[test]
    //#[ignore] // need to wait for transaction to go through
    fn test_all() {
        let (mut rt, rpc) = test_setup();
        let from = Keyring::Alice;
        let to = Keyring::Bob;
        let amount = 10_000;

        let balance_from = account_balance(&mut rt, &rpc, from);
        let balance_to = account_balance(&mut rt, &rpc, to);
        transfer_balance(&mut rt, &rpc, from, to, amount);
        let balance_from2 = account_balance(&mut rt, &rpc, from);
        let balance_to2 = account_balance(&mut rt, &rpc, to);
        assert_eq!(balance_from, balance_from2 + amount);
        assert_eq!(balance_to + amount, balance_to2);
    }
}
