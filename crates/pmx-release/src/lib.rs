use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseManifest {
    pub release_id: String,
    pub executor_version: String,
    pub contract_version: String,
    pub build_sequence: u64,
    pub git_commit: Option<String>,
}

pub fn assert_not_regression(
    current: &ReleaseManifest,
    candidate: &ReleaseManifest,
) -> Result<(), String> {
    if candidate.build_sequence <= current.build_sequence {
        return Err(format!(
            "candidate build_sequence {} is not greater than current {}",
            candidate.build_sequence, current.build_sequence
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_release_regression() {
        let current = ReleaseManifest {
            release_id: "a".into(),
            executor_version: "0.1.0".into(),
            contract_version: "1.0.0".into(),
            build_sequence: 10,
            git_commit: None,
        };
        let candidate = ReleaseManifest {
            release_id: "b".into(),
            executor_version: "0.1.1".into(),
            contract_version: "1.0.0".into(),
            build_sequence: 9,
            git_commit: None,
        };
        assert!(assert_not_regression(&current, &candidate).is_err());
    }
}
