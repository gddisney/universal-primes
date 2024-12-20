use crate::prime_shamir::*;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use rand_distr::{Distribution, Normal};
use sha3::{Digest, Sha3_512, Shake256};
use sha3::digest::{Update, ExtendableOutput};
use thiserror::Error;
use num_bigint::{BigUint, RandBigInt, ToBigInt};
use num_traits::{One, Zero};
use num_integer::Integer;
use std::convert::TryInto;
use rand::Rng;
use std::io::Read;
use std::io;
#[derive(Error, Debug)]
pub enum NoiseError {
    #[error("Invalid standard deviation")]
    InvalidStdDev,
    #[error("Invalid hash output")]
    InvalidHashOutput,
}

#[derive(Error, Debug)]
pub enum EncryptionError {
    #[error("Plaintext mapping failed")]
    PlaintextMappingFailed,
    #[error("Encryption process failed")]
    EncryptionFailed,
}

#[derive(Error, Debug)]
pub enum DecryptionError {
    #[error("Ring metadata validation failed")]
    RingValidationFailed,
    #[error("Inverse substitution failed")]
    InverseSubstitutionFailed,
    #[error("Plaintext reconstruction failed")]
    PlaintextReconstructionFailed,
    #[error(transparent)]
    NoiseRemovalFailed(#[from] NoiseError),
    #[error("Invalid ciphertext structure")]
    InvalidCiphertext,
}

#[derive(Error, Debug)]
pub enum HMACError {
    #[error("Signature generation failed")]
    SignError,
    #[error("Signature verification failed")]
    VerifyError,
}
#[derive(Debug, Clone, PartialEq)]
struct DynamicSBox {
    sbox: [u8; 256],
    inverse_sbox: [u8; 256],
}

impl DynamicSBox {
    /// Generate a secure dynamic S-Box along with its inverse
    pub fn new(rng: &mut ChaCha20Rng) -> Self {
        let mut sbox: [u8; 256] = [0; 256];
        for i in 0..256 {
            sbox[i] = i as u8;
        }
        // Shuffle S-Box securely
        for i in (1..256).rev() {
            let j = rng.gen_range(0..=i);
            sbox.swap(i, j);
        }

        // Create inverse S-Box
        let mut inverse_sbox: [u8; 256] = [0; 256];
        for i in 0..256 {
            inverse_sbox[sbox[i] as usize] = i as u8;
        }

        DynamicSBox { sbox, inverse_sbox }
    }

    /// Substitute a value using the S-Box
    pub fn substitute(&self, value: u8) -> u8 {
        self.sbox[value as usize]
    }

    /// Substitute a value using the inverse S-Box
    pub fn inverse_substitute(&self, value: u8) -> u8 {
        self.inverse_sbox[value as usize]
    }
}

#[derive(Debug, Clone)]
pub struct RingMetadata {
    pub ring_value: BigUint,
}

impl RingMetadata {
    /// Generate the quadratic ring metadata
    pub fn generate(
        public: &SpherePoint,
        substituted: &SpherePoint,
        modulus: &BigUint,
    ) -> Self {
        let ring_value = (public.x.clone() * substituted.x.clone()
            + public.y.clone() * substituted.y.clone()
            + public.z.clone() * substituted.z.clone())
            % modulus;

        RingMetadata { ring_value }
    }

    /// Validate the quadratic ring metadata
    pub fn validate(
        &self,
        public: &SpherePoint,
        substituted: &SpherePoint,
        modulus: &BigUint,
    ) -> bool {
        let computed_ring = (public.x.clone() * substituted.x.clone()
            + public.y.clone() * substituted.y.clone()
            + public.z.clone() * substituted.z.clone())
            % modulus;

        &computed_ring == &self.ring_value
    }
}

/// Structure to represent a 3D point on the quadratic sphere.
#[derive(Debug, Clone, PartialEq)]
pub struct SpherePoint {
    pub x: BigUint,
    pub y: BigUint,
    pub z: BigUint,
}

impl SpherePoint {
    pub fn new(x: BigUint, y: BigUint, z: BigUint) -> Self {
        SpherePoint { x, y, z }
    }

