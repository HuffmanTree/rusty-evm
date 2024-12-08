use ethnum::{u256, U256};
use sha3::{Digest, Keccak256};

pub trait NeededSizeInBytes { fn needed_size_in_bytes(self) -> usize; }

pub trait IsNeg { fn is_neg(&self) -> bool; }

trait Abs { fn abs(&self) -> Self; }

pub trait WrappingSignedDiv { fn wrapping_signed_div(&self, rhs: Self) -> Self; }

pub trait WrappingSignedRem { fn wrapping_signed_rem(&self, rhs: Self) -> Self; }

pub trait WrappingBigPow { fn wrapping_big_pow(&self, e: u256) -> u256; }

pub trait Hash { fn keccak256(&self) -> u256; }

impl Hash for Vec<u8> {
    fn keccak256(&self) -> u256 {
        let mut result = U256::ZERO;
        let mut hasher = Keccak256::new();
        hasher.update(self);
        let arr = hasher.finalize();

        for i in 0..32 {
            result <<= 8;
            result |= u256::from(match arr.get(i) {
                Some(v) => *v,
                None => 0,
            });
        }
        result
    }
}

impl NeededSizeInBytes for u256 {
    fn needed_size_in_bytes(mut self) -> usize {
        let mut n = 0_usize;
        while self != 0 {
            self >>= 8;
            n += 1;
        }
        n
    }
}

impl IsNeg for u256 {
    fn is_neg(&self) -> bool {
        (self & u256::from_str_hex("0x8000000000000000000000000000000000000000000000000000000000000000").unwrap()) != 0
    }
}

impl Abs for u256 {
    fn abs(&self) -> Self {
        if self.is_neg() { self.wrapping_neg() } else { *self }
    }
}

impl WrappingSignedDiv for u256 {
    fn wrapping_signed_div(&self, rhs: Self) -> Self {
        let negate = self.is_neg() ^ rhs.is_neg();
        let res = self.abs().wrapping_div(rhs.abs());
        if negate { res.wrapping_neg() } else { res }
    }
}

impl WrappingSignedRem for u256 {
    fn wrapping_signed_rem(&self, rhs: Self) -> Self {
        let negate = self.is_neg();
        let res = self.abs().wrapping_rem(rhs.abs());
        if negate { res.wrapping_neg() } else { res }
    }
}

