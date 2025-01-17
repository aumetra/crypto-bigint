//! Modular reduction implementation.

use crate::{Limb, Uint, WideWord, Word};

#[cfg(feature = "alloc")]
use {crate::BoxedUint, subtle::Choice};

/// Returns `(hi, lo)` such that `hi * R + lo = x * y + z + w`.
#[inline(always)]
const fn muladdcarry(x: Word, y: Word, z: Word, w: Word) -> (Word, Word) {
    let res = (x as WideWord)
        .wrapping_mul(y as WideWord)
        .wrapping_add(z as WideWord)
        .wrapping_add(w as WideWord);
    ((res >> Word::BITS) as Word, res as Word)
}

/// Implement the Montgomery reduction algorithm.
///
/// This is implemented as a macro to abstract over `const fn` and boxed use cases, since the latter
/// needs mutable references and thus the unstable `const_mut_refs` feature (rust-lang/rust#57349).
// TODO(tarcieri): change this into a `const fn` when `const_mut_refs` is stable
macro_rules! impl_montgomery_reduction {
    ($upper:expr, $lower:expr, $modulus:expr, $mod_neg_inv:expr, $limbs:expr) => {{
        let mut meta_carry = Limb(0);
        let mut new_sum;

        let mut i = 0;
        while i < $limbs {
            let u = $lower[i].0.wrapping_mul($mod_neg_inv.0);

            let (mut carry, _) = muladdcarry(u, $modulus[0].0, $lower[i].0, 0);
            let mut new_limb;

            let mut j = 1;
            while j < ($limbs - i) {
                (carry, new_limb) = muladdcarry(u, $modulus[j].0, $lower[i + j].0, carry);
                $lower[i + j] = Limb(new_limb);
                j += 1;
            }
            while j < $limbs {
                (carry, new_limb) = muladdcarry(u, $modulus[j].0, $upper[i + j - $limbs].0, carry);
                $upper[i + j - $limbs] = Limb(new_limb);
                j += 1;
            }

            (new_sum, meta_carry) = $upper[i].adc(Limb(carry), meta_carry);
            $upper[i] = new_sum;

            i += 1;
        }

        meta_carry
    }};
}

/// Algorithm 14.32 in Handbook of Applied Cryptography <https://cacr.uwaterloo.ca/hac/about/chap14.pdf>
pub const fn montgomery_reduction<const LIMBS: usize>(
    lower_upper: &(Uint<LIMBS>, Uint<LIMBS>),
    modulus: &Uint<LIMBS>,
    mod_neg_inv: Limb,
) -> Uint<LIMBS> {
    let (mut lower, mut upper) = *lower_upper;
    let meta_carry =
        impl_montgomery_reduction!(upper.limbs, lower.limbs, &modulus.limbs, mod_neg_inv, LIMBS);

    // Division is simply taking the upper half of the limbs
    // Final reduction (at this point, the value is at most 2 * modulus,
    // so `meta_carry` is either 0 or 1)
    upper.sub_mod_with_carry(meta_carry, modulus, modulus)
}

/// Algorithm 14.32 in Handbook of Applied Cryptography <https://cacr.uwaterloo.ca/hac/about/chap14.pdf>
///
/// This version writes the result into the provided [`BoxedUint`].
#[cfg(feature = "alloc")]
pub(crate) fn montgomery_reduction_boxed_mut(
    x: &mut BoxedUint,
    modulus: &BoxedUint,
    mod_neg_inv: Limb,
    out: &mut BoxedUint,
) {
    debug_assert_eq!(x.nlimbs(), modulus.nlimbs() * 2);
    debug_assert_eq!(out.nlimbs(), modulus.nlimbs());

    let (lower, upper) = x.limbs.split_at_mut(modulus.nlimbs());
    let meta_carry =
        impl_montgomery_reduction!(upper, lower, &modulus.limbs, mod_neg_inv, modulus.nlimbs());

    out.limbs.copy_from_slice(upper);
    let borrow = out.sbb_assign(modulus, Limb::ZERO);

    // The new `borrow = Word::MAX` iff `carry == 0` and `borrow == Word::MAX`.
    let borrow = Limb((!meta_carry.0.wrapping_neg()) & borrow.0);

    // If underflow occurred on the final limb, borrow = 0xfff...fff, otherwise
    // borrow = 0x000...000. Thus, we use it as a mask to conditionally add the modulus.
    out.conditional_adc_assign(modulus, Choice::from((borrow.0 & 1) as u8));
}

/// Algorithm 14.32 in Handbook of Applied Cryptography <https://cacr.uwaterloo.ca/hac/about/chap14.pdf>
///
/// This version allocates and returns a [`BoxedUint`].
#[cfg(feature = "alloc")]
#[inline]
pub(crate) fn montgomery_reduction_boxed(
    x: &mut BoxedUint,
    modulus: &BoxedUint,
    mod_neg_inv: Limb,
) -> BoxedUint {
    let mut ret = BoxedUint::zero_with_precision(modulus.bits_precision());
    montgomery_reduction_boxed_mut(x, modulus, mod_neg_inv, &mut ret);
    ret
}
