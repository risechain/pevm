use revm::{
    context::Cfg,
    context_interface::ContextTr,
    handler::{EthPrecompiles, PrecompileProvider},
    interpreter::{CallInputs, InterpreterResult},
    precompile::{
        self, EthPrecompileResult, Precompile, PrecompileHalt, PrecompileId, Precompiles, bn254,
        eth_precompile_fn, secp256r1,
    },
    primitives::{AddressSet, OnceLock, hardfork::SpecId},
};
use std::string::String;

/// Precompile provider for RISE — always uses the Jovian precompile set.
#[derive(Debug, Clone)]
pub struct OpPrecompiles(EthPrecompiles);

impl Default for OpPrecompiles {
    fn default() -> Self {
        Self(EthPrecompiles {
            precompiles: rise_precompiles(),
            spec: SpecId::default(),
        })
    }
}

impl<CTX> PrecompileProvider<CTX> for OpPrecompiles
where
    CTX: ContextTr<Cfg: Cfg<Spec = SpecId>>,
{
    type Output = InterpreterResult;

    #[inline]
    fn set_spec(&mut self, _: SpecId) -> bool {
        false // RISE always uses Jovian precompiles
    }

    #[inline]
    fn run(
        &mut self,
        context: &mut CTX,
        inputs: &CallInputs,
    ) -> Result<Option<Self::Output>, String> {
        self.0.run(context, inputs)
    }

    #[inline]
    fn warm_addresses(&self) -> &AddressSet {
        self.0.warm_addresses()
    }

    #[inline]
    fn contains(&self, address: &revm::primitives::Address) -> bool {
        self.0.contains(address)
    }
}

/// Builds the Jovian precompile set used by RISE.
///
/// Starts from Cancun, adds secp256r1 and BLS12-381 (Isthmus/Granite), then replaces
/// bn254 pairing and BLS12-381 MSM/pairing with Jovian input-size-limited versions.
fn rise_precompiles() -> &'static Precompiles {
    static INSTANCE: OnceLock<Precompiles> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        let mut p = Precompiles::cancun().clone();
        p.extend([secp256r1::P256VERIFY]);
        p.extend(precompile::bls12_381::precompiles());
        // Jovian: replace bn254 pair and BLS MSM/pairing with tighter input-size limits.
        p.extend([bn254_pair::JOVIAN]);
        p.extend([
            bls12_381::JOVIAN_G1_MSM,
            bls12_381::JOVIAN_G2_MSM,
            bls12_381::JOVIAN_PAIRING,
        ]);
        p
    })
}

/// Bn254 pairing precompile with Jovian input size limit.
mod bn254_pair {
    use super::*;

    const JOVIAN_MAX: usize = 81_984;
    pub(super) const JOVIAN: Precompile =
        Precompile::new(PrecompileId::Bn254Pairing, bn254::pair::ADDRESS, jovian_fn);

    fn run_jovian(input: &[u8], gas_limit: u64) -> EthPrecompileResult {
        if input.len() > JOVIAN_MAX {
            return Err(PrecompileHalt::Bn254PairLength);
        }
        bn254::run_pair(
            input,
            bn254::pair::ISTANBUL_PAIR_PER_POINT,
            bn254::pair::ISTANBUL_PAIR_BASE,
            gas_limit,
        )
    }
    eth_precompile_fn!(jovian_fn, run_jovian);
}

/// BLS12-381 precompiles with Jovian input size limits.
mod bls12_381 {
    use super::*;
    use revm::precompile::bls12_381_const::{G1_MSM_ADDRESS, G2_MSM_ADDRESS, PAIRING_ADDRESS};

    const JOVIAN_G1_MSM_MAX: usize = 288_960;
    const JOVIAN_G2_MSM_MAX: usize = 278_784;
    const JOVIAN_PAIRING_MAX: usize = 156_672;

    pub(super) const JOVIAN_G1_MSM: Precompile =
        Precompile::new(PrecompileId::Bls12G1Msm, G1_MSM_ADDRESS, jovian_g1_msm_fn);
    pub(super) const JOVIAN_G2_MSM: Precompile =
        Precompile::new(PrecompileId::Bls12G2Msm, G2_MSM_ADDRESS, jovian_g2_msm_fn);
    pub(super) const JOVIAN_PAIRING: Precompile = Precompile::new(
        PrecompileId::Bls12Pairing,
        PAIRING_ADDRESS,
        jovian_pairing_fn,
    );

    #[inline(always)]
    fn run_with_limit(
        input: &[u8],
        gas_limit: u64,
        max: usize,
        err: &'static str,
        inner: fn(&[u8], u64) -> EthPrecompileResult,
    ) -> EthPrecompileResult {
        if input.len() > max {
            Err(PrecompileHalt::Other(err.into()))
        } else {
            inner(input, gas_limit)
        }
    }

    fn run_jovian_g1_msm(input: &[u8], gas_limit: u64) -> EthPrecompileResult {
        run_with_limit(
            input,
            gas_limit,
            JOVIAN_G1_MSM_MAX,
            "G1MSM input exceeds Jovian limit",
            precompile::bls12_381::g1_msm::g1_msm,
        )
    }
    eth_precompile_fn!(jovian_g1_msm_fn, run_jovian_g1_msm);

    fn run_jovian_g2_msm(input: &[u8], gas_limit: u64) -> EthPrecompileResult {
        run_with_limit(
            input,
            gas_limit,
            JOVIAN_G2_MSM_MAX,
            "G2MSM input exceeds Jovian limit",
            precompile::bls12_381::g2_msm::g2_msm,
        )
    }
    eth_precompile_fn!(jovian_g2_msm_fn, run_jovian_g2_msm);

    fn run_jovian_pairing(input: &[u8], gas_limit: u64) -> EthPrecompileResult {
        run_with_limit(
            input,
            gas_limit,
            JOVIAN_PAIRING_MAX,
            "Pairing input exceeds Jovian limit",
            precompile::bls12_381::pairing::pairing,
        )
    }
    eth_precompile_fn!(jovian_pairing_fn, run_jovian_pairing);
}
