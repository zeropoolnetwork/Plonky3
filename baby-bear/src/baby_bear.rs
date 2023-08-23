use core::fmt::{self, Debug, Display, Formatter};
use core::iter::{Product, Sum};
use core::ops::{Add, AddAssign, Div, Mul, MulAssign, Neg, Sub, SubAssign};

use p3_field::{AbstractField, Field, PrimeField, PrimeField32, PrimeField64, TwoAdicField};
use rand::distributions::{Distribution, Standard};
use rand::Rng;

const P: u32 = 0x78000001;
const MONTY_BITS: u32 = 31;
const MONTY_MASK: u32 = (1 << MONTY_BITS) - 1;
const MONTY_MU: u32 = 0x8000001;

/// The prime field `2^31 - 2^27 + 1`, a.k.a. the Baby Bear field.
#[derive(Copy, Clone, Default, Eq, Hash, PartialEq)]
#[repr(transparent)] // `PackedBabyBearNeon` relies on this!
pub struct BabyBear {
    value: u32,
}

impl Ord for BabyBear {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.as_canonical_u32().cmp(&other.as_canonical_u32())
    }
}

impl PartialOrd for BabyBear {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Display for BabyBear {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.as_canonical_u32(), f)
    }
}

impl Debug for BabyBear {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.as_canonical_u32(), f)
    }
}

impl Distribution<BabyBear> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> BabyBear {
        loop {
            let next_u31 = rng.next_u32() & 0x7ffffff;
            let is_canonical = next_u31 < P;
            if is_canonical {
                return BabyBear { value: next_u31 };
            }
        }
    }
}

impl AbstractField for BabyBear {
    const ZERO: Self = Self { value: 0 };
    const ONE: Self = Self { value: 0x7ffffff };
    const TWO: Self = Self { value: 0xffffffe };
    const NEG_ONE: Self = Self { value: 0x70000002 };

    fn from_bool(b: bool) -> Self {
        Self::from_canonical_u32(b as u32)
    }

    fn from_canonical_u8(n: u8) -> Self {
        Self::from_canonical_u32(n as u32)
    }

    fn from_canonical_u16(n: u16) -> Self {
        Self::from_canonical_u32(n as u32)
    }

    fn from_canonical_u32(n: u32) -> Self {
        debug_assert!(n < P);
        Self::from_wrapped_u32(n)
    }

    fn from_canonical_u64(n: u64) -> Self {
        debug_assert!(n < P as u64);
        Self::from_canonical_u32(n as u32)
    }

    fn from_canonical_usize(n: usize) -> Self {
        debug_assert!(n < P as usize);
        Self::from_canonical_u32(n as u32)
    }

    fn from_wrapped_u32(n: u32) -> Self {
        Self { value: to_monty(n) }
    }

    fn from_wrapped_u64(n: u64) -> Self {
        Self {
            value: to_monty_64(n),
        }
    }

    fn multiplicative_group_generator() -> Self {
        Self::from_canonical_u32(0x1f)
    }
}

impl Field for BabyBear {
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    type Packing = crate::PackedBabyBearNeon;
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    type Packing = Self;

    fn try_inverse(&self) -> Option<Self> {
        if self.is_zero() {
            return None;
        }

        // From Fermat's little theorem, in a prime field `F_p`, the inverse of `a` is `a^(p-2)`.
        // Here p-2 = 2013265919.
        // We use an addition chain for 2013265919 generated by https://github.com/str4d/addchain

        let x1 = *self;
        let x2 = x1.square();
        let x3 = x1 * x2;
        let x6 = x3 * x3;
        let x7 = x1 * x6;
        let x56 = x7.exp_power_of_2(3);
        let x63 = x7 * x56;
        let x4032 = x63.exp_power_of_2(6);
        let x4095 = x63 * x4032;
        let x8190 = x4095.square();
        let x12285 = x4095 * x8190;
        let x24570 = x12285.square();
        let x28665 = x4095 * x24570;
        let x28672 = x7 * x28665;
        let x32767 = x4095 * x28672;
        let x61439 = x28672 * x32767;
        let x2013233152 = x61439.exp_power_of_2(15);
        let x2013265919 = x32767 * x2013233152;

        Some(x2013265919)
    }
}

impl PrimeField for BabyBear {}

impl PrimeField64 for BabyBear {
    const ORDER_U64: u64 = <Self as PrimeField32>::ORDER_U32 as u64;

    fn as_canonical_u64(&self) -> u64 {
        u64::from(self.as_canonical_u32())
    }

    fn linear_combination_u64<const N: usize>(u: [u64; N], v: &[Self; N]) -> Self {
        // In order not to overflow a u64, we must have sum(u) <= 2^32.
        debug_assert!(u.iter().sum::<u64>() <= (1u64 << 32));

        let mut dot = u[0] * v[0].value as u64;
        for i in 1..N {
            dot += u[i] * v[i].value as u64;
        }
        Self {
            value: (dot % (P as u64)) as u32,
        }
    }
}

impl PrimeField32 for BabyBear {
    const ORDER_U32: u32 = P;

    fn as_canonical_u32(&self) -> u32 {
        from_monty(self.value)
    }
}

impl TwoAdicField for BabyBear {
    const TWO_ADICITY: usize = 27;

    fn power_of_two_generator() -> Self {
        Self::from_canonical_u32(0x1a427a41)
    }
}

impl Add for BabyBear {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        let mut sum = self.value + rhs.value;
        if sum >= P {
            sum -= P;
        }
        Self { value: sum }
    }
}

