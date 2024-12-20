use num_bigint::*;
use num_traits::*;
use rand::Rng;

use std::fs::File;
use std::io::Write;

fn classify_prime(p: &BigUint) -> Vec<&'static str> {
    let mut classifications = Vec::new();

    // Check if it's a Germain prime
    if is_germain_prime(p) {
        classifications.push("Germain");
    }
    // Check if it's a Safe prime
    if is_safe_prime(p) {
        classifications.push("Safe");
    }
    // Check if it's a Prime (basic primality check)
    if is_prime(p, 20) {
        classifications.push("Prime");
    }

    classifications
}

fn is_germain_prime(p: &BigUint) -> bool {
    let two = BigUint::from(2u32);
    let q = p * &two + BigUint::one();
    is_prime(&q, 20)
}

fn is_safe_prime(p: &BigUint) -> bool {
    let two = BigUint::from(2u32);
    if p <= &two {
        return false;
    }
    let q = (p - BigUint::one()) / &two;
    is_prime(&q, 20)
}

fn is_prime(n: &BigUint, k: usize) -> bool {
    if n == &BigUint::from(2u32) || n == &BigUint::from(3u32) {
        return true;
    }
    if n < &BigUint::from(2u32) || n % BigUint::from(2u32) == BigUint::zero() {
        return false;
    }

    let mut d = n - BigUint::one();
    let mut s = 0usize;
    while &d % BigUint::from(2u32) == BigUint::zero() {
        d /= BigUint::from(2u32);
        s += 1;
    }

    let mut rng = rand::thread_rng();
    'witness_loop: for _ in 0..k {
        let a = rng.gen_biguint_range(&BigUint::from(2u32), &(n - BigUint::one()));
        let mut x = a.modpow(&d, n);
        if x == BigUint::one() || x == n - BigUint::one() {
            continue;
        }
        for _ in 0..s - 1 {
            x = x.modpow(&BigUint::from(2u32), n);
            if x == n - BigUint::one() {
                continue 'witness_loop;
            }
        }
        return false;
    }
    true
}

fn compute_n(x: &BigUint, y: &BigUint, z: &BigUint) -> BigUint {
    let a = BigUint::from(5u32);
    let b = BigUint::from(7u32);
    let c = BigUint::from(11u32);
    let d = BigUint::from(23u32);
    let e = BigUint::from(47u32);
    let f = BigUint::from(83u32);
    let g = BigUint::from(107u32);

    &a * x * x
        + &b * x * y
        + &c * y * y
        + &d * x * z
        + &e * y * z
        + &f * z * z
        + &g
}

fn main() {
    // Define the first few known primes
    let primes = vec![
    BigUint::from(3u32),
    BigUint::from(5u32),
    BigUint::from(7u32),
    BigUint::from(11u32),
    BigUint::from(13u32),
    BigUint::from(23u32),
    BigUint::from(47u32),
    BigUint::from(83u32),
    BigUint::from(107u32),
    BigUint::from(167u32),
    BigUint::from(227u32),
    BigUint::from(359u32),
    BigUint::from(383u32),
    BigUint::from(467u32),
    BigUint::from(479u32),
    BigUint::from(503u32),
    BigUint::from(563u32),
    BigUint::from(587u32),
    BigUint::from(719u32),
    BigUint::from(839u32),
    BigUint::from(863u32),
    BigUint::from(887u32),
    BigUint::from(983u32),
    BigUint::from(1019u32),
    BigUint::from(1187u32),
    BigUint::from(1283u32),
    BigUint::from(1307u32),
    BigUint::from(1319u32),
    BigUint::from(1367u32),
    BigUint::from(1439u32),
    BigUint::from(1487u32),
    BigUint::from(1523u32),
    BigUint::from(1619u32),
    BigUint::from(1823u32),
    BigUint::from(1907u32),
];


    // Create output file and write header
    let output_file = "universal_primes_index.csv";
    let mut file = File::create(output_file).expect("Failed to create output file.");
    writeln!(
        file,
        "x,y,z,n,classifications_n,classifications_x,classifications_y,classifications_z"
    )
    .expect("Failed to write header.");

    // Iterate through all combinations of (x, y, z)
    for x in &primes {
        for y in &primes {
            for z in &primes {
                let n = compute_n(x, y, z);

                let classifications_n = classify_prime(&n);

                // Proceed only if N is prime
                if classifications_n.contains(&"Prime") {
                    let classifications_x = classify_prime(x);
                    let classifications_y = classify_prime(y);
                    let classifications_z = classify_prime(z);

                    // Write to CSV file
                    writeln!(
                        file,
                        "{},{},{},{},{:?},{:?},{:?},{:?}",
                        x, y, z, n, classifications_n, classifications_x, classifications_y, classifications_z
                    )
                    .expect("Failed to write to CSV file.");
                }
            }
        }
    }

    println!("Data has been saved to {}", output_file);
}

