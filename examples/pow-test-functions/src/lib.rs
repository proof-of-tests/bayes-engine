#[inline]
fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}

#[inline]
fn avalanche(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ceb9fe1a85ec53);
    x ^ (x >> 33)
}

#[no_mangle]
pub extern "C" fn hash_splitmix(seed: u64) -> u64 {
    splitmix64(seed)
}

#[no_mangle]
pub extern "C" fn hash_twist(seed: u64) -> u64 {
    let a = splitmix64(seed ^ 0xfeedfacecafebeef);
    let b = splitmix64(seed.rotate_left(17) ^ 0x0123_4567_89ab_cdef);
    avalanche(a ^ b.rotate_right(11))
}

#[no_mangle]
pub extern "C" fn hash_murmurish(seed: u64) -> u64 {
    let mut x = seed;
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58476d1ce4e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d049bb133111eb);
    x ^= x >> 31;
    x
}

#[no_mangle]
pub extern "C" fn hash_xoroshiroish(seed: u64) -> u64 {
    let s0 = splitmix64(seed);
    let mut s1 = splitmix64(seed ^ 0xa5a5_a5a5_5a5a_5a5a);
    s1 ^= s0;
    let out = s0.wrapping_mul(5).rotate_left(7).wrapping_mul(9);
    avalanche(out ^ s1.rotate_left(13))
}
