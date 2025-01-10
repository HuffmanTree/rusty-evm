use ethnum::{u256, U256};
use rlp::RlpStream;

use crate::utils::Hash;

#[derive(Default, Clone, Copy, PartialEq, Eq, Hash)]
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


#[derive(Debug, Default, Clone)]
pub struct Account {
    pub balance: u256,
    pub code: Vec<u8>,
}

#[derive(Default, Debug, Clone)]
pub struct Transaction {
    pub data: Vec<u8>,
    pub from: Address,
    pub gas: usize,
    pub gas_price: usize,
    pub nonce: usize,
    pub to: Address,
    pub value: u256,
}

#[derive(Default)]
pub struct Block {
    pub difficulty: u256,
    pub gas_limit: u256,
    pub miner: Address,
    pub number: u256,
    pub time: u256,
}

impl Transaction {
    pub fn contract_address(&self) -> Address {
        let Self { mut from, mut nonce, to, .. } = self;
        if self.is_contract_creation() { // keccak256(rlp([sender, nonce]))
            let mut from_vec: Vec<u8> = vec![];
            for _ in 0..20 {
                from_vec.push((from.0 & 0xFF).try_into().unwrap());
                from.0 >>= 8;
            }
            from_vec.reverse();
            let mut nonce_vec: Vec<u8> = vec![];
            while nonce != 0 {
                nonce_vec.push((nonce & 0xFF).try_into().unwrap());
                nonce >>= 8;
            }
            nonce_vec.reverse();
            let mut stream = RlpStream::new_list(2);
            stream.append(&from_vec).append(&nonce_vec);
            Address(stream.out().to_vec().keccak256() & u256::from_str_hex("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").unwrap())
        } else {
            *to
        }
    }

    pub fn is_contract_creation(&self) -> bool {
        self.to.0 == U256::ZERO
    }

    pub fn intrinsic_gas_cost(&self) -> usize {
        21000 +
            if self.is_contract_creation() { 32000 } else { 0 } +
            self.data.iter().map(|b| if *b == 0 { 4 } else { 16 }).sum::<usize>()
    }
}

#[cfg(test)]
mod tests {
    use ethnum::uint;
    use super::*;

    #[test]
    fn intrinsic_gas_cost() {
        let tx1 = Transaction {
            data: hex::decode("4200").unwrap(),
            from: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            gas: 1,
            gas_price: 1,
            to: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0D")),
            nonce: 0,
            value: uint!("4"),
        };
        assert_eq!(tx1.intrinsic_gas_cost(), 21020);

        let tx2 = Transaction {
            data: hex::decode("42002025").unwrap(),
            from: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            gas: 1,
            gas_price: 1,
            to: Address(uint!("0")),
            nonce: 7,
            value: uint!("4"),
        };
        assert_eq!(tx2.intrinsic_gas_cost(), 53052);
    }

    #[test]
    fn contract_address() {
        let transaction = Transaction {
            data: Default::default(),
            from: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            gas: 1,
            gas_price: 1,
            to: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0D")),
            nonce: 0,
            value: uint!("4"),
        };
        assert_eq!(transaction.contract_address(), Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0D")));
    }

    #[test]
    fn contract_address_creation() {
        let transaction = Transaction {
            data: Default::default(),
            from: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            gas: 1,
            gas_price: 1,
            to: Address(uint!("0")),
            nonce: 7,
            value: uint!("4"),
        };
        assert_eq!(transaction.nonce, 7);
        assert_eq!(transaction.contract_address(), Address(uint!("0xD0CB8E86E90C8170565878A666070ADD140B39D3"))); // keccak256(rlp([from, nonce]))
        assert_eq!(transaction.nonce, 7);
    }

    #[test]
    fn is_contract_creation() {
        assert!(!Transaction {
            data: Default::default(),
            from: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            gas: 1,
            gas_price: 1,
            to: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0D")),
            nonce: 0,
            value: uint!("4"),
        }.is_contract_creation());
        assert!(Transaction {
            data: Default::default(),
            from: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            gas: 1,
            gas_price: 1,
            to: Address::default(),
            nonce: 0,
            value: uint!("4"),
        }.is_contract_creation());
    }

    #[test]
    fn u256_try_into_address() {
        assert_eq!(TryInto::<Address>::try_into(uint!("0x372BDB7F2E599AD23590DAEAF0490D46185BEC962CAC93120B52389748E99C0C")), Err(crate::errors::Error::InvalidAddress));
        assert_eq!(TryInto::<Address>::try_into(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")), Ok(Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C"))));
    }
}
