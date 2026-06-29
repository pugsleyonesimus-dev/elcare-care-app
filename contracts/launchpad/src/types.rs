use soroban_sdk::{contracterror, contracttype, Address, String};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    NotAdmin = 3,
    WasmHashNotSet = 4,
    InvalidFeeBps = 5,
}

/// Which of the four collection types was deployed.
#[contracttype]
#[derive(Clone)]
pub enum CollectionKind {
    Normal721,
    Normal1155,
    LazyMint721,
    LazyMint1155,
}

/// A record stored for every deployed collection (issues #37 + #38).
#[contracttype]
#[derive(Clone)]
pub struct CollectionRecord {
    pub address: Address,
    pub kind: CollectionKind,
    pub creator: Address,
    pub name: String,
    pub symbol: String,
    pub ledger: u32,
    pub platform_fee_bps: u32,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Initialized,
    Admin,
    PlatformFeeReceiver,
    PlatformFeeBps,
    WasmNormal721,
    WasmNormal1155,
    WasmLazy721,
    WasmLazy1155,
    CollectionCount,
    ByCreator(Address),
    AllCollections,
    CollectionByIndex(u64),
    CreatorCollectionCount(Address),
    CreatorCollectionByIndex(Address, u64),
    /// Direct lookup by collection address (#37)
    CollectionByAddress(Address),
}
