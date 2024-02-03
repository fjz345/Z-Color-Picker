pub fn factorial(n: u64) -> u64 {
    (1..=n).product()
}

// n! / (n - r)!

pub fn combination(n: u64, r: u64) -> u64 {
    factorial(n) / factorial(n - r)
}

pub fn count_combinations(n: u64, r: u64) -> u64 {
    if r > n {
        0
    } else {
        (1..=r.min(n - r)).fold(1, |acc, val| acc * (n - val + 1) / val)
    }
}

pub fn count_permutations(n: u64, r: u64) -> u64 {
    (n - r + 1..=n).product()
}

pub fn mul_array<const D: usize, T: std::ops::MulAssign + std::marker::Copy>(
    mut lhs: [T; D],
    rhs: T,
) -> [T; D] {
    for i in 0..D {
        lhs[i] *= rhs;
    }
    lhs
}

pub fn add_array<const D: usize, T: std::ops::AddAssign + std::marker::Copy>(
    mut lhs: [T; D],
    rhs: T,
) -> [T; D] {
    for i in 0..D {
        lhs[i] += rhs;
    }
    lhs
}

pub fn add_array_array<const D: usize, T: std::ops::AddAssign + std::marker::Copy>(
    mut lhs: [T; D],
    rhs: [T; D],
) -> [T; D] {
    for i in 0..D {
        lhs[i] += rhs[i];
    }
    lhs
}
