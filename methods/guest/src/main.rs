use risc0_zkvm::guest::env;

use std::io::Read;

use sha2::Digest;

fn main() {
    // read the attestation
    let mut attestation = Vec::<u8>::new();
    env::stdin().read_to_end(&mut attestation).unwrap();

    println!("Attestation: {:?}", attestation);

    // short circuit parsing by just asserting a specific structure
    // initial fields
    assert_eq!(
        attestation[0..8],
        [0x84, 0x44, 0xa1, 0x01, 0x38, 0x22, 0xa0, 0x59]
    );
    // payload size
    let size = u16::from_be_bytes([attestation[8], attestation[9]]) as usize;
    println!("Size: {size}");
    // payload should be in attestation[10..10+size]
    // total size
    assert_eq!(attestation.len(), 10 + size + 98);
    // signature size
    assert_eq!(attestation[size + 10], 0x58);
    assert_eq!(attestation[size + 11], 0x60);

    let mut hasher = sha2::Sha384::new();
    // array with 4 elements
    hasher.update(&[0x84]);
    // context field length
    hasher.update(&[0x4a]);
    // context field
    hasher.update("Signature1");
    // empty body_protected
    hasher.update(&[0x40]);
    // empty aad
    hasher.update(&[0x40]);
    // payload length
    hasher.update(&[0x59, attestation[8], attestation[9]]);
    // payload
    hasher.update(&attestation[10..10 + size]);
    let hash = hasher.finalize();

    println!("Hash: {hash:?}");

    // write public output to the journal
    env::commit(&true);
}
