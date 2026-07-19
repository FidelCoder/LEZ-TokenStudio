use attestation_types::{sha256, Digest32, ProgramOwner};
use serde::{Deserialize, Serialize};

pub const COMMITMENT_PREFIX: &[u8; 32] =
    b"/LEE/v0.3/Commitment/\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

pub const DUMMY_COMMITMENT: Commitment = Commitment([
    55, 228, 215, 207, 112, 221, 239, 49, 238, 79, 71, 135, 155, 15, 184, 45, 104, 74, 51, 211,
    238, 42, 160, 243, 15, 124, 253, 62, 3, 229, 90, 27,
]);

pub const DUMMY_COMMITMENT_HASH: Digest32 = [
    250, 237, 192, 113, 155, 101, 119, 30, 235, 183, 20, 84, 26, 32, 196, 229, 154, 74, 254, 249,
    129, 241, 118, 39, 41, 253, 141, 171, 184, 71, 8, 41,
];

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LezAccount {
    pub account_id: Digest32,
    pub program_owner: ProgramOwner,
    pub balance: u128,
    pub nonce: u128,
    pub data: Vec<u8>,
}

impl LezAccount {
    #[must_use]
    pub fn commitment(&self) -> Commitment {
        Commitment::new(self)
    }

    #[must_use]
    pub fn commitment_preimage(&self) -> [u8; 160] {
        let mut bytes = [0_u8; 160];
        bytes[..32].copy_from_slice(COMMITMENT_PREFIX);
        bytes[32..64].copy_from_slice(&self.account_id);

        for (index, word) in self.program_owner.iter().enumerate() {
            let start = 64 + index * 4;
            bytes[start..start + 4].copy_from_slice(&word.to_le_bytes());
        }

        bytes[96..112].copy_from_slice(&self.balance.to_le_bytes());
        bytes[112..128].copy_from_slice(&self.nonce.to_le_bytes());
        bytes[128..160].copy_from_slice(&hash_account_data(&self.data));
        bytes
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Commitment(pub Digest32);

impl Commitment {
    #[must_use]
    pub fn new(account: &LezAccount) -> Self {
        Self(sha256(&account.commitment_preimage()))
    }

    #[must_use]
    pub fn as_bytes(&self) -> &Digest32 {
        &self.0
    }

    #[must_use]
    pub fn leaf_hash(&self) -> Digest32 {
        sha256(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MembershipProof {
    pub leaf_index: usize,
    pub siblings: Vec<Digest32>,
}

impl MembershipProof {
    #[must_use]
    pub fn compute_root(&self, commitment: &Commitment) -> Digest32 {
        let mut result = commitment.leaf_hash();
        let mut level_index = self.leaf_index;

        for sibling in &self.siblings {
            let mut pair = [0_u8; 64];
            if level_index & 1 == 0 {
                pair[..32].copy_from_slice(&result);
                pair[32..].copy_from_slice(sibling);
            } else {
                pair[..32].copy_from_slice(sibling);
                pair[32..].copy_from_slice(&result);
            }
            result = sha256(&pair);
            level_index >>= 1;
        }

        result
    }

    #[must_use]
    pub fn verifies(&self, commitment: &Commitment, expected_root: &Digest32) -> bool {
        self.compute_root(commitment) == *expected_root
    }
}

#[must_use]
pub fn hash_account_data(data: &[u8]) -> Digest32 {
    sha256(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_account_matches_official_dummy_commitment() {
        let account = LezAccount::default();

        assert_eq!(account.commitment(), DUMMY_COMMITMENT);
        assert_eq!(account.commitment().leaf_hash(), DUMMY_COMMITMENT_HASH);
    }

    #[test]
    fn commitment_preimage_has_official_field_order_and_endianness() {
        let account = LezAccount {
            account_id: [0x41; 32],
            program_owner: [
                0x0302_0100,
                0x0706_0504,
                0x0b0a_0908,
                0x0f0e_0d0c,
                0x1312_1110,
                0x1716_1514,
                0x1b1a_1918,
                0x1f1e_1d1c,
            ],
            balance: 0x0f0e_0d0c_0b0a_0908_0706_0504_0302_0100,
            nonce: 0x1f1e_1d1c_1b1a_1918_1716_1514_1312_1110,
            data: b"proofgate".to_vec(),
        };

        let preimage = account.commitment_preimage();
        assert_eq!(&preimage[..32], COMMITMENT_PREFIX);
        assert_eq!(&preimage[32..64], &[0x41; 32]);
        assert_eq!(&preimage[64..96], &(0_u8..32).collect::<Vec<_>>());
        assert_eq!(&preimage[96..112], &(0_u8..16).collect::<Vec<_>>());
        assert_eq!(&preimage[112..128], &(16_u8..32).collect::<Vec<_>>());
        assert_eq!(&preimage[128..], &hash_account_data(b"proofgate"));
    }

    #[test]
    fn each_private_account_field_changes_the_commitment() {
        let base = LezAccount::default();
        let expected = base.commitment();

        let mut account_id = base.clone();
        account_id.account_id[0] = 1;
        let mut owner = base.clone();
        owner.program_owner[0] = 1;
        let mut balance = base.clone();
        balance.balance = 1;
        let mut nonce = base.clone();
        nonce.nonce = 1;
        let mut data = base;
        data.data.push(1);

        for changed in [account_id, owner, balance, nonce, data] {
            assert_ne!(changed.commitment(), expected);
        }
    }

    #[test]
    fn membership_path_respects_leaf_position() {
        let left = LezAccount::default().commitment();
        let right = LezAccount {
            account_id: [7; 32],
            ..LezAccount::default()
        }
        .commitment();
        let mut pair = [0_u8; 64];
        pair[..32].copy_from_slice(&left.leaf_hash());
        pair[32..].copy_from_slice(&right.leaf_hash());
        let root = sha256(&pair);

        let left_proof = MembershipProof {
            leaf_index: 0,
            siblings: vec![right.leaf_hash()],
        };
        let right_proof = MembershipProof {
            leaf_index: 1,
            siblings: vec![left.leaf_hash()],
        };

        assert!(left_proof.verifies(&left, &root));
        assert!(right_proof.verifies(&right, &root));
        assert!(!right_proof.verifies(&left, &root));
    }

    #[test]
    fn tampered_membership_path_fails() {
        let commitment = LezAccount::default().commitment();
        let proof = MembershipProof {
            leaf_index: 0,
            siblings: vec![[3; 32], [4; 32]],
        };
        let root = proof.compute_root(&commitment);
        let tampered = MembershipProof {
            leaf_index: 0,
            siblings: vec![[3; 32], [5; 32]],
        };

        assert!(!tampered.verifies(&commitment, &root));
    }
}