impl WrappingBigPow for u256 {
    fn wrapping_big_pow(&self, e: u256) -> u256 {
        match TryInto::<u32>::try_into(e) {
            Ok(e) => self.wrapping_pow(e),
            _ => {
                let ep = e.div_euclid(u32::MAX.into());
                let r: u32 = e.rem_euclid(u32::MAX.into()).try_into().unwrap();
                self.wrapping_pow(u32::MAX).wrapping_big_pow(ep).wrapping_mul(self.wrapping_pow(r))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethnum::uint;

    #[test]
    fn u256_needed_size_in_bytes() {
        assert_eq!(uint!("0").needed_size_in_bytes(), 0);
        assert_eq!(uint!("1").needed_size_in_bytes(), 1);
        assert_eq!(uint!("2").needed_size_in_bytes(), 1);
        assert_eq!(uint!("126").needed_size_in_bytes(), 1);
        assert_eq!(uint!("127").needed_size_in_bytes(), 1);
        assert_eq!(uint!("128").needed_size_in_bytes(), 1);
        assert_eq!(uint!("254").needed_size_in_bytes(), 1);
        assert_eq!(uint!("255").needed_size_in_bytes(), 1);
        assert_eq!(uint!("256").needed_size_in_bytes(), 2);
        assert_eq!(uint!("257").needed_size_in_bytes(), 2);
    }

    #[test]
    fn u256_is_neg() {
        assert!(!uint!("6").is_neg());
        assert!(!uint!("10").is_neg());

        assert!(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE").is_neg());
        assert!(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").is_neg());
    }

    #[test]
    fn u256_abs() {
        assert_eq!(uint!("6").abs(), uint!("6"));
        assert_eq!(uint!("10").abs(), uint!("10"));

        assert_eq!(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE").abs(), uint!("2"));
        assert_eq!(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").abs(), uint!("1"));
    }

    #[test]
    #[should_panic(expected = "attempt to divide by zero")]
    fn u256_wrapping_signed_div() {
        assert_eq!(uint!("4").wrapping_signed_div(uint!("2")), uint!("2"));
        assert_eq!(uint!("4").wrapping_signed_div(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE"));
        assert_eq!(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFC").wrapping_signed_div(uint!("2")), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE"));
        assert_eq!(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFC").wrapping_signed_div(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")), uint!("2"));
        uint!("4").wrapping_signed_div(uint!("0"));
    }

    #[test]
    #[should_panic(expected = "attempt to calculate the remainder with a divisor of zero")]
    fn u256_wrapping_signed_rem() {
        assert_eq!(uint!("5").wrapping_signed_rem(uint!("2")), uint!("1"));
        assert_eq!(uint!("5").wrapping_signed_rem(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")), uint!("1"));
        assert_eq!(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFB").wrapping_signed_rem(uint!("2")), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"));
        assert_eq!(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFB").wrapping_signed_rem(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"));
        uint!("5").wrapping_signed_rem(uint!("0"));
    }

    #[test]
    fn u256_wrapping_big_pow() {
        assert_eq!(uint!("2").wrapping_big_pow(uint!("3")), uint!("8"));
        assert_eq!(uint!("5").wrapping_big_pow(uint!("20")), uint!("95367431640625"));
        assert_eq!(uint!("3").wrapping_big_pow(uint!("95367431640625")), uint!("0x44E51AFABFFC26671C3EC521656015E130346F0C26FE984F672212FD2EF68943"));
    }

    #[test]
    fn vec_u8_keccak256() {
        assert_eq!(vec![].keccak256(), uint!("0xC5D2460186F7233C927E7DB2DCC703C0E500B653CA82273B7BFAD8045D85A470"));
        assert_eq!(vec![0xF0, 0xBD, 0x5A, 0x61, 0x9C, 0xAD, 0x26, 0x29].keccak256(), uint!("0xE88A3F7420AC15E5F28B6260FF05BF4700AA744BC3C0C3F801C9EBC65AC260CA"));
        assert_eq!(vec![0x6A, 0x39, 0xC5, 0xC7, 0xA7, 0x07, 0x09, 0x46].keccak256(), uint!("0x1DB6F251E1D56865A76D5AF4AAD5108B958332250BE4B35BCD7F04147DC2E461"));
        assert_eq!(vec![0x90, 0x59, 0x06, 0x7B, 0x3C, 0xC9, 0x37, 0x5F, 0x89, 0xFB, 0x27].keccak256(), uint!("0xE2C182AA8D1BDEB12A9684B10B3477AED202CD2756542C68ADE59FCEF1DF812B"));
        assert_eq!(vec![0xBB, 0x7F, 0x48, 0xA4, 0xD9, 0x7C, 0xA3, 0xFC, 0xE2, 0xAF, 0xD2].keccak256(), uint!("0xD143127746860FF0809E71F49385047A4AB6E4F42CD6A76B348DC4735BB00231"));
        assert_eq!(vec![0xFF, 0x27, 0x22, 0x99, 0x1E, 0x15, 0x4A, 0x4E, 0x7B, 0x0B, 0xDA, 0xC4, 0xF5, 0x37, 0x95, 0x61, 0x9F, 0x68, 0x75, 0x10, 0x7A, 0xA9, 0x26, 0xEC, 0x04, 0x75, 0xE2, 0xB7, 0x89, 0xB1, 0x03, 0xEA, 0xF1, 0x23, 0xD2].keccak256(), uint!("0x4C8C638A6B0331B817AF4AE4F43A16380FE4C44DB50B58E1515E38C563B1D405"));
        assert_eq!(vec![0xE4, 0x0A, 0xA7, 0x56, 0x9B, 0x5B, 0x4D, 0xE7, 0xA4, 0x64, 0x68, 0xCD, 0x84, 0x1E, 0x49, 0xF4, 0xB0, 0x64, 0x96, 0x26, 0xC8, 0x44, 0x40, 0x72, 0x69, 0x8D, 0x5E, 0x42, 0xDA, 0x80, 0x39, 0x1B, 0xC5, 0xF9, 0x53].keccak256(), uint!("0x6588E86008B3DD13DA0E8391447CE4C6CD42C141C1F496F618B750ED1A102F7E"));
    }
}
