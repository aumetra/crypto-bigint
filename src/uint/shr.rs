//! [`Uint`] bitwise right shift operations.

use super::Uint;
use crate::{CtChoice, Limb};
use core::ops::{Shr, ShrAssign};

impl<const LIMBS: usize> Uint<LIMBS> {
    /// Computes `self << shift`.
    /// Returns zero if `shift >= Self::BITS`.
    pub const fn shr(&self, shift: u32) -> Self {
        let overflow = CtChoice::from_u32_lt(shift, Self::BITS).not();
        let shift = shift % Self::BITS;
        let mut result = *self;
        let mut i = 0;
        while i < Self::LOG2_BITS + 1 {
            let bit = CtChoice::from_u32_lsb((shift >> i) & 1);
            result = Uint::ct_select(&result, &result.shr_vartime(1 << i), bit);
            i += 1;
        }

        Uint::ct_select(&result, &Self::ZERO, overflow)
    }

    /// Computes `self >> shift`.
    ///
    /// NOTE: this operation is variable time with respect to `shift` *ONLY*.
    ///
    /// When used with a fixed `shift`, this function is constant-time with respect
    /// to `self`.
    #[inline(always)]
    pub const fn shr_vartime(&self, shift: u32) -> Self {
        let full_shifts = (shift / Limb::BITS) as usize;
        let small_shift = shift & (Limb::BITS - 1);
        let mut limbs = [Limb::ZERO; LIMBS];

        if shift > Self::BITS {
            return Self { limbs };
        }

        let shift = LIMBS - full_shifts;
        let mut i = 0;

        if small_shift == 0 {
            while i < shift {
                limbs[i] = Limb(self.limbs[i + full_shifts].0);
                i += 1;
            }
        } else {
            while i < shift {
                let mut lo = self.limbs[i + full_shifts].0 >> small_shift;

                if i < (LIMBS - 1) - full_shifts {
                    lo |= self.limbs[i + full_shifts + 1].0 << (Limb::BITS - small_shift);
                }

                limbs[i] = Limb(lo);
                i += 1;
            }
        }

        Self { limbs }
    }

    /// Computes a right shift on a wide input as `(lo, hi)`.
    ///
    /// NOTE: this operation is variable time with respect to `shift` *ONLY*.
    ///
    /// When used with a fixed `shift`, this function is constant-time with respect
    /// to `self`.
    #[inline(always)]
    pub const fn shr_vartime_wide(lower_upper: (Self, Self), shift: u32) -> (Self, Self) {
        let (mut lower, upper) = lower_upper;
        let new_upper = upper.shr_vartime(shift);
        lower = lower.shr_vartime(shift);
        if shift >= Self::BITS {
            lower = lower.bitor(&upper.shr_vartime(shift - Self::BITS));
        } else {
            lower = lower.bitor(&upper.shl_vartime(Self::BITS - shift));
        }

        (lower, new_upper)
    }

    /// Computes `self >> 1` in constant-time, returning [`CtChoice::TRUE`] if the overflowing bit
    /// was set, and [`CtChoice::FALSE`] otherwise.
    pub(crate) const fn shr1_with_overflow(&self) -> (Self, CtChoice) {
        let carry = CtChoice::from_word_lsb(self.limbs[0].0 & 1);
        let mut ret = Self::ZERO;
        ret.limbs[0] = self.limbs[0].shr(1);

        let mut i = 1;
        while i < LIMBS {
            // set carry bit
            ret.limbs[i - 1].0 |= (self.limbs[i].0 & 1) << Limb::HI_BIT;
            ret.limbs[i] = self.limbs[i].shr(1);
            i += 1;
        }

        (ret, carry)
    }

    /// Computes `self >> 1` in constant-time.
    pub(crate) const fn shr1(&self) -> Self {
        self.shr1_with_overflow().0
    }
}

impl<const LIMBS: usize> Shr<u32> for Uint<LIMBS> {
    type Output = Uint<LIMBS>;

    fn shr(self, shift: u32) -> Uint<LIMBS> {
        Uint::<LIMBS>::shr(&self, shift)
    }
}

impl<const LIMBS: usize> Shr<u32> for &Uint<LIMBS> {
    type Output = Uint<LIMBS>;

    fn shr(self, shift: u32) -> Uint<LIMBS> {
        self.shr(shift)
    }
}

impl<const LIMBS: usize> ShrAssign<u32> for Uint<LIMBS> {
    fn shr_assign(&mut self, shift: u32) {
        *self = self.shr(shift);
    }
}

#[cfg(test)]
mod tests {
    use crate::{Uint, U128, U256};

    const N: U256 =
        U256::from_be_hex("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141");

    const N_2: U256 =
        U256::from_be_hex("7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF5D576E7357A4501DDFE92F46681B20A0");

    #[test]
    fn shr1() {
        assert_eq!(N >> 1, N_2);
    }

    #[test]
    fn shr_wide_1_1_128() {
        assert_eq!(
            Uint::shr_vartime_wide((U128::ONE, U128::ONE), 128),
            (U128::ONE, U128::ZERO)
        );
    }

    #[test]
    fn shr_wide_0_max_1() {
        assert_eq!(
            Uint::shr_vartime_wide((U128::ZERO, U128::MAX), 1),
            (U128::ONE << 127, U128::MAX >> 1)
        );
    }

    #[test]
    fn shr_wide_max_max_256() {
        assert_eq!(
            Uint::shr_vartime_wide((U128::MAX, U128::MAX), 256),
            (U128::ZERO, U128::ZERO)
        );
    }
}
