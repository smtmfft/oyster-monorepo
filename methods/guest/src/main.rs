use risc0_zkvm::guest::env;

use std::io::Read;

fn main() {
    // read the attestation
    let mut attestation = Vec::<u8>::new();
    env::stdin().read_to_end(&mut attestation).unwrap();

    println!("Input len: {}", attestation.len());

    // write public output to the journal
    env::commit(&attestation.len());
}
