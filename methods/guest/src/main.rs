use risc0_zkvm::guest::env;

fn main() {
    // TODO: Implement your guest code here

    // read the input
    let input: Vec<u8> = env::read();

    println!("Input len: {}", input.len());

    // TODO: do something with the input

    // write public output to the journal
    env::commit(&input);
}
