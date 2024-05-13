pub mod contract;

use crate::erc20::contract::ERC20Token;
use contract::{SingleSwap, SwapRouter, UniswapV3Factory, UniswapV3Pool, WETH9};
use revm::{
    db::PlainAccount,
    primitives::{fixed_bytes, uint, AccountInfo, Address, Bytes, TransactTo, TxEnv, B256, U256},
};

pub fn generate_cluster(
    num_people: usize,
    num_swaps_per_person: usize,
) -> (Vec<(Address, PlainAccount)>, Vec<TxEnv>) {
    let people_addresses: Vec<Address> = (0..num_people)
        .map(|_| Address::new(rand::random()))
        .collect();

    // make sure dai_address < usdc_address
    let (dai_address, usdc_address) = {
        let x = Address::new(rand::random());
        let y = Address::new(rand::random());
        (std::cmp::min(x, y), std::cmp::max(x, y))
    };

    let pool_init_code_hash = B256::new(rand::random());
    let swap_router_address = Address::new(rand::random());
    let single_swap_address = Address::new(rand::random());
    let weth9_address = Address::new(rand::random());
    let owner = Address::new(rand::random());
    let factory_address = Address::new(rand::random());
    let nonfungible_position_manager_address = Address::new(rand::random());
    let pool_address = UniswapV3Pool::new(dai_address, usdc_address, factory_address)
        .get_address(factory_address, pool_init_code_hash);

    let weth9_account = WETH9::new().build();

    let dai_account = ERC20Token::new("DAI", "DAI", 18, 222_222_000_000_000_000_000_000u128)
        .add_balances(&[pool_address], uint!(111_111_000_000_000_000_000_000_U256))
        .add_balances(&people_addresses, uint!(1_000_000_000_000_000_000_U256))
        .add_allowances(
            &people_addresses,
            single_swap_address,
            uint!(1_000_000_000_000_000_000_U256),
        )
        .build();

    let usdc_account = ERC20Token::new("USDC", "USDC", 18, 222_222_000_000_000_000_000_000u128)
        .add_balances(&[pool_address], uint!(111_111_000_000_000_000_000_000_U256))
        .add_balances(&people_addresses, uint!(1_000_000_000_000_000_000_U256))
        .add_allowances(
            &people_addresses,
            single_swap_address,
            uint!(1_000_000_000_000_000_000_U256),
        )
        .build();

    let factory_account = UniswapV3Factory::new(owner)
        .add_pool(dai_address, usdc_address, pool_address)
        .build(factory_address);

    let pool_account = UniswapV3Pool::new(dai_address, usdc_address, factory_address)
        .add_position(
            nonfungible_position_manager_address,
            -600000,
            600000,
            [
                uint!(0x00000000000000000000000000000000000000000000178756e190b388651605_U256),
                uint!(0x0000000000000000000000000000000000000000000000000000000000000000_U256),
                uint!(0x0000000000000000000000000000000000000000000000000000000000000000_U256),
                uint!(0x0000000000000000000000000000000000000000000000000000000000000000_U256),
            ],
        )
        .add_tick(
            -600000,
            [
                uint!(0x000000000000178756e190b388651605000000000000178756e190b388651605_U256),
                uint!(0x0000000000000000000000000000000000000000000000000000000000000000_U256),
                uint!(0x0000000000000000000000000000000000000000000000000000000000000000_U256),
                uint!(0x0100000001000000000000000000000000000000000000000000000000000000_U256),
            ],
        )
        .add_tick(
            600000,
            [
                uint!(0xffffffffffffe878a91e6f4c779ae9fb000000000000178756e190b388651605_U256),
                uint!(0x0000000000000000000000000000000000000000000000000000000000000000_U256),
                uint!(0x0000000000000000000000000000000000000000000000000000000000000000_U256),
                uint!(0x0100000000000000000000000000000000000000000000000000000000000000_U256),
            ],
        )
        .build(pool_address);

    let swap_router_account =
        SwapRouter::new(weth9_address, factory_address, pool_init_code_hash).build();

    let single_swap_account =
        SingleSwap::new(swap_router_address, dai_address, usdc_address).build();

    let mut state = Vec::from(&[
        (weth9_address, weth9_account),
        (dai_address, dai_account),
        (usdc_address, usdc_account),
        (factory_address, factory_account),
        (pool_address, pool_account),
        (swap_router_address, swap_router_account),
        (single_swap_address, single_swap_account),
    ]);

    for person in people_addresses.iter() {
        let info = AccountInfo::from_balance(uint!(4_567_000_000_000_000_000_000_U256));
        state.push((*person, PlainAccount::from(info)));
    }

    let mut txs = Vec::new();

    // sellToken0(uint256): c92b0891
    // sellToken1(uint256): 6b055260
    // buyToken0(uint256,uint256): 8dc33f82
    // buyToken1(uint256,uint256): b2db18a2
    for nonce in 0..num_swaps_per_person {
        for person in people_addresses.iter() {
            let data_bytes: Vec<u8> = match nonce % 4 {
                0 => [
                    &fixed_bytes!("c92b0891")[..],
                    &B256::from(U256::from(2000))[..],
                ]
                .concat(),
                1 => [
                    &fixed_bytes!("6b055260")[..],
                    &B256::from(U256::from(2000))[..],
                ]
                .concat(),
                2 => [
                    &fixed_bytes!("8dc33f82")[..],
                    &B256::from(U256::from(1000))[..],
                    &B256::from(U256::from(2000))[..],
                ]
                .concat(),
                3 => [
                    &fixed_bytes!("b2db18a2")[..],
                    &B256::from(U256::from(1000))[..],
                    &B256::from(U256::from(2000))[..],
                ]
                .concat(),
                _ => Default::default(),
            };

            txs.push(TxEnv {
                caller: *person,
                gas_limit: 16_777_216u64,
                gas_price: U256::from(0xb2d05e07u64),
                transact_to: TransactTo::Call(single_swap_address),
                value: U256::ZERO,
                data: Bytes::from(data_bytes),
                nonce: Some(nonce as u64),
                chain_id: None,
                access_list: Vec::new(),
                gas_priority_fee: None,
                blob_hashes: Vec::new(),
                max_fee_per_blob_gas: None,
            })
        }
    }

    (state, txs)
}

pub fn generate_clusters(
    num_clusters: usize,
    num_people_per_cluster: usize,
    num_swaps_per_person: usize,
) -> (Vec<(Address, PlainAccount)>, Vec<TxEnv>) {
    let mut final_state = Vec::new();
    let mut final_txs = Vec::new();

    for _ in 0..num_clusters {
        let (state, txs) = generate_cluster(num_people_per_cluster, num_swaps_per_person);
        final_state.extend(state);
        final_txs.extend(txs);
    }

    (final_state, final_txs)
}
