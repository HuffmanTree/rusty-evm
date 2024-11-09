use ethnum::u256;

pub trait NeededSizeInBytes { fn needed_size_in_bytes(self) -> u32; }

pub trait IsNeg { fn is_neg(&self) -> bool; }

trait Abs { fn abs(&self) -> Self; }

pub trait WrappingSignedDiv { fn wrapping_signed_div(&self, rhs: Self) -> Self; }

pub trait WrappingSignedRem { fn wrapping_signed_rem(&self, rhs: Self) -> Self; }

impl NeededSizeInBytes for u32 {
    fn needed_size_in_bytes(mut self) -> u32 {
        let mut n = 0_u32;
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
        if self.is_neg() { self.wrapping_neg() } else { self.clone() }
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

#[cfg(test)]
mod tests {
    use super::*;
    use ethnum::uint;

    #[test]
    fn u32_needed_size_in_bytes() {
        assert_eq!(0_u32.needed_size_in_bytes(), 0);
        assert_eq!(1_u32.needed_size_in_bytes(), 1);
        assert_eq!(2_u32.needed_size_in_bytes(), 1);

        assert_eq!(126_u32.needed_size_in_bytes(), 1);
        assert_eq!(127_u32.needed_size_in_bytes(), 1);
        assert_eq!(128_u32.needed_size_in_bytes(), 1);

        assert_eq!(254_u32.needed_size_in_bytes(), 1);
        assert_eq!(255_u32.needed_size_in_bytes(), 1);
        assert_eq!(256_u32.needed_size_in_bytes(), 2);
        assert_eq!(257_u32.needed_size_in_bytes(), 2);
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
}
