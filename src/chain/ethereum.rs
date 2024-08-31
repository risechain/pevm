//! Ethereum

use std::{
    collections::{BTreeMap, HashMap},
    fmt::Debug,
};

use ahash::HashSet;
use alloy_chains::NamedChain;
use alloy_consensus::{ReceiptEnvelope, TxType};
use alloy_primitives::{address, Address, TxKind, B256, U256};
use alloy_provider::network::eip2718::Encodable2718;
use alloy_rpc_types::{BlockTransactions, Header, Transaction};
use once_cell::sync::Lazy;
use revm::{
    primitives::{BlockEnv, SpecId, TxEnv},
    Handler,
};

use super::{PevmChain, RewardPolicy};
use crate::{
    mv_memory::MvMemory, BuildIdentityHasher, MemoryLocation, PevmTxExecutionResult, TxIdx,
};

/// Implementation of [PevmChain] for Ethereum
#[derive(Debug, Clone, PartialEq)]
pub struct PevmEthereum {
    id: u64,
}

impl PevmEthereum {
    /// Ethereum Mainnet
    pub fn mainnet() -> Self {
        Self {
            id: NamedChain::Mainnet.into(),
        }
    }

    // TODO: support Ethereum Sepolia and other testnets
}

/// Error type for [PevmEthereum::get_block_spec].
#[derive(Debug, Clone, PartialEq)]
pub enum EthereumBlockSpecError {
    /// When [header.number] is none.
    MissingBlockNumber,
    /// When [header.total_difficulty] is none.
    MissingTotalDifficulty,
}

/// Error type for [PevmEthereum::get_gas_price].
#[derive(Debug, Clone, PartialEq)]
pub enum EthereumGasPriceError {
    /// [tx.type] is invalid.
    InvalidType(u8),
    /// [tx.gas_price] is none.
    MissingGasPrice,
    /// [tx.max_fee_per_gas] is none.
    MissingMaxFeePerGas,
}

impl PevmChain for PevmEthereum {
    type BlockSpecError = EthereumBlockSpecError;
    type GasPriceError = EthereumGasPriceError;

    fn id(&self) -> u64 {
        self.id
    }

    /// Get the REVM spec id of an Alloy block.
    // Currently hardcoding Ethereum hardforks from these reference:
    // https://github.com/paradigmxyz/reth/blob/4fa627736681289ba899b38f1c7a97d9fcf33dc6/crates/primitives/src/revm/config.rs#L33-L78
    // https://github.com/paradigmxyz/reth/blob/4fa627736681289ba899b38f1c7a97d9fcf33dc6/crates/primitives/src/chain/spec.rs#L44-L68
    // TODO: Better error handling & properly test this.
    // TODO: Only Ethereum Mainnet is supported at the moment.
    fn get_block_spec(&self, header: &Header) -> Result<SpecId, Self::BlockSpecError> {
        let number = header
            .number
            .ok_or(EthereumBlockSpecError::MissingBlockNumber)?;
        let total_difficulty = header
            .total_difficulty
            .ok_or(EthereumBlockSpecError::MissingTotalDifficulty)?;

        Ok(if header.timestamp >= 1710338135 {
            SpecId::CANCUN
        } else if header.timestamp >= 1681338455 {
            SpecId::SHANGHAI
        } else if total_difficulty.saturating_sub(header.difficulty)
            >= U256::from(58_750_000_000_000_000_000_000_u128)
        {
            SpecId::MERGE
        } else if number >= 12965000 {
            SpecId::LONDON
        } else if number >= 12244000 {
            SpecId::BERLIN
        } else if number >= 9069000 {
            SpecId::ISTANBUL
        } else if number >= 7280000 {
            SpecId::PETERSBURG
        } else if number >= 4370000 {
            SpecId::BYZANTIUM
        } else if number >= 2675000 {
            SpecId::SPURIOUS_DRAGON
        } else if number >= 2463000 {
            SpecId::TANGERINE
        } else if number >= 1150000 {
            SpecId::HOMESTEAD
        } else {
            SpecId::FRONTIER
        })
    }

    fn get_gas_price(&self, tx: &Transaction) -> Result<U256, Self::GasPriceError> {
        let tx_type_raw: u8 = tx.transaction_type.unwrap_or_default();
        let Ok(tx_type) = TxType::try_from(tx_type_raw) else {
            return Err(EthereumGasPriceError::InvalidType(tx_type_raw));
        };

        match tx_type {
            TxType::Legacy | TxType::Eip2930 => tx
                .gas_price
                .map(U256::from)
                .ok_or(EthereumGasPriceError::MissingGasPrice),
            TxType::Eip1559 | TxType::Eip4844 => tx
                .max_fee_per_gas
                .map(U256::from)
                .ok_or(EthereumGasPriceError::MissingMaxFeePerGas),
        }
    }

