// Holographic Reduced Representation (Plate 1995).
// Adapted from /home/josh/soft/manas/sutra/src/hrr.rs with parameterized dimensionality.

use std::f64::consts::PI;

use super::VsaOps;

#[derive(Clone)]
pub struct Hrr {
    dim: usize,
}

impl Hrr {
    pub fn new(dim: usize) -> Self {
        assert!(dim.is_power_of_two(), "HRR dim must be power of two for FFT");
        Self { dim }
    }
}

#[derive(Clone, Copy)]
struct Complex {
    re: f64,
    im: f64,
}

impl Complex {
    fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    fn mul(self, other: Self) -> Self {
        Self {
            re: self.re * other.re - self.im * other.im,
            im: self.re * other.im + self.im * other.re,
        }
    }

    fn conj(self) -> Self {
        Self {
            re: self.re,
            im: -self.im,
        }
    }
}

impl std::ops::Add for Complex {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            re: self.re + other.re,
            im: self.im + other.im,
        }
    }
}

impl std::ops::Sub for Complex {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self {
            re: self.re - other.re,
            im: self.im - other.im,
        }
    }
}

fn fft(buf: &mut [Complex], inverse: bool) {
    let n = buf.len();
    assert!(n.is_power_of_two());

    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            buf.swap(i, j);
        }
    }

    let mut len = 2;
    while len <= n {
        let half = len / 2;
        let angle = 2.0 * PI / len as f64 * if inverse { -1.0 } else { 1.0 };
        let wn = Complex::new(angle.cos(), angle.sin());

        for start in (0..n).step_by(len) {
            let mut w = Complex::new(1.0, 0.0);
            for k in 0..half {
                let u = buf[start + k];
                let t = w.mul(buf[start + k + half]);
                buf[start + k] = u + t;
                buf[start + k + half] = u - t;
                w = w.mul(wn);
            }
        }
        len <<= 1;
    }

    if inverse {
        let inv_n = 1.0 / n as f64;
        for x in buf.iter_mut() {
            x.re *= inv_n;
            x.im *= inv_n;
        }
    }
}

fn to_freq(data: &[f64]) -> Vec<Complex> {
    let mut buf: Vec<Complex> = data.iter().map(|&x| Complex::new(x, 0.0)).collect();
    fft(&mut buf, false);
    buf
}

fn from_freq(buf: &mut [Complex]) -> Vec<f64> {
    fft(buf, true);
    buf.iter().map(|c| c.re).collect()
}

fn convolve(a: &[f64], b: &[f64]) -> Vec<f64> {
    let mut fa = to_freq(a);
    let fb = to_freq(b);
    for i in 0..fa.len() {
        fa[i] = fa[i].mul(fb[i]);
    }
    from_freq(&mut fa)
}

fn correlate(a: &[f64], b: &[f64]) -> Vec<f64> {
    let mut fa = to_freq(a);
    let fb = to_freq(b);
    for i in 0..fa.len() {
        fa[i] = fa[i].mul(fb[i].conj());
    }
    from_freq(&mut fa)
}

fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let nb: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    if na < 1e-15 || nb < 1e-15 {
        return 0.0;
    }
    dot / (na * nb)
}

fn normalize(v: &[f64]) -> Vec<f64> {
    let n: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    if n < 1e-15 {
        return v.to_vec();
    }
    v.iter().map(|x| x / n).collect()
}

fn elementwise_add(a: &[f64], b: &[f64]) -> Vec<f64> {
    a.iter().zip(b).map(|(x, y)| x + y).collect()
}

struct Rng {
    state: [u64; 2],
}

impl Rng {
    fn new(seed: u64) -> Self {
        Self {
            state: [
                seed ^ 0x123456789abcdef0,
                seed.wrapping_mul(6364136223846793005),
            ],
        }
    }

    fn next_u64(&mut self) -> u64 {
        let s0 = self.state[0];
        let mut s1 = self.state[1];
        let result = s0.wrapping_add(s1);
        s1 ^= s0;
        self.state[0] = s0.rotate_left(55) ^ s1 ^ (s1 << 14);
        self.state[1] = s1.rotate_left(36);
        result
    }

    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn next_gaussian(&mut self) -> f64 {
        let u1 = self.next_f64().max(1e-15);
        let u2 = self.next_f64();
        (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos()
    }
}

impl VsaOps for Hrr {
    type Vector = Vec<f64>;

    fn dim(&self) -> usize {
        self.dim
    }

    fn random_vector(&self, seed: u64) -> Vec<f64> {
        let mut rng = Rng::new(seed);
        let scale = 1.0 / (self.dim as f64).sqrt();
        (0..self.dim).map(|_| rng.next_gaussian() * scale).collect()
    }

    fn bind(&self, a: &Vec<f64>, b: &Vec<f64>) -> Vec<f64> {
        convolve(a, b)
    }

    fn unbind(&self, a: &Vec<f64>, b: &Vec<f64>) -> Vec<f64> {
        correlate(a, b)
    }

    fn bundle(&self, vecs: &[Vec<f64>]) -> Vec<f64> {
        assert!(!vecs.is_empty());
        if vecs.len() == 1 {
            return vecs[0].clone();
        }
        let mut sum = vecs[0].clone();
        for v in &vecs[1..] {
            sum = elementwise_add(&sum, v);
        }
        normalize(&sum)
    }

    fn similarity(&self, a: &Vec<f64>, b: &Vec<f64>) -> f64 {
        cosine_similarity(a, b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vsa::fnv1a;

    fn make_ops() -> Hrr {
        Hrr::new(1024)
    }

    fn seeded_vec(ops: &Hrr, label: &str) -> Vec<f64> {
        ops.random_vector(fnv1a(label.as_bytes()))
    }

    #[test]
    fn random_vectors_roughly_orthogonal() {
        let ops = make_ops();
        let a = seeded_vec(&ops, "alpha");
        let b = seeded_vec(&ops, "beta");
        let sim = ops.similarity(&a, &b);
        assert!(sim.abs() < 0.15, "expected ~0, got {sim:.4}");
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
        assert!(sim_a.abs() < 0.15, "sim_a={sim_a:.4}");
        assert!(sim_b.abs() < 0.15, "sim_b={sim_b:.4}");
    }

    #[test]
    fn bind_unbind_roundtrip() {
        let ops = make_ops();
        let a = seeded_vec(&ops, "alpha");
        let b = seeded_vec(&ops, "beta");
        let bound = ops.bind(&a, &b);
        let recovered = ops.unbind(&bound, &b);
        let sim = ops.similarity(&recovered, &a);
        assert!(sim > 0.6, "bind-unbind sim={sim:.4}, expected >0.6");
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
        assert!(sim_a > 0.3, "sim_a={sim_a:.4}");
        assert!(sim_b > 0.3, "sim_b={sim_b:.4}");
        assert!(sim_c > 0.3, "sim_c={sim_c:.4}");
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
        assert!(
            sim_correct > 0.1,
            "sim_correct={sim_correct:.4}, expected meaningful similarity"
        );
    }
}
