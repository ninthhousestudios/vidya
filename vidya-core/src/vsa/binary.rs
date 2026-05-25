// binary bipolar is an alternative architecture to hrr
// we decided that hrr was more suitable for vidya; see yojana task vidya/28 for the spike results
use super::VsaOps;

#[derive(Clone)]
pub struct BinaryBipolar {
    dim: usize,
    byte_len: usize,
}

impl BinaryBipolar {
    pub fn new(dim: usize) -> Self {
        let byte_len = (dim + 7) / 8;
        Self { dim, byte_len }
    }
}

struct Xoshiro128 {
    state: [u32; 4],
}

impl Xoshiro128 {
    fn from_seed(seed: u64) -> Self {
        let mut s = [0u32; 4];
        s[0] = seed as u32;
        s[1] = (seed >> 32) as u32;
        s[2] = seed.wrapping_mul(0x9e3779b97f4a7c15) as u32;
        s[3] = (seed.wrapping_mul(0x9e3779b97f4a7c15) >> 32) as u32;
        if s == [0; 4] {
            s[0] = 1;
        }
        Self { state: s }
    }

    fn next_u32(&mut self) -> u32 {
        let result = (self.state[1].wrapping_mul(5))
            .rotate_left(7)
            .wrapping_mul(9);
        let t = self.state[1] << 9;
        self.state[2] ^= self.state[0];
        self.state[3] ^= self.state[1];
        self.state[1] ^= self.state[2];
        self.state[0] ^= self.state[3];
        self.state[2] ^= t;
        self.state[3] = self.state[3].rotate_left(11);
        result
    }

    fn fill_bytes(&mut self, buf: &mut [u8]) {
        let mut i = 0;
        while i < buf.len() {
            let word = self.next_u32().to_le_bytes();
            let remaining = buf.len() - i;
            let copy_len = remaining.min(4);
            buf[i..i + copy_len].copy_from_slice(&word[..copy_len]);
            i += copy_len;
        }
    }
}

impl VsaOps for BinaryBipolar {
    type Vector = Vec<u8>;

    fn dim(&self) -> usize {
        self.dim
    }

    fn random_vector(&self, seed: u64) -> Vec<u8> {
        let mut rng = Xoshiro128::from_seed(seed);
        let mut buf = vec![0u8; self.byte_len];
        rng.fill_bytes(&mut buf);
        if self.dim % 8 != 0 {
            let mask = (1u8 << (self.dim % 8)) - 1;
            buf[self.byte_len - 1] &= mask;
        }
        buf
    }

    fn bind(&self, a: &Vec<u8>, b: &Vec<u8>) -> Vec<u8> {
        a.iter().zip(b.iter()).map(|(x, y)| x ^ y).collect()
    }

    fn unbind(&self, a: &Vec<u8>, b: &Vec<u8>) -> Vec<u8> {
        self.bind(a, b)
    }

    fn bundle(&self, vecs: &[Vec<u8>]) -> Vec<u8> {
        assert!(!vecs.is_empty());
        if vecs.len() == 1 {
            return vecs[0].clone();
        }

        let mut accum = vec![0i16; self.dim];
        for v in vecs {
            for bit_idx in 0..self.dim {
                let byte_idx = bit_idx / 8;
                let bit_pos = bit_idx % 8;
                if (v[byte_idx] >> bit_pos) & 1 == 1 {
                    accum[bit_idx] += 1;
                } else {
                    accum[bit_idx] -= 1;
                }
            }
        }

        let threshold = 0i16;
        let mut result = vec![0u8; self.byte_len];
        for bit_idx in 0..self.dim {
            let val = accum[bit_idx];
            let set = if val > threshold {
                true
            } else if val < threshold {
                false
            } else {
                bit_idx % 2 == 0
            };
            if set {
                result[bit_idx / 8] |= 1 << (bit_idx % 8);
            }
        }
        result
    }

    fn similarity(&self, a: &Vec<u8>, b: &Vec<u8>) -> f64 {
        let mut hamming = 0u32;
        for (x, y) in a.iter().zip(b.iter()) {
            hamming += (x ^ y).count_ones();
        }
        1.0 - (hamming as f64 / self.dim as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vsa::fnv1a;

    fn make_ops() -> BinaryBipolar {
        BinaryBipolar::new(4096)
    }

    fn seeded_vec(ops: &BinaryBipolar, label: &str) -> Vec<u8> {
        ops.random_vector(fnv1a(label.as_bytes()))
    }

    #[test]
    fn random_vectors_roughly_orthogonal() {
        let ops = make_ops();
        let a = seeded_vec(&ops, "alpha");
        let b = seeded_vec(&ops, "beta");
        let sim = ops.similarity(&a, &b);
        assert!((sim - 0.5).abs() < 0.05, "expected ~0.5, got {sim:.4}");
    }

    #[test]
    fn same_seed_same_vector() {
        let ops = make_ops();
        let a = seeded_vec(&ops, "alpha");
        let b = seeded_vec(&ops, "alpha");
        assert_eq!(a, b);
    }

    #[test]
    fn bind_produces_dissimilar() {
        let ops = make_ops();
        let a = seeded_vec(&ops, "alpha");
        let b = seeded_vec(&ops, "beta");
        let bound = ops.bind(&a, &b);
        let sim_a = ops.similarity(&bound, &a);
        let sim_b = ops.similarity(&bound, &b);
        assert!((sim_a - 0.5).abs() < 0.05, "sim_a={sim_a:.4}");
        assert!((sim_b - 0.5).abs() < 0.05, "sim_b={sim_b:.4}");
    }

    #[test]
    fn bind_unbind_exact_roundtrip() {
        let ops = make_ops();
        let a = seeded_vec(&ops, "alpha");
        let b = seeded_vec(&ops, "beta");
        let bound = ops.bind(&a, &b);
        let recovered = ops.unbind(&bound, &b);
        assert_eq!(recovered, a, "XOR unbind should be exact");
    }

    #[test]
    fn bundle_preserves_similarity() {
        let ops = make_ops();
        let a = seeded_vec(&ops, "alpha");
        let b = seeded_vec(&ops, "beta");
        let c = seeded_vec(&ops, "gamma");
        let bundled = ops.bundle(&[a.clone(), b.clone(), c.clone()]);
        let sim_a = ops.similarity(&bundled, &a);
        let sim_b = ops.similarity(&bundled, &b);
        let sim_c = ops.similarity(&bundled, &c);
        assert!(sim_a > 0.55, "sim_a={sim_a:.4}");
        assert!(sim_b > 0.55, "sim_b={sim_b:.4}");
        assert!(sim_c > 0.55, "sim_c={sim_c:.4}");
    }

    #[test]
    fn bundle_of_bound_pairs_unbind_recovery() {
        let ops = make_ops();
        let role_a = seeded_vec(&ops, "role_a");
        let role_b = seeded_vec(&ops, "role_b");
        let filler_a = seeded_vec(&ops, "filler_a");
        let filler_b = seeded_vec(&ops, "filler_b");

        let pair_a = ops.bind(&role_a, &filler_a);
        let pair_b = ops.bind(&role_b, &filler_b);
        let bundled = ops.bundle(&[pair_a, pair_b]);

        let recovered = ops.unbind(&bundled, &role_a);
        let sim_correct = ops.similarity(&recovered, &filler_a);
        let sim_wrong = ops.similarity(&recovered, &filler_b);

        assert!(
            sim_correct > sim_wrong,
            "correct={sim_correct:.4}, wrong={sim_wrong:.4}"
        );
    }
}