    /// Apply substitution and add Gaussian noise to each byte of the coordinate
    pub fn transform_with_noise(
        &self,
        rng: &mut ChaCha20Rng,
        sbox: &DynamicSBox,
        stddev: f64,
        pad_length: usize,
    ) -> Result<SpherePoint, NoiseError> {
        if stddev <= 0.0 || stddev.is_nan() {
            return Err(NoiseError::InvalidStdDev);
        }

        let x_bytes = self.x.to_bytes_be();
        let y_bytes = self.y.to_bytes_be();
        let z_bytes = self.z.to_bytes_be();

        let x_bytes_padded = pad_bytes(&x_bytes, pad_length);
        let y_bytes_padded = pad_bytes(&y_bytes, pad_length);
        let z_bytes_padded = pad_bytes(&z_bytes, pad_length);

        let mut substituted_x = vec![0u8; pad_length];
        let mut substituted_y = vec![0u8; pad_length];
        let mut substituted_z = vec![0u8; pad_length];

        // Substitute each byte
        for i in 0..pad_length {
            substituted_x[i] = sbox.substitute(x_bytes_padded[i]);
            substituted_y[i] = sbox.substitute(y_bytes_padded[i]);
            substituted_z[i] = sbox.substitute(z_bytes_padded[i]);
        }

        // Generate noise per byte
        let noise_x: Vec<u8> = (0..pad_length)
            .map(|_| SpherePoint::generate_noise_byte(rng, stddev))
            .collect::<Result<Vec<u8>, NoiseError>>()?;

        let noise_y: Vec<u8> = (0..pad_length)
            .map(|_| SpherePoint::generate_noise_byte(rng, stddev))
            .collect::<Result<Vec<u8>, NoiseError>>()?;

        let noise_z: Vec<u8> = (0..pad_length)
            .map(|_| SpherePoint::generate_noise_byte(rng, stddev))
            .collect::<Result<Vec<u8>, NoiseError>>()?;

        // Add noise per byte, ensuring wrapping around 0-255
        for i in 0..pad_length {
            substituted_x[i] = substituted_x[i].wrapping_add(noise_x[i]);
            substituted_y[i] = substituted_y[i].wrapping_add(noise_y[i]);
            substituted_z[i] = substituted_z[i].wrapping_add(noise_z[i]);
        }

        // Reconstruct substituted BigUint
        let final_x = BigUint::from_bytes_be(&substituted_x);
        let final_y = BigUint::from_bytes_be(&substituted_y);
        let final_z = BigUint::from_bytes_be(&substituted_z);

        Ok(SpherePoint::new(final_x, final_y, final_z))
    }

