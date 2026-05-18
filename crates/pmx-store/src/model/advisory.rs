#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AdvisoryLockKey(pub i64);

/// Deterministically maps a resource identity to a PostgreSQL advisory lock key.
pub fn advisory_lock_key(namespace: &str, account_id: &str, resource_key: &str) -> AdvisoryLockKey {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    fn feed(mut hash: u64, bytes: &[u8]) -> u64 {
        for b in bytes {
            hash ^= u64::from(*b);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash
    }

    let mut hash = FNV_OFFSET;
    let parts = [
        namespace.as_bytes(),
        account_id.as_bytes(),
        resource_key.as_bytes(),
    ];
    for part in parts {
        hash = feed(hash, &(part.len() as u64).to_be_bytes());
        hash = feed(hash, part);
    }
    AdvisoryLockKey(i64::from_ne_bytes(hash.to_ne_bytes()))
}
