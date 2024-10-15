use risc0_zkvm::guest::env;

use std::io::Read;

use p384::ecdsa::signature::hazmat::PrehashVerifier;
// use p384::ecdsa::Signature;
// use p384::ecdsa::VerifyingKey;
use sha2::Digest;
use x509_cert::der::Decode;
use x509_cert::der::Encode;
use x509_verify::{Signature, VerifyInfo, VerifyingKey};

// Design notes:
// Generally, it asserts a specific structure instead of parsing everything in a generic fashion.
// Helps keep the proving time low at the cost of being less flexible towards structure changes.
// Skips processing certificate extensions. Verifies only signatures, expiry and subject/issuer.

fn main() {
    // read the attestation
    let mut attestation = Vec::<u8>::new();
    env::stdin().read_to_end(&mut attestation).unwrap();

    println!(
        "Attestation: {} bytes: {:?}",
        attestation.len(),
        attestation
    );

    // assert initial fields
    assert_eq!(
        attestation[0..8],
        [
            0x84, // the COSE structure is an array of size 4
            0x44, 0xa1, 0x01, 0x38, 0x22, // protected header, specifying P384 signature
            0xa0, // empty unprotected header
            0x59, // payload size of 2 bytes follows
        ]
    );

    // get payload size
    let payload_size = u16::from_be_bytes([attestation[8], attestation[9]]) as usize;
    println!("Payload size: {payload_size}");

    // assert total size
    assert_eq!(attestation.len(), 10 + payload_size + 98);

    // payload should be in attestation[10..10 + payload_size]
    // signature should be in attestation[12 + payload_size..108 + payload_size]

    // skip fields and simply assert length
    assert_eq!(
        attestation[10..12],
        [
            0xa9, // attestation doc payload is map of size 9
            // expected keys: module_id, digest, timestamp, pcrs, certificate, cabundle,
            // public_key, user_data, nonce
            0x69, // text of size 9, "module_id" key
        ]
    );
    assert_eq!(
        attestation[21..23],
        [
            0x78, // text with one byte length follows
            0x27, // text of size 39, module id value
        ]
    );
    assert_eq!(attestation[62], 0x66); // text of size 6, "digest" key
    assert_eq!(attestation[69], 0x66); // text of size 6, "SHA384" value

    // assert timestamp key
    assert_eq!(attestation[76], 0x69); // text of size 9
    assert_eq!(&attestation[77..86], b"timestamp");
    // commit the timestamp value
    assert_eq!(attestation[86], 0x1b); // unsigned int, size 8
    println!("Timestamp: {:?}", &attestation[87..95]);
    env::commit_slice(&attestation[87..95]);

    // assert pcrs key
    assert_eq!(attestation[95], 0x64); // text of size 4
    assert_eq!(&attestation[96..100], b"pcrs");
    // commit pcrs 0, 1 and 2
    assert_eq!(attestation[100], 0xb0); // pcrs is a map of size 16
    assert_eq!(
        attestation[101..104],
        [
            0x00, // pcr number
            0x58, // bytes with one byte length follows
            0x30, // 48 length
        ]
    );
    println!("PCR0: {:?}", &attestation[104..152]);
    env::commit_slice(&attestation[104..152]);
    assert_eq!(attestation[152..155], [0x01, 0x58, 0x30]);
    println!("PCR1: {:?}", &attestation[155..203]);
    env::commit_slice(&attestation[155..203]);
    assert_eq!(attestation[203..206], [0x02, 0x58, 0x30]);
    println!("PCR2: {:?}", &attestation[206..254]);
    env::commit_slice(&attestation[206..254]);

    // skip rest of the pcrs, 3 to 15
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

    // assert certificate key
    assert_eq!(attestation[917], 0x6b); // text of size 11
    assert_eq!(&attestation[918..929], b"certificate");
    // get leaf certificate
    assert_eq!(attestation[929], 0x59); // bytes where two byte length follows
    let leaf_cert_size = u16::from_be_bytes([attestation[930], attestation[931]]) as usize;
    let leaf_cert =
        x509_cert::Certificate::from_der(&attestation[932..932 + leaf_cert_size]).unwrap();

    // assert cabundle key
    assert_eq!(attestation[932 + leaf_cert_size], 0x68); // text of length 8
    assert_eq!(
        &attestation[932 + leaf_cert_size + 1..932 + leaf_cert_size + 9],
        b"cabundle"
    );

    // cabundle should be an array, read length
    let chain_size = attestation[932 + leaf_cert_size + 9];
    // just restrict chain size instead of figuring out parsing too much
    // works for tiny field encoded cabundle up to 16 length
    assert!(chain_size > 0x80 && chain_size <= 0x90);
    let chain_size = chain_size - 0x80; // real size is minus 0x80 for bytes type

    // verify certificate chain
    // first certificate in the list is the root certificate
    // last certificate in the list signs the leaf certificate obtained above

    // track start of each section so we know where to proceed from after the block
    let mut next_cert_start = 932 + leaf_cert_size + 10;
    {
        // start with the root cert

        // parse root cert
        assert_eq!(attestation[next_cert_start], 0x59); // bytes where two byte length follows
        let size = u16::from_be_bytes([
            attestation[next_cert_start + 1],
            attestation[next_cert_start + 2],
        ]) as usize;

        // cert with the pubkey, start with the root
        let mut parent_cert = x509_cert::Certificate::from_der(
            &attestation[next_cert_start + 3..next_cert_start + 3 + size],
        )
        .unwrap();

        // commit the root pubkey
        let pubkey = parent_cert
            .tbs_certificate
            .subject_public_key_info
            .subject_public_key
            .raw_bytes();
        println!(
            "Root certificate public key: {} bytes: {:?}",
            pubkey.len(),
            pubkey
        );
        env::commit_slice(pubkey);

        // start of next cert that is to be verified
        next_cert_start = next_cert_start + 3 + size;

        for _ in 0..chain_size - 1 {
            // parse child cert
            assert_eq!(attestation[next_cert_start], 0x59); // bytes where two byte length follows
            let size = u16::from_be_bytes([
                attestation[next_cert_start + 1],
                attestation[next_cert_start + 2],
            ]) as usize;

            // parse the next cert and get the public key
            let child_cert = x509_cert::Certificate::from_der(
                &attestation[next_cert_start + 3..next_cert_start + 3 + size],
            )
            .unwrap();

            // verify signature
            let verify_info = VerifyInfo::new(
                child_cert.tbs_certificate.to_der().unwrap().into(),
                Signature::new(
                    &child_cert.signature_algorithm,
                    child_cert.signature.as_bytes().unwrap(),
                ),
            );

            let key: VerifyingKey = parent_cert
                .tbs_certificate
                .subject_public_key_info
                .clone()
                .try_into()
                .unwrap();

            key.verify(verify_info).unwrap();

            // set up for next iteration
            parent_cert = child_cert;
            next_cert_start = next_cert_start + 3 + size;
        }

        // verify the leaf cert with the last cert in the chain
        let verify_info = VerifyInfo::new(
            leaf_cert.tbs_certificate.to_der().unwrap().into(),
            Signature::new(
                &leaf_cert.signature_algorithm,
                leaf_cert.signature.as_bytes().unwrap(),
            ),
        );

        let key: VerifyingKey = parent_cert
            .tbs_certificate
            .subject_public_key_info
            .clone()
            .try_into()
            .unwrap();

        key.verify(verify_info).unwrap();
    }

    // assert public_key key
    assert_eq!(attestation[next_cert_start], 0x6a); // text of size 10
    assert_eq!(
        &attestation[next_cert_start + 1..next_cert_start + 11],
        b"public_key"
    );
    // commit public key, expected length of 64 since it is a secp256k1 key
    assert_eq!(attestation[next_cert_start + 11], 0x58); // bytes where one byte length follows
    assert_eq!(attestation[next_cert_start + 12], 0x40); // 64 length
    env::commit_slice(&attestation[next_cert_start + 13..next_cert_start + 77]);

    // assert user_data key
    assert_eq!(attestation[next_cert_start + 77], 0x69); // text of size 9
    assert_eq!(
        &attestation[next_cert_start + 78..next_cert_start + 87],
        b"user_data"
    );
    // commit user data
    // handle cases up to 65536 size
    let (user_data_size, user_data) = if attestation[next_cert_start + 87] == 0xf6 {
        // empty
        (0, [].as_slice())
    } else if attestation[next_cert_start + 87] == 0x58 {
        // one byte length follows
        let size = attestation[next_cert_start + 88] as u16;

        (
            size,
            &attestation[next_cert_start + 89..next_cert_start + 89 + size as usize],
        )
    } else {
        // only allow 2 byte lengths as max
        // technically, this is already enforced by COSE doc size parsing
        assert_eq!(attestation[next_cert_start + 87], 0x59);

        let size = u16::from_be_bytes([
            attestation[next_cert_start + 88],
            attestation[next_cert_start + 89],
        ]);

        (
            size,
            &attestation[next_cert_start + 90..next_cert_start + 90 + size as usize],
        )
    };
    println!("User data: {} bytes: {:?}", user_data_size, user_data);
    // commit 2 byte length, then data
    env::commit_slice(&user_data_size.to_be_bytes());
    env::commit_slice(user_data);

    // prepare COSE verification hash
    let mut hasher = sha2::Sha384::new();
    // array with 4 elements
    hasher.update(&[0x84]);
    // context field length
    hasher.update(&[0x6a]);
    // context field
    hasher.update("Signature1");
    // body_protected
    hasher.update(&[0x44, 0xa1, 0x01, 0x38, 0x22]);
    // empty aad
    hasher.update(&[0x40]);
    // payload length
    hasher.update(&[0x59, attestation[8], attestation[9]]);
    // payload
    hasher.update(&attestation[10..10 + payload_size]);
    let hash = hasher.finalize();

    // verify signature
    // signature size
    assert_eq!(attestation[payload_size + 10], 0x58); // bytes where one byte length follows
    assert_eq!(attestation[payload_size + 11], 0x60); // 96 length

    let leaf_cert_pubkey = leaf_cert
        .tbs_certificate
        .subject_public_key_info
        .subject_public_key
        .raw_bytes()
        .to_owned();
    let verifying_key = p384::ecdsa::VerifyingKey::from_sec1_bytes(&leaf_cert_pubkey).unwrap();
    let r: [u8; 48] = attestation[12 + payload_size..60 + payload_size]
        .try_into()
        .unwrap();
    let s: [u8; 48] = attestation[60 + payload_size..108 + payload_size]
        .try_into()
        .unwrap();
    let signature = p384::ecdsa::Signature::from_scalars(r, s).unwrap();

    verifying_key.verify_prehash(&hash, &signature).unwrap();

    println!("Done!");
}