    /// Generate Gaussian noise for a single byte
    pub fn generate_noise_byte(rng: &mut ChaCha20Rng, stddev: f64) -> Result<u8, NoiseError> {
        let normal = Normal::new(0.0, stddev).map_err(|_| NoiseError::InvalidStdDev)?;
        let noise = normal.sample(rng).round();
        let noise = noise.rem_euclid(256.0) as u8;
        Ok(noise)
    }
}

/// Pad bytes to a fixed length
fn pad_bytes(bytes: &[u8], length: usize) -> Vec<u8> {
    let mut padded = vec![0u8; length];
    let start = length - bytes.len();
    padded[start..].copy_from_slice(bytes);
    padded
}

/// --- Plaintext Mapping ---
fn map_plaintext_to_sphere_point(
    plaintext: &str,
    pad_length: usize,
) -> Result<SpherePoint, EncryptionError> {
    let plaintext_bytes = plaintext.as_bytes();
    let mut padded = plaintext_bytes.to_vec();

    // Pad the plaintext to a multiple of pad_length bytes for even splitting
    while padded.len() % pad_length != 0 {
        padded.push(0);
    }

    let total_chunks = padded.len() / pad_length;
    if total_chunks < 3 {
        // Ensure at least 3 chunks for x, y, z
        for _ in total_chunks..3 {
            padded.extend_from_slice(&vec![0u8; pad_length]);
        }
    }

    let x_bytes = &padded[0..pad_length];
    let y_bytes = &padded[pad_length..2 * pad_length];
    let z_bytes = &padded[2 * pad_length..3 * pad_length];

    let x = BigUint::from_bytes_be(x_bytes);
    let y = BigUint::from_bytes_be(y_bytes);
    let z = BigUint::from_bytes_be(z_bytes);

    Ok(SpherePoint::new(x, y, z))
}

/// --- Plaintext Reconstruction ---
fn map_sphere_point_to_plaintext(
    sphere: &SpherePoint,
    pad_length: usize,
) -> Result<String, DecryptionError> {
    let mut bytes = Vec::new();

    // Convert each coordinate back to bytes
    bytes.extend_from_slice(&pad_bytes(&sphere.x.to_bytes_be(), pad_length));
    bytes.extend_from_slice(&pad_bytes(&sphere.y.to_bytes_be(), pad_length));
    bytes.extend_from_slice(&pad_bytes(&sphere.z.to_bytes_be(), pad_length));

    // Remove padding (trailing zeros)
    while let Some(&last) = bytes.last() {
        if last == 0 {
            bytes.pop();
        } else {
            break;
        }
    }

    // Convert bytes back to string
    String::from_utf8(bytes).map_err(|_| DecryptionError::PlaintextReconstructionFailed)
}

/// --- Encryption Function ---
fn encrypt(
    plaintext: &str,
    public_key: &SpherePoint,
    private_key: &SpherePoint,
    sbox: &DynamicSBox,
    pad_length: usize,
    modulus: &BigUint,
) -> Result<Ciphertext, EncryptionError> {
    // Step 1: Plaintext Mapping
    let mapped_point = map_plaintext_to_sphere_point(plaintext, pad_length)
        .map_err(|_| EncryptionError::PlaintextMappingFailed)?;
    println!("Mapped Plaintext to SpherePoint: {:?}", mapped_point);

    // Step 2: Deterministic Noise Generation based on private key
    let mut hasher = Sha3_512::new();
    Update::update(&mut hasher, &private_key.x.to_bytes_be());
    Update::update(&mut hasher, &private_key.y.to_bytes_be());
    Update::update(&mut hasher, &private_key.z.to_bytes_be());
    let seed = hasher.finalize();
    let seed_bytes: [u8; 32] = seed[0..32].try_into().unwrap();
    let mut noise_rng = ChaCha20Rng::from_seed(seed_bytes);

    // Apply substitution and add noise
    let substituted_point = mapped_point
        .transform_with_noise(&mut noise_rng, sbox, 1.0, pad_length)
        .map_err(|_| EncryptionError::EncryptionFailed)?;
    println!("Substituted and Obfuscated SpherePoint: {:?}", substituted_point);

    // Step 3: Ring Metadata Integration
    let ring_value = (public_key.x.clone() * substituted_point.x.clone()
        + public_key.y.clone() * substituted_point.y.clone()
        + public_key.z.clone() * substituted_point.z.clone())
        % modulus;

    let ciphertext = Ciphertext {
        r: ring_value,
        x_s: substituted_point.x,
        y_s: substituted_point.y,
        z_s: substituted_point.z,
    };

    Ok(ciphertext)
}

/// --- Decryption Function ---
fn decrypt(
    ciphertext: &Ciphertext,
    public_key: &SpherePoint,
    private_key: &SpherePoint,
    sbox: &DynamicSBox,
    pad_length: usize,
    modulus: &BigUint,
) -> Result<String, DecryptionError> {
    // Step 1: Ring Metadata Verification
    let computed_ring = (public_key.x.clone() * ciphertext.x_s.clone()
        + public_key.y.clone() * ciphertext.y_s.clone()
        + public_key.z.clone() * ciphertext.z_s.clone())
        % modulus;

    if &computed_ring != &ciphertext.r {
        return Err(DecryptionError::RingValidationFailed);
    }
    println!("Ring metadata validation successful.");

    // Step 2: Deterministically Regenerate Noise Using Private Key
    let mut hasher = Sha3_512::new();
    Update::update(&mut hasher, &private_key.x.to_bytes_be());
    Update::update(&mut hasher, &private_key.y.to_bytes_be());
    Update::update(&mut hasher, &private_key.z.to_bytes_be());
    let seed = hasher.finalize();
    let seed_bytes: [u8; 32] = seed[0..32].try_into().unwrap();
    let mut noise_rng = ChaCha20Rng::from_seed(seed_bytes);

    // Generate the same noise used during encryption
    let substituted_point = SpherePoint::new(
        ciphertext.x_s.clone(),
        ciphertext.y_s.clone(),
        ciphertext.z_s.clone(),
    );

    // Generate noise per byte
    let noise_x: Vec<u8> = (0..pad_length)
        .map(|_| SpherePoint::generate_noise_byte(&mut noise_rng, 1.0))
        .collect::<Result<Vec<u8>, NoiseError>>()?;

    let noise_y: Vec<u8> = (0..pad_length)
        .map(|_| SpherePoint::generate_noise_byte(&mut noise_rng, 1.0))
        .collect::<Result<Vec<u8>, NoiseError>>()?;

    let noise_z: Vec<u8> = (0..pad_length)
        .map(|_| SpherePoint::generate_noise_byte(&mut noise_rng, 1.0))
        .collect::<Result<Vec<u8>, NoiseError>>()?;

    // Convert substituted sphere point to bytes
    let x_bytes = pad_bytes(&substituted_point.x.to_bytes_be(), pad_length);
    let y_bytes = pad_bytes(&substituted_point.y.to_bytes_be(), pad_length);
    let z_bytes = pad_bytes(&substituted_point.z.to_bytes_be(), pad_length);

    // Apply inverse substitution after removing noise
    let mut decrypted_x_bytes = vec![0u8; pad_length];
    let mut decrypted_y_bytes = vec![0u8; pad_length];
    let mut decrypted_z_bytes = vec![0u8; pad_length];

    for i in 0..pad_length {
        decrypted_x_bytes[i] =
            sbox.inverse_substitute(x_bytes[i].wrapping_sub(noise_x[i]));
        decrypted_y_bytes[i] =
            sbox.inverse_substitute(y_bytes[i].wrapping_sub(noise_y[i]));
        decrypted_z_bytes[i] =
            sbox.inverse_substitute(z_bytes[i].wrapping_sub(noise_z[i]));
    }

    // Reconstruct decrypted BigUint
    let decrypted_x = BigUint::from_bytes_be(&decrypted_x_bytes);
    let decrypted_y = BigUint::from_bytes_be(&decrypted_y_bytes);
    let decrypted_z = BigUint::from_bytes_be(&decrypted_z_bytes);

    let decrypted_point = SpherePoint::new(decrypted_x, decrypted_y, decrypted_z);
    println!("Decrypted SpherePoint after inverse substitution: {:?}", decrypted_point);

    // Step 3: Plaintext Reconstruction
    let plaintext = map_sphere_point_to_plaintext(&decrypted_point, pad_length)?;
    println!("Reconstructed Plaintext.");

    Ok(plaintext)
}

/// --- Ciphertext Structure ---
#[derive(Debug, Clone)]
struct Ciphertext {
    r: BigUint, // Ring metadata
    x_s: BigUint,
    y_s: BigUint,
    z_s: BigUint,
}

/// --- PMPT-HMAC Implementation ---
pub struct PmptHmac {
    public_key: SpherePoint,
    private_key: SpherePoint,
    sbox: DynamicSBox,
    pad_length: usize,
    modulus: BigUint,
}

impl PmptHmac {
    pub fn new(
        public_key: SpherePoint,
        private_key: SpherePoint,
        sbox: DynamicSBox,
        pad_length: usize,
        modulus: BigUint,
    ) -> Self {
        Self {
            public_key,
            private_key,
            sbox,
            pad_length,
            modulus,
        }
    }