    fn build_mv_memory(
        &self,
        hasher: &ahash::RandomState,
        block_env: &BlockEnv,
        txs: &[TxEnv],
    ) -> MvMemory {
        let block_size = txs.len();
        let beneficiary_location_hash = hasher.hash_one(MemoryLocation::Basic(block_env.coinbase));

        // TODO: Estimate more locations based on sender, to, etc.
        let mut estimated_locations = HashMap::with_hasher(BuildIdentityHasher::default());
        estimated_locations.insert(
            beneficiary_location_hash,
            (0..block_size).collect::<Vec<TxIdx>>(),
        );

        MvMemory::new(
            block_size,
            estimated_locations,
            [MemoryLocation::Basic(block_env.coinbase)],
        )
    }

    fn get_handler<'a, EXT, DB: revm::Database>(
        &self,
        spec_id: SpecId,
        with_reward_beneficiary: bool,
    ) -> Handler<'a, revm::Context<EXT, DB>, EXT, DB> {
        Handler::mainnet_with_spec(spec_id, with_reward_beneficiary)
    }

    fn get_reward_policy(&self, _hasher: &ahash::RandomState) -> RewardPolicy {
        RewardPolicy::Ethereum
    }

    // Refer to section 4.3.2. Holistic Validity in the Ethereum Yellow Paper.
    // https://github.com/ethereum/go-ethereum/blob/master/cmd/era/main.go#L289
    fn calculate_receipt_root(
        &self,
        _spec_id: SpecId,
        txs: &BlockTransactions<Transaction>,
        tx_results: &[PevmTxExecutionResult],
    ) -> B256 {
        // 1. Create an iterator of ReceiptEnvelope
        let tx_type_iter = txs
            .txns()
            .map(|tx| TxType::try_from(tx.transaction_type.unwrap_or_default()).unwrap());

        let receipt_iter = tx_results.iter().map(|tx| tx.receipt.clone().with_bloom());

        let receipt_envelope_iter =
            Iterator::zip(tx_type_iter, receipt_iter).map(|(tx_type, receipt)| match tx_type {
                TxType::Legacy => ReceiptEnvelope::Legacy(receipt),
                TxType::Eip2930 => ReceiptEnvelope::Eip2930(receipt),
                TxType::Eip1559 => ReceiptEnvelope::Eip1559(receipt),
                TxType::Eip4844 => ReceiptEnvelope::Eip4844(receipt),
            });

        // 2. Create a trie then calculate the root hash
        // We use BTreeMap because the keys must be sorted in ascending order.
        let trie_entries: BTreeMap<_, _> = receipt_envelope_iter
            .enumerate()
            .map(|(index, receipt)| {
                let key_buffer = alloy_rlp::encode_fixed_size(&index);
                let mut value_buffer = Vec::new();
                receipt.encode_2718(&mut value_buffer);
                (key_buffer, value_buffer)
            })
            .collect();

        let mut hash_builder = alloy_trie::HashBuilder::default();
        for (k, v) in trie_entries {
            hash_builder.add_leaf(alloy_trie::Nibbles::unpack(&k), &v);
        }
        hash_builder.root()
    }

    fn is_erc20_transfer(&self, tx: &TxEnv) -> bool {
        let TxKind::Call(contract_address) = tx.transact_to else {
            return false;
        };

        tx.data.len() == 4 + 32 + 32
            && tx.data.starts_with(&[0xa9, 0x05, 0x9c, 0xbb])
            && ERC20_KNOWN_ADDRESSES.contains(&contract_address)
    }
}

