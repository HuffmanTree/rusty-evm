#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    EmptyStack,
    InvalidAddress,
    InvalidJumpDest,
    MemoryOutOfBounds,
    OutOfGas,
    StackOverflow,
}