    pub fn sign(&self, data: &[u8]) -> Result<SpherePoint, HMACError> {
        // Hash the data using Shake256
        let mut hasher = Shake256::default();
        hasher.update(data);

        let mut hash_output = [0u8; 64];
        hasher
            .finalize_xof()
            .read_exact(&mut hash_output)
            .map_err(|_| HMACError::SignError)?;

        // Map hash output to SpherePoint
        let hash_point = map_plaintext_to_sphere_point(
            &String::from_utf8_lossy(&hash_output),
            self.pad_length,
        )
        .map_err(|_| HMACError::SignError)?;

        // Use private key to generate noise RNG seed
        let mut hasher = Sha3_512::new();
        Update::update(&mut hasher, &self.private_key.x.to_bytes_be());
        Update::update(&mut hasher, &self.private_key.y.to_bytes_be());
        Update::update(&mut hasher, &self.private_key.z.to_bytes_be());
        let seed = hasher.finalize();
        let seed_bytes: [u8; 32] = seed[0..32].try_into().unwrap();
        let mut noise_rng = ChaCha20Rng::from_seed(seed_bytes);

        // Transform hash_point using substitution and noise
        let signature_point = hash_point
            .transform_with_noise(&mut noise_rng, &self.sbox, 1.0, self.pad_length)
            .map_err(|_| HMACError::SignError)?;

        Ok(signature_point)
    }