impl AddAssign for BabyBear {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sum for BabyBear {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.reduce(|x, y| x + y).unwrap_or(Self::ZERO)
    }
}

impl Sub for BabyBear {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        BabyBear {
            value: canonical_sub(self.value, rhs.value),
        }
    }
}

impl SubAssign for BabyBear {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl Neg for BabyBear {
    type Output = Self;

    fn neg(self) -> Self::Output {
        BabyBear {
            value: canonical_sub(0, self.value),
        }
    }
}

impl Mul for BabyBear {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        let long_prod = self.value as u64 * rhs.value as u64;
        Self {
            value: monty_reduce(long_prod),
        }
    }
}

impl MulAssign for BabyBear {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl Product for BabyBear {
    fn product<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.reduce(|x, y| x * y).unwrap_or(Self::ONE)
    }
}

impl Div for BabyBear {
    type Output = Self;

    #[must_use]
    #[allow(clippy::suspicious_arithmetic_impl)]
    fn div(self, rhs: Self) -> Self {
        self * rhs.inverse()
    }
}

#[must_use]
fn canonical_sub(x: u32, y: u32) -> u32 {
    let (mut diff, over) = x.overflowing_sub(y);
    if over {
        diff = diff.wrapping_add(P);
    }
    diff
}

#[must_use]
fn to_monty(x: u32) -> u32 {
    (((x as u64) << 31) % P as u64) as u32
}

#[must_use]
fn to_monty_64(x: u64) -> u32 {
    (((x as u128) << 31) % P as u128) as u32
}

#[must_use]
fn from_monty(x: u32) -> u32 {
    monty_reduce(x as u64)
}

/// Split unsigned integer of width `2 * MONTY_BITS` into two unsigned integers
/// of `MONTY_BITS` `(lo, hi)`.
#[must_use]
fn monty_split_double(x: u64) -> (u32, u32) {
    let lo = x as u32 & MONTY_MASK;
    let hi = (x >> MONTY_BITS) as u32;
    (lo, hi)
}

/// Multiply two unsigned integers of width `MONTY_BITS`, returning the low
/// `MONTY_BITS` of the result.
#[must_use]
fn monty_mul_lo(x: u32, y: u32) -> u32 {
    x.wrapping_mul(y) & MONTY_MASK
}

/// Multiply two unsigned integers of width `MONTY_BITS`, returning the high
/// `MONTY_BITS` of the result.
#[must_use]
fn monty_mul_hi(x: u32, y: u32) -> u32 {
    let long_prod = (x as u64) * (y as u64);
    (long_prod >> MONTY_BITS) as u32
}

/// Montgomery reduction of a value in `0..P << MONTY_BITS`.
#[must_use]
fn monty_reduce(x: u64) -> u32 {
    let (x_lo, x_hi) = monty_split_double(x);

    let t = monty_mul_lo(MONTY_MU, x_lo);
    let u = monty_mul_hi(t, P);

    // Observe that `x_hi` and `u` are both in `0..P`.
    canonical_sub(x_hi, u)
}

#[cfg(test)]
mod tests {
    use p3_field::PrimeField64;
    use p3_field_testing::{
        test_inverse, test_two_adic_coset_zerofier, test_two_adic_subgroup_zerofier,
    };

    use super::*;

    type F = BabyBear;

    #[test]
    fn test_baby_bear() {
        let f = F::from_canonical_u32(100);
        assert_eq!(f.as_canonical_u64(), 100);

        let f = F::from_canonical_u32(0);
        assert!(f.is_zero());

        let f = F::from_wrapped_u32(F::ORDER_U32);
        assert!(f.is_zero());

        let f_1 = F::ONE;
        let f_1_copy = F::from_canonical_u32(1);

        let expected_result = F::ZERO;
        assert_eq!(f_1 - f_1_copy, expected_result);

        let expected_result = F::TWO;
        assert_eq!(f_1 + f_1_copy, expected_result);

        let f_2 = F::from_canonical_u32(2);
        let expected_result = F::from_canonical_u32(3);
        assert_eq!(f_1 + f_1_copy * f_2, expected_result);

        let expected_result = F::from_canonical_u32(5);
        assert_eq!(f_1 + f_2 * f_2, expected_result);

        let f_p_minus_1 = F::from_canonical_u32(F::ORDER_U32 - 1);
        let expected_result = F::ZERO;
        assert_eq!(f_1 + f_p_minus_1, expected_result);

        let f_p_minus_2 = F::from_canonical_u32(F::ORDER_U32 - 2);
        let expected_result = F::from_canonical_u32(F::ORDER_U32 - 3);
        assert_eq!(f_p_minus_1 + f_p_minus_2, expected_result);

        let expected_result = F::from_canonical_u32(1);
        assert_eq!(f_p_minus_1 - f_p_minus_2, expected_result);

        let expected_result = f_p_minus_1;
        assert_eq!(f_p_minus_2 - f_p_minus_1, expected_result);

        let expected_result = f_p_minus_2;
        assert_eq!(f_p_minus_1 - f_1, expected_result);

        let m1 = F::from_canonical_u32(0x34167c58);
        let m2 = F::from_canonical_u32(0x61f3207b);
        let expected_prod = F::from_canonical_u32(0x1b5c8046);
        assert_eq!(m1 * m2, expected_prod);
    }

    #[test]
    fn inverse() {
        test_inverse::<BabyBear>();
    }

    #[test]
    fn two_adic_subgroup_zerofier() {
        test_two_adic_subgroup_zerofier::<BabyBear>();
    }

    #[test]
    fn two_adic_coset_zerofier() {
        test_two_adic_coset_zerofier::<BabyBear>();
    }
}
