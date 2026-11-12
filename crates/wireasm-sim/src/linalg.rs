//! Dense linear algebra for the circuit solver.
//!
//! Circuits in WireASM are small to medium (tens to a few thousand nodes), so a
//! dense LU with partial pivoting is both simple and fast enough. The same
//! code is duplicated for real and complex numbers to keep the hot loops
//! monomorphic.

use std::ops::{Add, Div, Mul, Neg, Sub};

/// Complex number with `f64` parts.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct C {
    pub re: f64,
    pub im: f64,
}

impl C {
    pub const ZERO: C = C { re: 0.0, im: 0.0 };
    pub const ONE: C = C { re: 1.0, im: 0.0 };

    pub fn new(re: f64, im: f64) -> C {
        C { re, im }
    }

    pub fn from_polar(mag: f64, angle: f64) -> C {
        C {
            re: mag * angle.cos(),
            im: mag * angle.sin(),
        }
    }

    pub fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }

    pub fn arg(self) -> f64 {
        self.im.atan2(self.re)
    }

    pub fn conj(self) -> C {
        C {
            re: self.re,
            im: -self.im,
        }
    }

    /// `j * omega * value`, used for capacitors and inductors.
    pub fn jw(omega: f64) -> C {
        C { re: 0.0, im: omega }
    }
}

impl Add for C {
    type Output = C;
    fn add(self, rhs: C) -> C {
        C::new(self.re + rhs.re, self.im + rhs.im)
    }
}

impl Sub for C {
    type Output = C;
    fn sub(self, rhs: C) -> C {
        C::new(self.re - rhs.re, self.im - rhs.im)
    }
}

impl Neg for C {
    type Output = C;
    fn neg(self) -> C {
        C::new(-self.re, -self.im)
    }
}

impl Mul for C {
    type Output = C;
    fn mul(self, rhs: C) -> C {
        C::new(
            self.re * rhs.re - self.im * rhs.im,
            self.re * rhs.im + self.im * rhs.re,
        )
    }
}

impl Div for C {
    type Output = C;
    fn div(self, rhs: C) -> C {
        let d = rhs.re * rhs.re + rhs.im * rhs.im;
        C::new(
            (self.re * rhs.re + self.im * rhs.im) / d,
            (self.im * rhs.re - self.re * rhs.im) / d,
        )
    }
}

impl Mul<f64> for C {
    type Output = C;
    fn mul(self, rhs: f64) -> C {
        C::new(self.re * rhs, self.im * rhs)
    }
}

/// Square real matrix with LU solve.
#[derive(Clone, Debug)]
pub struct Matrix {
    n: usize,
    a: Vec<f64>,
}

impl Matrix {
    pub fn zeros(n: usize) -> Matrix {
        Matrix {
            n,
            a: vec![0.0; n * n],
        }
    }

    pub fn size(&self) -> usize {
        self.n
    }

    #[inline]
    pub fn get(&self, i: usize, j: usize) -> f64 {
        self.a[i * self.n + j]
    }

    #[inline]
    pub fn set(&mut self, i: usize, j: usize, v: f64) {
        self.a[i * self.n + j] = v;
    }

    #[inline]
    pub fn add(&mut self, i: usize, j: usize, v: f64) {
        self.a[i * self.n + j] += v;
    }

