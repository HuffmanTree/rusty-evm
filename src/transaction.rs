use ethnum::u256;

#[derive(Clone, Copy)]
pub struct Address(pub u256);

pub struct Transaction {
    pub data: Vec<u8>,
    pub from: Address,
    pub gas: usize,
    pub nonce: usize,
    pub to: Address,
}