static ERC20_KNOWN_ADDRESSES: Lazy<HashSet<Address>> = Lazy::new(|| {
    HashSet::from_iter([
        address!("038a68ff68c393373ec894015816e33ad41bd564"),
        address!("0391d2021f89dc339f60fff84546ea23e337750f"),
        address!("043c308bb8a5ae96d0093444be7f56459f1340b1"),
        address!("059956483753947536204e89bfad909e1a434cc6"),
        address!("06450dee7fd2fb8e39061434babcfc05599a6fb8"),
        address!("07c52c2537d84e532a9f15d32e152c8b94d2b232"),
        address!("07d0ae1aa81a747fe98fec0cb7e5e5f3ccafb589"),
        address!("07d9e49ea402194bf48a8276dafb16e4ed633317"),
        address!("090185f2135308bad17527004364ebcc2d37e5f6"),
        address!("09a3ecafa817268f77be1283176b946c4ff2e608"),
        address!("0b38210ea11411557c13457d4da7dc6ea731b88a"),
        address!("0bc529c00c6401aef6d220be8c6ea1667f6ad93e"),
        address!("0d152b9ee87ebae179f64c067a966dd716c50742"),
        address!("0f51bb10119727a7e5ea3538074fb341f56b09ad"),
        address!("10633216e7e8281e33c86f02bf8e565a635d9770"),
        address!("111111111117dc0aa78b770fa6a738034120c302"),
        address!("14d312ac2bfc95d9bbefa87deb1d3cfcf69980de"),
        address!("15d4c048f83bd7e37d49ea4c83a07267ec4203da"),
        address!("163f8c2467924be0ae7b5347228cabf260318753"),
        address!("16484d73ac08d2355f466d448d2b79d2039f6ebb"),
        address!("17e67d1cb4e349b9ca4bc3e17c7df2a397a7bb64"),
        address!("18084be33d80c3fdf6e7e7deab4a4e5e26657331"),
        address!("1ce270557c1f68cfb577b856766310bf8b47fd9c"),
        address!("249e38ea4102d0cf8264d3701f1a0e39c4f2dc3b"),
        address!("25f8087ead173b73d6e8b84329989a8eea16cf73"),
        address!("26c8afbbfe1ebaca03c2bb082e69d0476bffe099"),
        address!("2b591e99afe9f32eaa6214f7b7629768c40eeb39"),
        address!("2e65e12b5f0fd1d58738c6f38da7d57f5f183d1c"),
        address!("2f1364d1af83afa748a452579b09d69ee83e8c86"),
        address!("3106a0a076bedae847652f42ef07fd58589e001f"),
        address!("31c2415c946928e9fd1af83cdfa38d3edbd4326f"),
        address!("36e66fbbce51e4cd5bd3c62b637eb411b18949d4"),
        address!("3b484b82567a09e2588a13d54d032153f0c0aee0"),
        address!("3f17dd476faf0a4855572f0b6ed5115d9bba22ad"),
        address!("41958d44a780696a2f18b7ac3585eae9bbbf0799"),
        address!("4575f41308ec1483f3d399aa9a2826d74da13deb"),
        address!("467bccd9d29f223bce8043b84e8c8b282827790f"),
        address!("469861bdfd02e7ebce7cdeb281e8eec53069cf5f"),
        address!("473037de59cf9484632f4a27b509cfe8d4a31404"),
        address!("485d17a6f1b8780392d53d64751824253011a260"),
        address!("491604c0fdf08347dd1fa4ee062a822a5dd06b5d"),
        address!("49e833337ece7afe375e44f4e3e8481029218e5c"),
        address!("4a220e6096b25eadb88358cb44068a3248254675"),
        address!("4d224452801aced8b2f0aebe155379bb5d594381"),
        address!("4ed25d8577feb83946b1548998fb7b157ead8bb9"),
        address!("4fe83213d56308330ec302a8bd641f1d0113a4cc"),
        address!("505b5eda5e25a67e1c24a2bf1a527ed9eb88bf04"),
        address!("50d1c9771902476076ecfc8b2a83ad6b9355a4c9"),
        address!("510975eda48a97e0ca228dd04d1217292487bea6"),
        address!("51521d62843c4edd90178658ab6e3eb9a4290fca"),
        address!("516e5436bafdc11083654de7bb9b95382d08d5de"),
        address!("51e00a95748dbd2a3f47bc5c3b3e7b3f0fea666c"),
        address!("5218e472cfcfe0b64a064f055b43b4cdc9efd3a6"),
        address!("549020a9cb845220d66d3e9c6d9f9ef61c981102"),
        address!("55652ce84d686177c8946e8c78078c0d6cfa4b30"),
        address!("582d872a1b094fc48f5de31d3b73f2d9be47def1"),
        address!("595832f8fc6bf59c85c527fec3740a1b7a361269"),
        address!("5ca381bbfb58f0092df149bd3d243b08b9a8386e"),
        address!("6468e79a80c0eab0f9a2b574c8d5bc374af59414"),
        address!("68291426e498af5eb7bff96bf613736fede7702b"),
        address!("69af81e73a73b40adf4f3d4223cd9b1ece623074"),
        address!("6b1a8f210ec6b7b6643cea3583fb0c079f367898"),
        address!("6cd3cbfa29ebb63e84132ad7b1a10407aba30acd"),
        address!("6d614686550b9e1c1df4b2cd8f91c9d4df66c810"),
        address!("6f259637dcd74c767781e37bc6133cd6a68aa161"),
        address!("70bc0dc6414eb8974bc70685f798838a87d8cce4"),
        address!("728f30fa2f100742c7949d1961804fa8e0b1387d"),
        address!("7480cf39529ab04d4968495f1d6eb0d232bc4790"),
        address!("761d38e5ddf6ccf6cf7c55759d5210750b5d60f3"),
        address!("7a58c0be72be218b41c608b7fe7c5bb630736c71"),
        address!("7de91b204c1c737bcee6f000aaa6569cf7061cb7"),
        address!("7e291890b01e5181f7ecc98d79ffbe12ad23df9e"),
        address!("8080b66e7505db9bd1d7bb44a7b9518754c8d26b"),
        address!("826180541412d574cf1336d22c0c0a287822678a"),
        address!("8290333cef9e6d528dd5618fb97a76f268f3edd4"),
        address!("853d955acef822db058eb8505911ed77f175b99e"),
        address!("85eee30c52b0b379b046fb0f85f4f3dc3009afec"),
        address!("888888888889c00c67689029d7856aac1065ec11"),
        address!("8f693ca8d21b157107184d29d398a8d082b38b76"),
        address!("9287aefe51047ef43f68612f80bac17745b23aec"),
        address!("9403ca0f802c3cc1a45372e44cdd7ed0e5cd1a04"),
        address!("95ad61b0a150d79219dcf64e1e6cc01f0b64c4ce"),
        address!("966daed1348fbd894bb6c404d9cddf78a9932913"),
        address!("9b4e2b4b13d125238aa0480dd42b4f6fc71b37cc"),
        address!("9e32b13ce7f2e80a01932b42553652e053d6ed8e"),
        address!("a0008f510fe9ee696e7e320c9e5cbf61e27791ee"),
        address!("a0ef786bf476fe0810408caba05e536ac800ff86"),
        address!("a130e3a33a4d84b04c3918c4e5762223ae252f80"),
        address!("a1edc78199a6e56fd52f69cf7c10f67ded15185d"),
        address!("a444ec96ee01bb219a44b285de47bf33c3447ad5"),
        address!("a45fdac9dc5673db72e64ae2c4b86e670db0187d"),
        address!("a47c8bf37f92abed4a126bda807a7b7498661acd"),
        address!("a7d10ff962eda41f3b037e3af1d8b4037eba4b86"),
        address!("a86a0da9d05d0771955df05b44ca120661af16de"),
        address!("a9b1eb5908cfc3cdf91f9b8b3a74108598009096"),
        address!("ac51066d7bec65dc4589368da368b212745d63e8"),
        address!("af5191b0de278c7286d6c7cc6ab6bb8a73ba2cd6"),
        address!("b131f4a55907b10d1f0a50d8ab8fa09ec342cd74"),
        address!("b7cb1c96db6b22b0d3d9536e0108d062bd488f74"),
        address!("b96f547da042737c95d7f9397cd86068d0a817a8"),
        address!("ba0dda8762c24da9487f5fa026a9b64b695a07ea"),
        address!("bbbbca6a901c926f240b89eacb641d8aec7aeafd"),
        address!("c3972ac283b3a7a56125674631a5c254f7f373cf"),
        address!("c6e145421fd494b26dcf2bfeb1b02b7c5721978f"),
        address!("c71b5f631354be6853efe9c3ab6b9590f8302e81"),
        address!("c8ccc82aa66193f8ab957859198f086e0e29d02d"),
        address!("cb84d72e61e383767c4dfeb2d8ff7f4fb89abc6e"),
        address!("ccc8cb5229b0ac8069c51fd58367fd1e622afd97"),
        address!("d0352a019e9ab9d757776f532377aaebd36fd541"),
        address!("d13cfd3133239a3c73a9e535a5c4dadee36b395c"),
        address!("d3ebdaea9aeac98de723f640bce4aa07e2e44192"),
        address!("d979c468a68062e7bdff4ba6df7842dfd3492e0f"),
        address!("d97e471695f73d8186deabc1ab5b8765e667cd96"),
        address!("ddb3422497e61e13543bea06989c0789117555c5"),
        address!("de4ce5447ce0c67920a1371605a39187cb6847c8"),
        address!("e28b3b32b6c345a34ff64674606124dd5aceca30"),
        address!("e3c408bd53c31c085a1746af401a4042954ff740"),
        address!("e41d2489571d322189246dafa5ebde1f4699f498"),
        address!("e469c4473af82217b30cf17b10bcdb6c8c796e75"),
        address!("e4815ae53b124e7263f08dcdbbb757d41ed658c6"),
        address!("e5caef4af8780e59df925470b050fb23c43ca68c"),
        address!("ea1ea0972fa092dd463f2968f9bb51cc4c981d71"),
        address!("ee9e7bb7e55bbc86414047b61d65c9c0d91ffbd0"),
        address!("f203ca1769ca8e9e8fe1da9d147db68b6c919817"),
        address!("f3fd2ff0b30151529e26bc5ce86714f97aad6a58"),
        address!("f4d2888d29d722226fafa5d9b24f9164c092421e"),
        address!("f57e7e7c23978c3caec3c3548e3d615c346e79ff"),
        address!("fa1a856cfa3409cfa145fa4e20eb270df3eb21ab"),
        address!("fe3e6a25e6b192a42a44ecddcd13796471735acf"),
    ])
});
