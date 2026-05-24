pub mod binary;
pub mod hrr;
pub mod index;

pub use binary::BinaryBipolar;
pub use hrr::Hrr;
pub use index::EntityIndex;

pub trait VsaOps: Clone {
    type Vector: Clone;

    fn dim(&self) -> usize;
    fn random_vector(&self, seed: u64) -> Self::Vector;
    fn bind(&self, a: &Self::Vector, b: &Self::Vector) -> Self::Vector;
    fn unbind(&self, a: &Self::Vector, b: &Self::Vector) -> Self::Vector;
    fn bundle(&self, vecs: &[Self::Vector]) -> Self::Vector;
    fn similarity(&self, a: &Self::Vector, b: &Self::Vector) -> f64;
}

pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
