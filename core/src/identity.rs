use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey, SECRET_KEY_LENGTH};
use rand::RngCore;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId([u8; 32]);

impl NodeId {
    pub fn from_pubkey(pubkey: &VerifyingKey) -> Self {
        let hash = blake3::hash(pubkey.as_bytes());
        Self(*hash.as_bytes())
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

pub struct Identity {
    signing_key: SigningKey,
    node_id: NodeId,
}

impl Identity {
    pub fn generate() -> Self {
        let mut csprng = rand::thread_rng();
        let mut secret_bytes = [0u8; SECRET_KEY_LENGTH];
        csprng.fill_bytes(&mut secret_bytes);

        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let verifying_key = signing_key.verifying_key();
        let node_id = NodeId::from_pubkey(&verifying_key);

        Self {
            signing_key,
            node_id,
        }
    }

    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    pub fn sign(&self, data: &[u8]) -> Signature {
        self.signing_key.sign(data)
    }

    pub fn verify(&self, data: &[u8], signature: &Signature, pubkey: &VerifyingKey) -> bool {
        pubkey.verify(data, signature).is_ok()
    }
}