    /// Solve `A x = b` in place. The matrix is consumed.
    pub fn solve(mut self, mut b: Vec<f64>) -> Result<Vec<f64>, String> {
        let n = self.n;
        debug_assert_eq!(b.len(), n);
        for k in 0..n {
            let mut pivot = k;
            let mut best = self.get(k, k).abs();
            for i in (k + 1)..n {
                let v = self.get(i, k).abs();
                if v > best {
                    best = v;
                    pivot = i;
                }
            }
            if best < 1e-18 {
                return Err(format!("singular matrix at row {k}"));
            }
            if pivot != k {
                for j in 0..n {
                    let tmp = self.get(k, j);
                    let other = self.get(pivot, j);
                    self.set(k, j, other);
                    self.set(pivot, j, tmp);
                }
                b.swap(k, pivot);
            }
            let d = self.get(k, k);
            for i in (k + 1)..n {
                let f = self.get(i, k) / d;
                if f == 0.0 {
                    continue;
                }
                self.set(i, k, 0.0);
                for j in (k + 1)..n {
                    let v = self.get(i, j) - f * self.get(k, j);
                    self.set(i, j, v);
                }
                b[i] -= f * b[k];
            }
        }
        let mut x = vec![0.0; n];
        for i in (0..n).rev() {
            let mut s = b[i];
            for (j, xj) in x.iter().enumerate().skip(i + 1) {
                s -= self.get(i, j) * xj;
            }
            x[i] = s / self.get(i, i);
        }
        Ok(x)
    }
}

/// Square complex matrix with LU solve.
#[derive(Clone, Debug)]
pub struct CMatrix {
    n: usize,
    a: Vec<C>,
}

impl CMatrix {
    pub fn zeros(n: usize) -> CMatrix {
        CMatrix {
            n,
            a: vec![C::ZERO; n * n],
        }
    }

    #[inline]
    pub fn get(&self, i: usize, j: usize) -> C {
        self.a[i * self.n + j]
    }

    #[inline]
    pub fn set(&mut self, i: usize, j: usize, v: C) {
        self.a[i * self.n + j] = v;
    }

    #[inline]
    pub fn add(&mut self, i: usize, j: usize, v: C) {
        self.a[i * self.n + j] = self.a[i * self.n + j] + v;
    }

    /// Solve `A x = b` in place. The matrix is consumed.
    pub fn solve(mut self, mut b: Vec<C>) -> Result<Vec<C>, String> {
        let n = self.n;
        for k in 0..n {
            let mut pivot = k;
            let mut best = self.get(k, k).abs();
            for i in (k + 1)..n {
                let v = self.get(i, k).abs();
                if v > best {
                    best = v;
                    pivot = i;
                }
            }
            if best < 1e-18 {
                return Err(format!("singular complex matrix at row {k}"));
            }
            if pivot != k {
                for j in 0..n {
                    let tmp = self.get(k, j);
                    let other = self.get(pivot, j);
                    self.set(k, j, other);
                    self.set(pivot, j, tmp);
                }
                b.swap(k, pivot);
            }
            let d = self.get(k, k);
            for i in (k + 1)..n {
                let f = self.get(i, k) / d;
                if f.abs() == 0.0 {
                    continue;
                }
                self.set(i, k, C::ZERO);
                for j in (k + 1)..n {
                    let v = self.get(i, j) - f * self.get(k, j);
                    self.set(i, j, v);
                }
                b[i] = b[i] - f * b[k];
            }
        }
        let mut x = vec![C::ZERO; n];
        for i in (0..n).rev() {
            let mut s = b[i];
            for (j, xj) in x.iter().enumerate().skip(i + 1) {
                s = s - self.get(i, j) * *xj;
            }
            x[i] = s / self.get(i, i);
        }
        Ok(x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_solve() {
        // 2x + y = 5, x + 3y = 10 -> x = 1, y = 3
        let mut m = Matrix::zeros(2);
        m.set(0, 0, 2.0);
        m.set(0, 1, 1.0);
        m.set(1, 0, 1.0);
        m.set(1, 1, 3.0);
        let x = m.solve(vec![5.0, 10.0]).unwrap();
        assert!((x[0] - 1.0).abs() < 1e-12);
        assert!((x[1] - 3.0).abs() < 1e-12);
    }

    #[test]
    fn complex_solve() {
        // (1+j) x = 2 -> x = 1 - j
        let mut m = CMatrix::zeros(1);
        m.set(0, 0, C::new(1.0, 1.0));
        let x = m.solve(vec![C::new(2.0, 0.0)]).unwrap();
        assert!((x[0].re - 1.0).abs() < 1e-12);
        assert!((x[0].im + 1.0).abs() < 1e-12);
    }
}