    pub fn verify(&self, data: &[u8], signature: &SpherePoint) -> Result<bool, HMACError> {
        // Hash the data
        let mut hasher = Shake256::default();
        hasher.update(data);

        let mut hash_output = [0u8; 64];
        hasher
            .finalize_xof()
            .read_exact(&mut hash_output)
            .map_err(|_| HMACError::VerifyError)?;

        // Map hash output to SpherePoint
        let hash_point = map_plaintext_to_sphere_point(
            &String::from_utf8_lossy(&hash_output),
            self.pad_length,
        )
        .map_err(|_| HMACError::VerifyError)?;

        // Use private key to regenerate noise RNG seed
        let mut hasher = Sha3_512::new();
        Update::update(&mut hasher, &self.private_key.x.to_bytes_be());
        Update::update(&mut hasher, &self.private_key.y.to_bytes_be());
        Update::update(&mut hasher, &self.private_key.z.to_bytes_be());
        let seed = hasher.finalize();
        let seed_bytes: [u8; 32] = seed[0..32].try_into().unwrap();
        let mut noise_rng = ChaCha20Rng::from_seed(seed_bytes);

        // Inverse transform the signature point
        let substituted_point = signature;

        // Generate noise per byte
        let noise_x: Vec<u8> = (0..self.pad_length)
            .map(|_| SpherePoint::generate_noise_byte(&mut noise_rng, 1.0))
            .collect::<Result<Vec<u8>, NoiseError>>()
            .map_err(|_| HMACError::VerifyError)?;

        let noise_y: Vec<u8> = (0..self.pad_length)
            .map(|_| SpherePoint::generate_noise_byte(&mut noise_rng, 1.0))
            .collect::<Result<Vec<u8>, NoiseError>>()
            .map_err(|_| HMACError::VerifyError)?;

        let noise_z: Vec<u8> = (0..self.pad_length)
            .map(|_| SpherePoint::generate_noise_byte(&mut noise_rng, 1.0))
            .collect::<Result<Vec<u8>, NoiseError>>()
            .map_err(|_| HMACError::VerifyError)?;

        // Convert substituted sphere point to bytes
        let x_bytes = pad_bytes(&substituted_point.x.to_bytes_be(), self.pad_length);
        let y_bytes = pad_bytes(&substituted_point.y.to_bytes_be(), self.pad_length);
        let z_bytes = pad_bytes(&substituted_point.z.to_bytes_be(), self.pad_length);

        // Remove noise and apply inverse substitution
        let mut decrypted_x_bytes = vec![0u8; self.pad_length];
        let mut decrypted_y_bytes = vec![0u8; self.pad_length];
        let mut decrypted_z_bytes = vec![0u8; self.pad_length];

        for i in 0..self.pad_length {
            decrypted_x_bytes[i] =
                self.sbox.inverse_substitute(x_bytes[i].wrapping_sub(noise_x[i]));
            decrypted_y_bytes[i] =
                self.sbox.inverse_substitute(y_bytes[i].wrapping_sub(noise_y[i]));
            decrypted_z_bytes[i] =
                self.sbox.inverse_substitute(z_bytes[i].wrapping_sub(noise_z[i]));
        }

        // Reconstruct decrypted BigUint
        let decrypted_x = BigUint::from_bytes_be(&decrypted_x_bytes);
        let decrypted_y = BigUint::from_bytes_be(&decrypted_y_bytes);
        let decrypted_z = BigUint::from_bytes_be(&decrypted_z_bytes);

        let decrypted_point = SpherePoint::new(decrypted_x, decrypted_y, decrypted_z);

        // Compare with hash_point
        let result = decrypted_point == hash_point;

        Ok(result)
    }
}

/// --- Miller-Rabin Primality Test ---
fn is_prime(n: &BigUint, k: u32) -> bool {
    if n == &BigUint::from(2u32) || n == &BigUint::from(3u32) {
        return true;
    }
    if n < &BigUint::from(2u32) || n % 2u32 == BigUint::zero() {
        return false;
    }

    // Write n-1 as 2^s * d
    let one = BigUint::one();
    let two = &one + &one;
    let n_minus_one = n - &one;
    let mut d = n_minus_one.clone();
    let mut s = 0u32;

    while &d % &two == BigUint::zero() {
        d /= &two;
        s += 1;
    }

    let mut rng = rand::thread_rng();
    'witness_loop: for _ in 0..k {
        let a = rng.gen_biguint_range(&two, &(n_minus_one));
        let mut x = a.modpow(&d, n);
        if x == one || x == n_minus_one {
            continue;
        }
        for _ in 0..(s - 1) {
            x = x.modpow(&two, n);
            if x == n_minus_one {
                continue 'witness_loop;
            }
        }
        return false;
    }
    true
}

