//! Modular exponentiation support for [`BoxedResidue`].

use super::{mul::MontgomeryMultiplier, BoxedResidue};
use crate::{BoxedUint, Limb, PowBoundedExp, Word};
use alloc::vec::Vec;
use subtle::ConstantTimeEq;

impl BoxedResidue {
    /// Raises to the `exponent` power.
    pub fn pow(&self, exponent: &BoxedUint) -> Self {
        let ret = self.pow_bounded_exp(exponent, exponent.bits_precision());
        debug_assert!(ret.retrieve() < self.residue_params.modulus);
        ret
    }

    /// Raises to the `exponent` power,
    /// with `exponent_bits` representing the number of (least significant) bits
    /// to take into account for the exponent.
    ///
    /// NOTE: `exponent_bits` may be leaked in the time pattern.
    pub fn pow_bounded_exp(&self, exponent: &BoxedUint, exponent_bits: u32) -> Self {
        Self {
            montgomery_form: pow_montgomery_form(
                &self.montgomery_form,
                exponent,
                exponent_bits,
                &self.residue_params.modulus,
                &self.residue_params.r,
                self.residue_params.mod_neg_inv,
            ),
            residue_params: self.residue_params.clone(),
        }
    }
}

impl PowBoundedExp<BoxedUint> for BoxedResidue {
    fn pow_bounded_exp(&self, exponent: &BoxedUint, exponent_bits: u32) -> Self {
        self.pow_bounded_exp(exponent, exponent_bits)
    }
}

/// Performs modular exponentiation using Montgomery's ladder.
/// `exponent_bits` represents the number of bits to take into account for the exponent.
///
/// NOTE: this value is leaked in the time pattern.
fn pow_montgomery_form(
    x: &BoxedUint,
    exponent: &BoxedUint,
    exponent_bits: u32,
    modulus: &BoxedUint,
    r: &BoxedUint,
    mod_neg_inv: Limb,
) -> BoxedUint {
    if exponent_bits == 0 {
        return r.clone(); // 1 in Montgomery form
    }

    const WINDOW: u32 = 4;
    const WINDOW_MASK: Word = (1 << WINDOW) - 1;

    let mut multiplier = MontgomeryMultiplier::new(modulus, mod_neg_inv);

    // powers[i] contains x^i
    let mut powers = Vec::with_capacity(1 << WINDOW);
    powers.push(r.clone()); // 1 in Montgomery form
    powers.push(x.clone());

    for i in 2..(1 << WINDOW) {
        powers.push(multiplier.mul(&powers[i - 1], x));
    }

    let starting_limb = ((exponent_bits - 1) / Limb::BITS) as usize;
    let starting_bit_in_limb = (exponent_bits - 1) % Limb::BITS;
    let starting_window = starting_bit_in_limb / WINDOW;
    let starting_window_mask = (1 << (starting_bit_in_limb % WINDOW + 1)) - 1;

    let mut z = r.clone(); // 1 in Montgomery form
    let mut power = powers[0].clone();

    for limb_num in (0..=starting_limb).rev() {
        let w = exponent.as_limbs()[limb_num].0;

        let mut window_num = if limb_num == starting_limb {
            starting_window + 1
        } else {
            Limb::BITS / WINDOW
        };

        while window_num > 0 {
            window_num -= 1;

            let mut idx = (w >> (window_num * WINDOW)) & WINDOW_MASK;

            if limb_num == starting_limb && window_num == starting_window {
                idx &= starting_window_mask;
            } else {
                for _ in 1..=WINDOW {
                    multiplier.square_assign(&mut z);
                }
            }

            // Constant-time lookup in the array of powers
            power.limbs.copy_from_slice(&powers[0].limbs);
            for i in 1..(1 << WINDOW) {
                power.conditional_assign(&powers[i as usize], i.ct_eq(&idx));
            }

            multiplier.mul_assign(&mut z, &power);
        }
    }

    z
}
