use risc0_zkvm::guest::env;

use std::io::Read;

fn main() {
    // TODO: Implement your guest code here

    // read the attestation
    let mut attestation = Vec::<u8>::new();
    env::stdin().read_to_end(&mut attestation).unwrap();

    println!("Input len: {}", attestation.len());

    // TODO: do something with the input

    // write public output to the journal
    env::commit(&attestation.len());
}
