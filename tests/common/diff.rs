use alloy_consensus::{Eip658Value, Receipt};
use alloy_primitives::{Log, B256, U256};
use pevm::PevmTxExecutionResult;
use revm::primitives::{Account, AccountInfo, AccountStatus, Bytecode, EvmStorageSlot};
use std::{
    collections::BTreeSet,
    fmt::{Debug, Display},
    hash::Hash,
    path::PathBuf,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Reason {
    path: PathBuf,
    left: String,
    right: String,
}

impl Reason {
    fn new(path: PathBuf, left: impl Debug, right: impl Debug) -> Self {
        Reason {
            path,
            left: format!("{:?}", left),
            right: format!("{:?}", right),
        }
    }
}

impl Display for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let path_components: Vec<&str> = self.path.iter().map(|s| s.to_str().unwrap()).collect();

        write!(
            f,
            "_.{}\n   left = {}\n  right = {}",
            path_components.join("."),
            self.left,
            self.right
        )
    }
}

fn diff_simple<T: PartialEq + Debug>(path: PathBuf, left: &T, right: &T) -> Vec<Reason> {
    if left != right {
        Vec::from(&[Reason::new(path, left, right)])
    } else {
        Vec::new()
    }
}

pub trait Diffable {
    fn diff(path: PathBuf, left: &Self, right: &Self) -> Vec<Reason>;
}

macro_rules! impl_diffable {
    ($t:ty) => {
        impl Diffable for $t {
            fn diff(path: PathBuf, left: &Self, right: &Self) -> Vec<Reason> {
                diff_simple(path, left, right)
            }
        }
    };
}

impl_diffable!(AccountStatus);
impl_diffable!(B256);
impl_diffable!(Bytecode);
impl_diffable!(Eip658Value);
impl_diffable!(EvmStorageSlot);
impl_diffable!(Log);
impl_diffable!(u128);
impl_diffable!(U256);
impl_diffable!(u64);
impl_diffable!(usize);

impl<T: Diffable> Diffable for Option<T> {
    fn diff(path: PathBuf, left: &Self, right: &Self) -> Vec<Reason> {
        match (left, right) {
            (None, None) => Vec::new(),
            (None, Some(_)) => Vec::from(&[Reason::new(path, "None", "Some(_)")]),
            (Some(_), None) => Vec::from(&[Reason::new(path, "Some(_)", "None")]),
            (Some(left_inner), Some(right_inner)) => {
                Diffable::diff(path.join("unwrap()"), left_inner, right_inner)
            }
        }
    }
}

impl<T: Diffable, E: Debug + PartialEq> Diffable for Result<T, E> {
    fn diff(path: PathBuf, left: &Self, right: &Self) -> Vec<Reason> {
        match (left, right) {
            (Ok(left_inner), Ok(right_inner)) => {
                Diffable::diff(path.join("unwrap()"), left_inner, right_inner)
            }
            (Ok(_), Err(_)) => Vec::from(&[Reason::new(path, "Ok(_)", "Err(_)")]),
            (Err(_), Ok(_)) => Vec::from(&[Reason::new(path, "Err(_)", "Ok(_)")]),
            (Err(left_error), Err(right_error)) => {
                diff_simple(path.join("unwrap_err()"), left_error, right_error)
            }
        }
    }
}

impl<T: Diffable> Diffable for [T] {
    fn diff(path: PathBuf, left: &Self, right: &Self) -> Vec<Reason> {
        let mut reasons = diff_simple(path.join("len()"), &left.len(), &right.len());
        for (index, (left_item, right_item)) in Iterator::zip(left.iter(), right.iter()).enumerate()
        {
            reasons.extend(Diffable::diff(
                path.join(index.to_string()),
                left_item,
                right_item,
            ));
        }
        reasons
    }
}

impl<T: Diffable> Diffable for Vec<T> {
    fn diff(path: PathBuf, left: &Self, right: &Self) -> Vec<Reason> {
        Diffable::diff(path, &left[..], &right[..])
    }
}

impl Diffable for AccountInfo {
    fn diff(path: PathBuf, left: &Self, right: &Self) -> Vec<Reason> {
        let mut reasons = Vec::new();
        reasons.extend(Diffable::diff(
            path.join("balance"),
            &left.balance,
            &right.balance,
        ));
        reasons.extend(Diffable::diff(
            path.join("nonce"), //
            &left.nonce,
            &right.nonce,
        ));
        reasons.extend(Diffable::diff(
            path.join("code_hash"),
            &left.code_hash,
            &right.code_hash,
        ));
        reasons
    }
}

impl Diffable for Receipt {
    fn diff(path: PathBuf, left: &Self, right: &Self) -> Vec<Reason> {
        let mut reasons = Vec::new();
        reasons.extend(Diffable::diff(
            path.join("status"),
            &left.status,
            &right.status,
        ));
        reasons.extend(Diffable::diff(
            path.join("cumulative_gas_used"),
            &left.cumulative_gas_used,
            &right.cumulative_gas_used,
        ));
        reasons.extend(Diffable::diff(
            path.join("logs"),
            &left.logs[..],
            &right.logs[..],
        ));
        reasons
    }
}

impl<K: Copy + Ord + Debug + Hash, V: Diffable + Clone> Diffable
    for std::collections::HashMap<K, V>
{
    fn diff(path: PathBuf, left: &Self, right: &Self) -> Vec<Reason> {
        let mut reasons = Vec::new();
        let all_addresses: BTreeSet<K> = Iterator::chain(left.keys(), right.keys())
            .copied()
            .collect();
        for key in all_addresses {
            reasons.extend(Diffable::diff(
                path.join(format!("get(\"{:?}\")", key)),
                &left.get(&key).cloned(),
                &right.get(&key).cloned(),
            ));
        }
        reasons
    }
}

impl Diffable for Account {
    fn diff(path: PathBuf, left: &Self, right: &Self) -> Vec<Reason> {
        let mut reasons = Vec::new();
        reasons.extend(Diffable::diff(
            path.join("info"), //
            &left.info,
            &right.info,
        ));
        reasons.extend(Diffable::diff(
            path.join("storage"),
            &left.storage,
            &right.storage,
        ));
        reasons.extend(Diffable::diff(
            path.join("status"),
            &left.status,
            &right.status,
        ));
        reasons
    }
}

impl Diffable for PevmTxExecutionResult {
    fn diff(path: PathBuf, left: &Self, right: &Self) -> Vec<Reason> {
        let mut reasons = Vec::new();
        reasons.extend(Diffable::diff(
            path.join("receipt"),
            &left.receipt,
            &right.receipt,
        ));
        reasons.extend(Diffable::diff(
            path.join("state"),
            &left.state,
            &right.state,
        ));
        reasons
    }
}
