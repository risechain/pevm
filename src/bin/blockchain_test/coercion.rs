use block_stm_revm::Storage;
use revm::{
    db::{DbAccount, EmptyDB},
    primitives::{Account, AccountStatus, StorageSlot},
    InMemoryDB,
};

fn from_account(value: Account) -> DbAccount {
    DbAccount {
        info: value.info,
        account_state: revm::db::AccountState::None,
        storage: value
            .storage
            .into_iter()
            .map(|(k, v)| (k, v.present_value))
            .collect(),
    }
}

fn to_account(value: DbAccount) -> Account {
    Account {
        info: value.info,
        storage: value
            .storage
            .into_iter()
            .map(|(k, v)| (k, StorageSlot::new(v)))
            .collect(),
        status: AccountStatus::default(),
    }
}

pub(crate) fn from_storage(storage: Storage) -> InMemoryDB {
    InMemoryDB {
        accounts: storage
            .accounts
            .into_iter()
            .map(|(k, v)| (k, from_account(v)))
            .collect(),
        contracts: storage.contracts,
        logs: Vec::new(),
        block_hashes: storage.block_hashes,
        db: EmptyDB::new(),
    }
}

pub(crate) fn to_storage(db: InMemoryDB) -> Storage {
    Storage {
        accounts: db
            .accounts
            .into_iter()
            .map(|(k, v)| (k, to_account(v)))
            .collect(),
        contracts: db.contracts,
        block_hashes: db.block_hashes,
    }
}
