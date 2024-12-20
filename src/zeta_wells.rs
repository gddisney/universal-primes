use num_bigint::BigUint;
use num_traits::Zero;
use crate::pmpt::*;
use log::{info, debug, error};

pub fn detect_anomalous_primes(primes: Vec<BigUint>, chaotic_points: Vec<SpherePoint>) -> Vec<BigUint> {
    let mut anomalous_primes = Vec::new();

    for universal_prime in primes.iter() {
        let mut ring_values = Vec::new();

        for point in chaotic_points.iter() {
            // Generate substituted points (perturb chaos)
            let substituted_point = SpherePoint::new(
                point.x.clone() + 1u32,
                point.y.clone() + 2u32,
                point.z.clone() + 3u32,
            );

            // Compute ring metadata directly without match
            let ring_metadata = RingMetadata::generate(point, &substituted_point, universal_prime);

            // Store ring value
            ring_values.push(ring_metadata.ring_value.clone());
        }

        // Analyze entropy or anomalies in ring values
        if is_anomalous(&ring_values) {
            anomalous_primes.push(universal_prime.clone());
        }
    }

    anomalous_primes
}

pub fn is_anomalous(ring_values: &Vec<BigUint>) -> bool {
    // Perform entropy analysis or identify anomalous clusters
    let entropy = compute_entropy(ring_values);
    entropy < 1e-9 // Define threshold for anomaly detection
}

pub fn compute_entropy(ring_values: &Vec<BigUint>) -> f64 {
    // Compute Shannon entropy or other statistical measures
    let mut frequency_map = std::collections::HashMap::new();

    for value in ring_values {
        *frequency_map.entry(value.clone()).or_insert(0) += 1;
    }

    let total = ring_values.len() as f64;
    frequency_map.values().map(|&count| {
        let p = count as f64 / total;
        -p * p.log2()
    }).sum()
}

