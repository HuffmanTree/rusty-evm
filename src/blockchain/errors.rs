use ethnum::u256;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    EmptyStack,
    InsufficientFunds(u256),
    IntrisicGasTooLow(usize),
    InvalidAddress,
    InvalidJumpDest,
    MemoryOutOfBounds,
    OutOfGas,
    StackOverflow,
}
