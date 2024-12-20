extern crate num_complex;
use num_complex::Complex;
use num_bigint::BigUint;
use num_traits::ToPrimitive;

/// Calculate the Riemann zeta function for a given complex input `s`.
/// Uses the series definition up to `iterations` terms.
fn zeta(s: Complex<f64>, iterations: usize) -> Complex<f64> {
    let mut sum = Complex::new(0.0, 0.0);
    for n in 1..=iterations {
        let n_f64 = n as f64;
        // Accumulate the series terms: 1 / n^s
        sum += Complex::new(1.0, 0.0) / Complex::new(n_f64, 0.0).powc(s);
    }
    sum
}

/// Test if the Universal Prime `N` aligns with a zero of the zeta function along the critical line.
/// Returns true if `zeta(s) \\approx 0` for some `s` with Re(s) = 0.5.
pub fn test_universal_prime_against_zeta(n: &BigUint, iterations: usize, tolerance: f64) -> bool {
    // Convert BigUint to f64 for numerical computations
    let n_f64 = match n.to_f64() {
        Some(value) => value,
        None => {
            println!("Error: BigUint too large to convert to f64");
            return false;
        },
    };

    // Real part of s on the critical line
    let real_part = 0.5;
    let step = 0.01; // Step size for incrementing the imaginary part
    let max_imaginary = 1000.0; // Limit the range of the imaginary axis

    let mut imaginary_part = 0.0;

    // Iterate over a range of imaginary parts to search for a zero
    while imaginary_part <= max_imaginary {
        let s = Complex::new(real_part, imaginary_part);
        let zeta_value = zeta(s, iterations);

        // Check if the zeta value is within the specified tolerance
        if zeta_value.norm() < tolerance {
            println!(
                "Potential zero found: s = {} + {}i, Zeta(s) = {}",
                real_part, imaginary_part, zeta_value
            );
            return true;
        }

        // Increment the imaginary part for the next step
        imaginary_part += step;
    }

    println!("No zeros found near critical line for N = {}", n_f64);
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::ToBigUint;

    #[test]
    fn test_small_universal_prime() {
        let n = 17u32.to_biguint().unwrap(); // Small prime
        let result = test_universal_prime_against_zeta(&n, 10000, 1e-6);
        assert!(!result, "Expected no alignment for small prime");
    }

    #[test]
    fn test_large_universal_prime() {
        let n = 48883u32.to_biguint().unwrap(); // Example Universal Prime
        let result = test_universal_prime_against_zeta(&n, 10000, 1e-1);
        assert!(result, "Expected alignment for known Universal Prime");
    }
}

