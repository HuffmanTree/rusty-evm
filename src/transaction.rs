use ethnum::{u256, U256};

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub struct Address(pub u256);

impl std::fmt::Debug for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Address({:#X})", self.0)
    }
}

impl TryInto::<Address> for u256 {
    type Error = crate::errors::Error;

    fn try_into(self) -> Result<Address, Self::Error> {
        if self >> 160 != U256::ZERO { Err(crate::errors::Error::InvalidAddress) } else { Ok(Address(self)) }
    }
}

#[derive(Default)]
pub struct Transaction {
    pub data: Vec<u8>,
    pub from: Address,
    pub gas: usize,
    pub nonce: usize,
    pub to: Address,
}

#[cfg(test)]
mod tests {
    use ethnum::uint;
    use super::*;

    #[test]
    fn u256_try_into_address() {
        assert_eq!(TryInto::<Address>::try_into(uint!("0x372BDB7F2E599AD23590DAEAF0490D46185BEC962CAC93120B52389748E99C0C")), Err(crate::errors::Error::InvalidAddress));
        assert_eq!(TryInto::<Address>::try_into(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")), Ok(Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C"))));
    }
}
