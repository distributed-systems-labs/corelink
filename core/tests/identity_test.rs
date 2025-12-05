use corelink_core::identity::Identity;

#[test]
fn test_identity_generation() {
    let id1 = Identity::generate();
    let id2 = Identity::generate();

    assert_ne!(id1.node_id(), id2.node_id());
}

#[test]
fn test_signature_verification() {
    let identity = Identity::generate();
    let data = b"test data";

    let signature = identity.sign(data);

    assert_eq!(signature.to_bytes().len(), 64);
}
