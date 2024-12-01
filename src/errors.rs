#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    EmptyStack,
    InvalidJumpDest,
    MemoryOutOfBounds,
    OutOfGas,
    StackOverflow,
}
