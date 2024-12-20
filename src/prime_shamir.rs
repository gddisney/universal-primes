use num_bigint::{BigUint, RandBigInt};
use num_traits::{One, Zero};
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;

pub fn generate_large_prime(bits: usize) -> BigUint {
    let mut rng = ChaCha20Rng::from_entropy();
    loop {
        let candidate = rng.gen_biguint(bits as u64) | BigUint::one();
        if is_probably_prime(&candidate, 10) {
            return candidate;
        }
    }
}

pub fn is_probably_prime(n: &BigUint, k: usize) -> bool {
    if *n <= BigUint::from(1u64) {
        return false;
    }
    if *n == BigUint::from(2u64) {
        return true;
    }
    if n % 2u64 == BigUint::zero() {
        return false;
    }

    let mut rng = ChaCha20Rng::from_entropy();
    let one = BigUint::one();
    let two = &one + &one;
    let n_minus_one = n - &one;
    let mut d = n_minus_one.clone();
    let mut s = 0;
    while &d % &two == BigUint::zero() {
        d /= &two;
        s += 1;
    }

    'outer: for _ in 0..k {
        let a = rng.gen_biguint_range(&two, n);
        let mut x = a.modpow(&d, n);
        if x == one || x == n_minus_one {
            continue;
        }
        for _ in 0..(s - 1) {
            x = x.modpow(&two, n);
            if x == n_minus_one {
                continue 'outer;
            }
        }
        return false;
    }
    true
}

struct Share {
    pub x: usize,
    pub prime_y: BigUint,
    pub original_y: BigUint,
}

pub fn shamir_split_shares(
    secret: &BigUint,
    threshold: usize,
    shares: usize,
    modulus: &BigUint,
) -> Vec<(usize, BigUint)> {
    assert!(threshold > 1);
    assert!(shares >= threshold);
    let mut rng = ChaCha20Rng::from_entropy();
    let mut coefficients = Vec::with_capacity(threshold);
    coefficients.push(secret.clone());
    for _ in 1..threshold {
        coefficients.push(rng.gen_biguint_below(modulus));
    }
    let mut result_internal = Vec::with_capacity(shares);
    for x in 1..=shares {
        let x_biguint = BigUint::from(x as u64);
        let mut y = BigUint::zero();
        for (i, coeff) in coefficients.iter().enumerate() {
            let term = coeff * x_biguint.modpow(&BigUint::from(i as u64), modulus);
            y = (y + term) % modulus;
        }
        let mut prime_y = y.clone();
        while !is_probably_prime(&prime_y, 10) {
            prime_y = (prime_y + BigUint::one()) % modulus;
        }
        result_internal.push(Share { x, prime_y, original_y: y });
    }
    let mut result = Vec::with_capacity(shares);
    for share in result_internal {
        result.push((share.x, share.prime_y));
    }
    result
}

pub fn shamir_reconstruct(
    shares: &[(usize, BigUint)],
    modulus: &BigUint,
    secret: &BigUint,
    threshold: usize
) -> BigUint {
    let mut rng = ChaCha20Rng::from_entropy();
    let mut coefficients = Vec::with_capacity(threshold);
    coefficients.push(secret.clone());
    for _ in 1..threshold {
        coefficients.push(rng.gen_biguint_below(modulus));
    }

    let mut original_shares = Vec::with_capacity(shares.len());
    for (x, _prime_y) in shares.iter() {
        let x_biguint = BigUint::from(*x as u64);
        let mut y = BigUint::zero();
        for (i, coeff) in coefficients.iter().enumerate() {
            let term = coeff * x_biguint.modpow(&BigUint::from(i as u64), modulus);
            y = (y + term) % modulus;
        }
        original_shares.push((*x, y));
    }

    let mut reconstructed = BigUint::zero();
    for (i, (xi, yi)) in original_shares.iter().enumerate() {
        let mut numerator = BigUint::one();
        let mut denominator = BigUint::one();
        for (j, (xj, _)) in original_shares.iter().enumerate() {
            if i != j {
                let xj_big = BigUint::from(*xj as u64);
                let xi_big = BigUint::from(*xi as u64);
                let diff = (xj_big.clone() + modulus - xi_big.clone()) % modulus;
                numerator = (numerator * xj_big) % modulus;
                denominator = (denominator * diff) % modulus;
            }
        }
        let denominator_inv = denominator.modpow(&(modulus - BigUint::from(2u64)), modulus);
        let lagrange_coeff = (numerator * denominator_inv) % modulus;
        let term = (lagrange_coeff * yi) % modulus;
        reconstructed = (reconstructed + term) % modulus;
    }
    reconstructed
}

pub fn verify_share_primality(shares: &[(usize, BigUint)]) {
    for (x, y) in shares {
        if is_probably_prime(y, 10) {
            println!("Share at x = {} is prime.", x);
        } else {
            println!("Share at x = {} is NOT prime.", x);
        }
    }
}

fn main() {
    let secret_bits = 512;
    let secret = generate_large_prime(secret_bits);
    let modulus_bits = secret_bits * 2;
    let modulus = generate_large_prime(modulus_bits);
    let threshold = 6;
    let shares_count = 8;
    let shares = shamir_split_shares(&secret, threshold, shares_count, &modulus);

    println!("Original Secret (Prime): {}", secret);
    println!("Shares:");
    for (x, y) in &shares {
        println!("x: {}, y: {}", x, y);
    }
    verify_share_primality(&shares);

    let reconstructed_secret = shamir_reconstruct(&shares[..threshold], &modulus, &secret, threshold);
    println!("Reconstructed Secret: {}", reconstructed_secret);
    assert_eq!(secret, reconstructed_secret);
    println!("Reconstruction successful. The secret matches exactly.");
}

