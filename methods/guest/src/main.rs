use risc0_zkvm::guest::env;

use std::io::Read;

use p384::ecdsa::signature::hazmat::PrehashVerifier;
use p384::ecdsa::Signature;
use p384::ecdsa::VerifyingKey;
use sha2::Digest;
use x509_cert::der::Decode;

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
    // total size
    assert_eq!(attestation.len(), 10 + size + 98);

    // payload should be in attestation[10..10+size]
    // signature should be in attestation[12+size..108+size]

    // skip fields and simply assert length
    assert_eq!(attestation[10..12], [0xa9, 0x69]);
    assert_eq!(attestation[21..23], [0x78, 0x27]);
    assert_eq!(attestation[62], 0x66);
    assert_eq!(attestation[69], 0x66);

    // timestamp key
    assert_eq!(attestation[76], 0x69);
    assert_eq!(&attestation[77..86], b"timestamp");
    // commit the timestamp value
    assert_eq!(attestation[86], 0x1b);
    println!("Timestamp: {:?}", &attestation[87..95]);
    env::commit::<[u8; 8]>(attestation[87..95].try_into().unwrap());

    // pcrs key
    assert_eq!(attestation[95], 0x64);
    assert_eq!(&attestation[96..100], b"pcrs");
    // commit pcrs 0, 1 and 2
    assert_eq!(attestation[100], 0xb0);
    assert_eq!(attestation[101..104], [0x00, 0x58, 0x30]);
    println!("pcr0: {:?}", &attestation[104..152]);
    // commit in 2 parts coz serde does not work for arrays over 32 length
    env::commit::<[u8; 32]>(attestation[104..136].try_into().unwrap());
    env::commit::<[u8; 16]>(attestation[136..152].try_into().unwrap());
    assert_eq!(attestation[152..155], [0x01, 0x58, 0x30]);
    println!("pcr1: {:?}", &attestation[155..203]);
    env::commit::<[u8; 32]>(attestation[155..187].try_into().unwrap());
    env::commit::<[u8; 16]>(attestation[187..203].try_into().unwrap());
    assert_eq!(attestation[203..206], [0x02, 0x58, 0x30]);
    println!("pcr2: {:?}", &attestation[206..254]);
    env::commit::<[u8; 32]>(attestation[206..238].try_into().unwrap());
    env::commit::<[u8; 16]>(attestation[238..254].try_into().unwrap());

    // skip rest of the pcrs
    assert_eq!(attestation[254..257], [0x03, 0x58, 0x30]);
    assert_eq!(attestation[305..308], [0x04, 0x58, 0x30]);
    assert_eq!(attestation[356..359], [0x05, 0x58, 0x30]);
    assert_eq!(attestation[407..410], [0x06, 0x58, 0x30]);
    assert_eq!(attestation[458..461], [0x07, 0x58, 0x30]);
    assert_eq!(attestation[509..512], [0x08, 0x58, 0x30]);
    assert_eq!(attestation[560..563], [0x09, 0x58, 0x30]);
    assert_eq!(attestation[611..614], [0x0a, 0x58, 0x30]);
    assert_eq!(attestation[662..665], [0x0b, 0x58, 0x30]);
    assert_eq!(attestation[713..716], [0x0c, 0x58, 0x30]);
    assert_eq!(attestation[764..767], [0x0d, 0x58, 0x30]);
    assert_eq!(attestation[815..818], [0x0e, 0x58, 0x30]);
    assert_eq!(attestation[866..869], [0x0f, 0x58, 0x30]);
    println!("Skipped rest of the pcrs");

    // certificate key
    assert_eq!(attestation[917], 0x6b);
    assert_eq!(&attestation[918..929], b"certificate");
    // certificate
    assert_eq!(attestation[929], 0x59);
    let cert_size = u16::from_be_bytes([attestation[930], attestation[931]]) as usize;
    println!("Certificate size: {}", cert_size);

    let cert = x509_cert::Certificate::from_der(&attestation[932..932 + cert_size]).unwrap();
    let cert_pubkey = cert
        .tbs_certificate
        .subject_public_key_info
        .subject_public_key
        .raw_bytes();
    println!(
        "Certificate public key: {} bytes: {:?}",
        cert_pubkey.len(),
        cert_pubkey
    );

    // TODO: extract and commit public key from attestation

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

    // verify signature
    // signature size
    assert_eq!(attestation[size + 10], 0x58);
    assert_eq!(attestation[size + 11], 0x60);

    let verifying_key = VerifyingKey::from_sec1_bytes(cert_pubkey).unwrap();
    let r: [u8; 48] = attestation[12 + size..60 + size].try_into().unwrap();
    let s: [u8; 48] = attestation[60 + size..108 + size].try_into().unwrap();
    let signature = Signature::from_scalars(r, s).unwrap();
    // let signature: [u8; 96] = attestation[12 + size..108 + size].try_into().unwrap();
    // let signature = Signature::try_from(signature.as_slice()).unwrap();
    println!("Verifying");
    verifying_key.verify_prehash(&hash, &signature).unwrap();
    println!("Verified");
}