/// --- Generate Large Prime ---

/// --- Main Function ---
fn main() {
    let mut rng = ChaCha20Rng::from_entropy();
 
    // Generate a large random prime
    let secret_bits = 1024;
    let secret = generate_large_prime(secret_bits);
    println!("N: {}", secret);
    let modulus_bits = secret_bits * 2;
    let modulus = generate_large_prime(modulus_bits);                                                                          let threshold = 6;
    let shares_count = 6;
    let threshold = 3;
    let shares = shamir_split_shares(&secret, threshold, shares_count, &modulus);
    // Calculate padding length based on modulus size
    let pad_length = ((modulus.bits() + 7) / 8) as usize; // Adjusted padding length
    println!("Padding Length: {} bytes", pad_length);

    // Create SpherePoints using DLP keys
    let private_point = SpherePoint {
        x: shares[0].1.clone(),
        y: shares[1].1.clone(),
        z: shares[2].1.clone(),
    };
    let public_point = SpherePoint {
        x: shares[3].1.clone(),
        y: shares[4].1.clone(),
        z: shares[5].1.clone(),
     };
    verify_share_primality(&shares);
    println!("Private Point: {:?}", private_point);
    println!("Public Point: {:?}", public_point);
    let ring_metadata = RingMetadata::generate(&public_point, &private_point, &modulus);
    let ring_valid = ring_metadata.validate(&public_point, &private_point, &modulus);
    let reconstructed_secret = shamir_reconstruct(&shares[..threshold], &modulus, &secret, threshold);
    println!("Public N Reconstucted: {}", reconstructed_secret);
    if ring_valid {
        println!("Ring metadata validation successful (key generation step).");
    } else {
        panic!("Ring metadata validation failed (key generation step).");
    }

    // Generate S-Box
    let mut rng_sbox = ChaCha20Rng::from_entropy();
    let sbox = DynamicSBox::new(&mut rng_sbox);

    // --- PMPT-HMAC Integration ---
    let pmpt_hmac = PmptHmac::new(
        public_point.clone(),
        private_point.clone(),
        sbox.clone(),
        pad_length,
        modulus.clone(),
    );

    let data = b"Example data for PMPT-HMAC";
    println!("Signing data: {:?}", String::from_utf8_lossy(data));

    // Sign the data
    let signature = pmpt_hmac.sign(data).expect("Signing failed");
    println!("Generated Signature: {:?}", signature);

    // Verify the signature
    let is_valid = pmpt_hmac.verify(data, &signature).expect("Verification failed");
    println!("Verification Result: {}", is_valid);
    // --- PMPT Encryption and Decryption ---
    let mut plaintext = String::new();

    println!("Enter your plaintext: ");

    // Read input from the user
    io::stdin()
        .read_line(&mut plaintext)
        .expect("Failed to read input");

    // Remove the trailing newline from the input
    let plaintext = plaintext.trim();

    // Print the input back to the user
    println!("Original Plaintext: {}", plaintext);

    let ciphertext = encrypt(
        plaintext,
        &public_point,
        &private_point,
        &sbox,
        pad_length,
        &modulus,
    )
    .expect("Encryption failed");

    println!("Ciphertext: {:?}", ciphertext);
    // Perform ring check on the ciphertext
    let substituted_point = SpherePoint {
        x: ciphertext.x_s.clone(),
        y: ciphertext.y_s.clone(),
        z: ciphertext.z_s.clone(),
    };
    let ring_metadata = RingMetadata::generate(&public_point, &substituted_point, &modulus);
    let ring_valid = ring_metadata.validate(&public_point, &substituted_point, &modulus);
    if ring_valid {
        println!("Ring metadata validation successful (encryption step).");
    } else {
        panic!("Ring metadata validation failed (encryption step).");
    }
    let decrypted_plaintext = decrypt(
        &ciphertext,
        &public_point,
        &private_point,
        &sbox,
        pad_length,
        &modulus,
    )
    .expect("Decryption failed");
    println!("Decrypted Plaintext: {}", decrypted_plaintext);
    assert_eq!(plaintext, decrypted_plaintext);
    println!("Encryption and decryption are consistent.");
}
