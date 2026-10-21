//! Numbers exactly as EasyGCode's JavaScript computes and prints them.
//!
//! The golden tests require G-code identical to EasyGCode's, so the few places where JavaScript
//! and Rust differ are reproduced here: `toFixed` rounding, `Math.round`, `Math.hypot` (V8 sums
//! the squares with Kahan compensation after scaling by the largest value, so it can differ from
//! `sqrt(x² + y²)` in the last bit), and `Math.sin`, `Math.cos` and `Math.pow`, which V8
//! computes with its own copy of fdlibm. Ported here, they give the same bits natively and in
//! WebAssembly as EasyGCode gets under Node.js, where the golden fixtures come from. (A browser's
//! JavaScript engine may use another sine, so EasyGCode itself can differ in the last bit from
//! one browser to the next; this engine gives the same G-code everywhere.)

/// Numbers as JavaScript's `Number.prototype.toFixed` writes them: rounded half away from zero
/// on the exact value (Rust rounds exact ties to even), and never "-0.000" for negative zero.
pub fn fixed(v: f64, digits: usize) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let x = v.abs();
    let scaled = x * 10f64.powi(digits as i32);
    let mut out = if (scaled - scaled.floor() - 0.5).abs() > 1e-6 {
        format!("{x:.digits$}")
    } else {
        // Possibly an exact tie: decide on the exact decimal expansion.
        let exact = format!("{x:.80}");
        let (int, frac) = exact.split_once('.').unwrap_or((exact.as_str(), ""));
        let keep = frac.get(..digits).unwrap_or(frac);
        let round_up = frac.as_bytes().get(digits).is_some_and(|d| *d >= b'5');
        let mut digits_str: Vec<u8> = format!("{int}{keep}").into_bytes();
        if round_up {
            let mut i = digits_str.len();
            loop {
                if i == 0 {
                    digits_str.insert(0, b'1');
                    break;
                }
                i -= 1;
                match digits_str.get_mut(i) {
                    Some(d) if *d == b'9' => *d = b'0',
                    Some(d) => {
                        *d += 1;
                        break;
                    }
                    None => break,
                }
            }
        }
        let s = String::from_utf8(digits_str).unwrap_or_default();
        let split = s.len().saturating_sub(digits);
        match (digits, s.get(..split), s.get(split..)) {
            (0, _, _) => s,
            (_, Some(a), Some(b)) => format!("{a}.{b}"),
            _ => format!("{x:.digits$}"),
        }
    };
    // JavaScript keeps the sign of any negative number, even one that rounds to zero
    // ((-0.0001).toFixed(3) is "-0.000"), but not of negative zero.
    if v < 0.0 {
        out.insert(0, '-');
    }
    out
}

/// `toFixed(3)`: coordinates.
pub fn f3(v: f64) -> String {
    fixed(v, 3)
}

/// `toFixed(5)`: extrusion amounts.
pub fn f5(v: f64) -> String {
    fixed(v, 5)
}

/// JavaScript's `+v.toFixed(digits)`: rounded to `digits` decimals and read back.
pub fn round_to(v: f64, digits: usize) -> f64 {
    fixed(v, digits).parse().unwrap_or(v)
}

/// JavaScript's `Math.round` (half up).
pub fn round(v: f64) -> i64 {
    (v + 0.5).floor() as i64
}

/// A number as JavaScript prints it in a string (`'S' + 210` is "S210", 0.3 is "0.3").
pub fn js(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// V8's `Math.hypot`: scaled by the largest magnitude, squares summed with Kahan compensation.
pub fn hypot(values: &[f64]) -> f64 {
    let mut max = 0.0f64;
    let mut nan = false;
    for v in values {
        if v.is_nan() {
            nan = true;
        } else if v.abs() > max {
            max = v.abs();
        }
    }
    if max == f64::INFINITY {
        return f64::INFINITY;
    }
    if nan {
        return f64::NAN;
    }
    if max == 0.0 {
        return 0.0;
    }
    let mut sum = 0.0;
    let mut compensation = 0.0;
    for v in values {
        let n = v.abs() / max;
        let summand = n * n - compensation;
        let preliminary = sum + summand;
        compensation = (preliminary - sum) - summand;
        sum = preliminary;
    }
    sum.sqrt() * max
}

/// `Math.hypot(x, y)`.
pub fn hypot2(x: f64, y: f64) -> f64 {
    hypot(&[x, y])
}

/// `Math.hypot(x, y, z)`.
pub fn hypot3(x: f64, y: f64, z: f64) -> f64 {
    hypot(&[x, y, z])
}

pub use fdlibm::{cos, pow, sin};

/// fdlibm's sine, cosine and power, as V8 has them (`src/base/ieee754.cc`), written as in the
/// original, constants included, so it can be checked line by line against it.
///
/// Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved. Developed at SunSoft, a Sun
/// Microsystems, Inc. business. Permission to use, copy, modify, and distribute this software is
/// freely granted, provided that this notice is preserved. Modified by Google Inc.: Copyright 2016
/// the V8 project authors (BSD-3-Clause). See THIRD-PARTY-NOTICES.md.
#[allow(
    clippy::excessive_precision,
    clippy::approx_constant,
    clippy::eq_op,
    clippy::unreadable_literal,
    clippy::many_single_char_names
)]
mod fdlibm {
    /// `Math.cos` as V8 computes it (fdlibm).
    pub fn cos(x: f64) -> f64 {
        let ix = high_word(x) & 0x7FFF_FFFF;
        if ix <= 0x3FE9_21FB {
            return kernel_cos(x, 0.0);
        }
        if ix >= 0x7FF0_0000 {
            return x - x; // cos(Inf or NaN) is NaN
        }
        match rem_pio2(x) {
            Some((n, y0, y1)) => match n & 3 {
                0 => kernel_cos(y0, y1),
                1 => -kernel_sin(y0, y1, true),
                2 => -kernel_cos(y0, y1),
                _ => kernel_sin(y0, y1, true),
            },
            None => libm::cos(x),
        }
    }

    /// `Math.sin` as V8 computes it (fdlibm).
    pub fn sin(x: f64) -> f64 {
        let ix = high_word(x) & 0x7FFF_FFFF;
        if ix <= 0x3FE9_21FB {
            return kernel_sin(x, 0.0, false);
        }
        if ix >= 0x7FF0_0000 {
            return x - x; // sin(Inf or NaN) is NaN
        }
        match rem_pio2(x) {
            Some((n, y0, y1)) => match n & 3 {
                0 => kernel_sin(y0, y1, true),
                1 => kernel_cos(y0, y1),
                2 => -kernel_sin(y0, y1, true),
                _ => -kernel_cos(y0, y1),
            },
            None => libm::sin(x),
        }
    }

    // musl's (the `libm` crate) and glibc's differ from these in the last bit now and then,
    // which is enough to flip a tie when EasyGCode simplifies a symmetric outline.

    fn high_word(x: f64) -> i32 {
        (x.to_bits() >> 32) as u32 as i32
    }

    fn from_words(hi: u32, lo: u32) -> f64 {
        f64::from_bits((u64::from(hi) << 32) | u64::from(lo))
    }

    /// `x` reduced by a multiple `n` of π/2 to `y0 + y1` in [−π/4, π/4]: `(n, y0, y1)`. `None` past
    /// 2¹⁹·π/2, where fdlibm needs its long reduction (never reached by a real print; `libm`'s then).
    fn rem_pio2(x: f64) -> Option<(i32, f64, f64)> {
        const INVPIO2: f64 = 6.366_197_723_675_813_824_33e-01;
        const PIO2_1: f64 = 1.570_796_326_734_125_614_17e+00;
        const PIO2_1T: f64 = 6.077_100_506_506_192_249_32e-11;
        const PIO2_2: f64 = 6.077_100_506_303_965_976_60e-11;
        const PIO2_2T: f64 = 2.022_266_248_795_950_631_54e-21;
        const PIO2_3: f64 = 2.022_266_248_711_166_455_80e-21;
        const PIO2_3T: f64 = 8.478_427_660_368_899_569_97e-32;
        const NPIO2_HW: [i32; 32] = [
            0x3FF921FB, 0x400921FB, 0x4012D97C, 0x401921FB, 0x401F6A7A, 0x4022D97C, 0x4025FDBB,
            0x402921FB, 0x402C463A, 0x402F6A7A, 0x4031475C, 0x4032D97C, 0x40346B9C, 0x4035FDBB,
            0x40378FDB, 0x403921FB, 0x403AB41B, 0x403C463A, 0x403DD85A, 0x403F6A7A, 0x40407E4C,
            0x4041475C, 0x4042106C, 0x4042D97C, 0x4043A28C, 0x40446B9C, 0x404534AC, 0x4045FDBB,
            0x4046C6CB, 0x40478FDB, 0x404858EB, 0x404921FB,
        ];
        let hx = high_word(x);
        let ix = hx & 0x7FFF_FFFF;
        if ix <= 0x3FE9_21FB {
            return Some((0, x, 0.0));
        }
        if ix < 0x4002_D97C {
            // |x| < 3π/4: n = ±1.
            return Some(if hx > 0 {
                let mut z = x - PIO2_1;
                if ix != 0x3FF9_21FB {
                    let y0 = z - PIO2_1T;
                    (1, y0, (z - y0) - PIO2_1T)
                } else {
                    z -= PIO2_2;
                    let y0 = z - PIO2_2T;
                    (1, y0, (z - y0) - PIO2_2T)
                }
            } else {
                let mut z = x + PIO2_1;
                if ix != 0x3FF9_21FB {
                    let y0 = z + PIO2_1T;
                    (-1, y0, (z - y0) + PIO2_1T)
                } else {
                    z += PIO2_2;
                    let y0 = z + PIO2_2T;
                    (-1, y0, (z - y0) + PIO2_2T)
                }
            });
        }
        if ix > 0x4139_21FB {
            return None;
        }
        let t = x.abs();
        let n = (t * INVPIO2 + 0.5) as i32;
        let f = f64::from(n);
        let mut r = t - f * PIO2_1;
        let mut w = f * PIO2_1T; // 1st round good to 85 bits
        let y0;
        let quick =
            (1..32).contains(&n) && NPIO2_HW.get((n - 1) as usize).is_some_and(|hw| ix != *hw);
        if quick {
            y0 = r - w; // no cancellation
        } else {
            let j = ix >> 20;
            let mut y = r - w;
            let i = j - ((high_word(y) >> 20) & 0x7FF);
            if i > 16 {
                // 2nd iteration, good to 118 bits
                let t = r;
                w = f * PIO2_2;
                r = t - w;
                w = f * PIO2_2T - ((t - r) - w);
                y = r - w;
                let i = j - ((high_word(y) >> 20) & 0x7FF);
                if i > 49 {
                    // 3rd iteration, 151 bits: covers every case
                    let t = r;
                    w = f * PIO2_3;
                    r = t - w;
                    w = f * PIO2_3T - ((t - r) - w);
                    y = r - w;
                }
            }
            y0 = y;
        }
        let y1 = (r - y0) - w;
        Some(if hx < 0 { (-n, -y0, -y1) } else { (n, y0, y1) })
    }

    /// Sine on [−π/4, π/4]; `y` is the tail of `x`, used when `tail`.
    fn kernel_sin(x: f64, y: f64, tail: bool) -> f64 {
        const S1: f64 = -1.666_666_666_666_663_243_48e-01;
        const S2: f64 = 8.333_333_333_322_489_461_24e-03;
        const S3: f64 = -1.984_126_982_985_794_931_34e-04;
        const S4: f64 = 2.755_731_370_707_006_767_89e-06;
        const S5: f64 = -2.505_076_025_340_686_341_95e-08;
        const S6: f64 = 1.589_690_995_211_550_102_21e-10;
        let ix = high_word(x) & 0x7FFF_FFFF;
        if ix < 0x3E40_0000 && x as i32 == 0 {
            return x;
        }
        let z = x * x;
        let v = z * x;
        let r = S2 + z * (S3 + z * (S4 + z * (S5 + z * S6)));
        if !tail {
            x + v * (S1 + z * r)
        } else {
            x - ((z * (0.5 * y - v * r) - y) - v * S1)
        }
    }

    /// Cosine on [−π/4, π/4]; `y` is the tail of `x`.
    fn kernel_cos(x: f64, y: f64) -> f64 {
        const C1: f64 = 4.166_666_666_666_660_190_37e-02;
        const C2: f64 = -1.388_888_888_887_410_957_49e-03;
        const C3: f64 = 2.480_158_728_947_672_941_78e-05;
        const C4: f64 = -2.755_731_435_139_066_330_35e-07;
        const C5: f64 = 2.087_572_321_298_174_827_90e-09;
        const C6: f64 = -1.135_964_755_778_819_482_65e-11;
        let ix = high_word(x) & 0x7FFF_FFFF;
        if ix < 0x3E40_0000 && x as i32 == 0 {
            return 1.0;
        }
        let z = x * x;
        let r = z * (C1 + z * (C2 + z * (C3 + z * (C4 + z * (C5 + z * C6)))));
        if ix < 0x3FD3_3333 {
            // |x| < 0.3
            1.0 - (0.5 * z - (z * r - x * y))
        } else {
            let qx = if ix > 0x3FE9_0000 {
                0.28125
            } else {
                from_words((ix - 0x0020_0000) as u32, 0) // x/4
            };
            let iz = 0.5 * z - qx;
            let a = 1.0 - qx;
            a - (iz - (z * r - x * y))
        }
    }

    /// `Math.pow` as V8 computes it (fdlibm's `e_pow.c`; the `libm` crate's differs in the last bit
    /// now and then).
    pub fn pow(x: f64, y: f64) -> f64 {
        const BP: [f64; 2] = [1.0, 1.5];
        const DP_H: [f64; 2] = [0.0, 5.849_624_872_207_641_601_56e-01];
        const DP_L: [f64; 2] = [0.0, 1.350_039_202_129_748_971_28e-08];
        const TWO53: f64 = 9_007_199_254_740_992.0;
        const HUGE: f64 = 1.0e300;
        const TINY: f64 = 1.0e-300;
        const L1: f64 = 5.999_999_999_999_946_487_25e-01;
        const L2: f64 = 4.285_714_285_785_501_842_52e-01;
        const L3: f64 = 3.333_333_298_183_774_329_18e-01;
        const L4: f64 = 2.727_281_238_085_340_064_89e-01;
        const L5: f64 = 2.306_607_457_755_617_540_67e-01;
        const L6: f64 = 2.069_750_178_003_384_177_84e-01;
        const P1: f64 = 1.666_666_666_666_660_190_37e-01;
        const P2: f64 = -2.777_777_777_701_559_338_42e-03;
        const P3: f64 = 6.613_756_321_437_934_361_17e-05;
        const P4: f64 = -1.653_390_220_546_525_153_90e-06;
        const P5: f64 = 4.138_136_797_057_238_460_39e-08;
        const LG2: f64 = 6.931_471_805_599_452_862_27e-01;
        const LG2_H: f64 = 6.931_471_824_645_996_093_75e-01;
        const LG2_L: f64 = -1.904_654_299_957_768_045_25e-09;
        const OVT: f64 = 8.008_566_259_537_294_437_2e-17;
        const CP: f64 = 9.617_966_939_259_755_543_29e-01;
        const CP_H: f64 = 9.617_967_009_544_372_558_59e-01;
        const CP_L: f64 = -7.028_461_650_952_758_265_16e-09;
        const IVLN2: f64 = 1.442_695_040_888_963_387_00e+00;
        const IVLN2_H: f64 = 1.442_695_021_629_333_496_09e+00;
        const IVLN2_L: f64 = 1.925_962_991_126_617_468_87e-08;

        let low = |v: f64| v.to_bits() as u32;
        let zero_low = |v: f64| f64::from_bits(v.to_bits() & 0xFFFF_FFFF_0000_0000);
        let set_high = |v: f64, hi: i32| {
            f64::from_bits((v.to_bits() & 0x0000_0000_FFFF_FFFF) | (u64::from(hi as u32) << 32))
        };

        let (hx, lx) = (high_word(x), low(x));
        let (hy, ly) = (high_word(y), low(y));
        let mut ix = hx & 0x7fff_ffff;
        let iy = hy & 0x7fff_ffff;

        if (iy as u32 | ly) == 0 {
            return 1.0; // x**0 = 1
        }
        if ix > 0x7ff0_0000
            || (ix == 0x7ff0_0000 && lx != 0)
            || iy > 0x7ff0_0000
            || (iy == 0x7ff0_0000 && ly != 0)
        {
            return x + y; // NaN
        }
        // Whether y is an odd (1) or even (2) integer, when x < 0.
        let mut yisint = 0;
        if hx < 0 {
            if iy >= 0x4340_0000 {
                yisint = 2;
            } else if iy >= 0x3ff0_0000 {
                let k = (iy >> 20) - 0x3ff;
                if k > 20 {
                    let j = ly >> (52 - k);
                    if (j << (52 - k)) == ly {
                        yisint = 2 - (j & 1) as i32;
                    }
                } else if ly == 0 {
                    let j = iy >> (20 - k);
                    if (j << (20 - k)) == iy {
                        yisint = 2 - (j & 1);
                    }
                }
            }
        }
        if ly == 0 {
            if iy == 0x7ff0_0000 {
                // y is ±inf
                return if ((ix - 0x3ff0_0000) as u32 | lx) == 0 {
                    y - y
                } else if ix >= 0x3ff0_0000 {
                    if hy >= 0 { y } else { 0.0 }
                } else if hy < 0 {
                    -y
                } else {
                    0.0
                };
            }
            if iy == 0x3ff0_0000 {
                return if hy < 0 { 1.0 / x } else { x };
            }
            if hy == 0x4000_0000 {
                return x * x;
            }
            if hy == 0x3fe0_0000 && hx >= 0 {
                return x.sqrt();
            }
        }
        let mut ax = x.abs();
        if lx == 0 && (ix == 0x7ff0_0000 || ix == 0 || ix == 0x3ff0_0000) {
            // x is ±0, ±inf, ±1
            let mut z = ax;
            if hy < 0 {
                z = 1.0 / z;
            }
            if hx < 0 {
                if ((ix - 0x3ff0_0000) | yisint) == 0 {
                    z = f64::NAN; // (−1)**non-int
                } else if yisint == 1 {
                    z = -z;
                }
            }
            return z;
        }
        let mut n: i32 = (hx >> 31) + 1;
        if (n | yisint) == 0 {
            return f64::NAN; // (x<0)**(non-int)
        }
        let mut s = 1.0;
        if (n | (yisint - 1)) == 0 {
            s = -1.0; // (−ve)**(odd int)
        }
        let (t1, t2);
        if iy > 0x41e0_0000 {
            // |y| > 2**31
            if iy > 0x43f0_0000 {
                if ix <= 0x3fef_ffff {
                    return if hy < 0 { HUGE * HUGE } else { TINY * TINY };
                }
                if ix >= 0x3ff0_0000 {
                    return if hy > 0 { HUGE * HUGE } else { TINY * TINY };
                }
            }
            if ix < 0x3fef_ffff {
                return if hy < 0 {
                    s * HUGE * HUGE
                } else {
                    s * TINY * TINY
                };
            }
            if ix > 0x3ff0_0000 {
                return if hy > 0 {
                    s * HUGE * HUGE
                } else {
                    s * TINY * TINY
                };
            }
            let t = ax - 1.0;
            let w = (t * t) * (0.5 - t * (0.333_333_333_333_333_333_333_3 - t * 0.25));
            let u = IVLN2_H * t;
            let v = t * IVLN2_L - w * IVLN2;
            t1 = zero_low(u + v);
            t2 = v - (t1 - u);
        } else {
            n = 0;
            if ix < 0x0010_0000 {
                ax *= TWO53;
                n -= 53;
                ix = high_word(ax);
            }
            n += (ix >> 20) - 0x3ff;
            let j = ix & 0x000f_ffff;
            ix = j | 0x3ff0_0000;
            let k: usize = if j <= 0x3988E {
                0
            } else if j < 0xBB67A {
                1
            } else {
                n += 1;
                ix -= 0x0010_0000;
                0
            };
            ax = set_high(ax, ix);
            let u = ax - BP[k];
            let v = 1.0 / (ax + BP[k]);
            let ss = u * v;
            let s_h = zero_low(ss);
            let t_h = set_high(
                0.0,
                ((ix >> 1) | 0x2000_0000) + 0x0008_0000 + ((k as i32) << 18),
            );
            let t_l = ax - (t_h - BP[k]);
            let s_l = v * ((u - s_h * t_h) - s_h * t_l);
            let mut s2 = ss * ss;
            let mut r = s2 * s2 * (L1 + s2 * (L2 + s2 * (L3 + s2 * (L4 + s2 * (L5 + s2 * L6)))));
            r += s_l * (s_h + ss);
            s2 = s_h * s_h;
            let t_h = zero_low(3.0 + s2 + r);
            let t_l = r - ((t_h - 3.0) - s2);
            let u = s_h * t_h;
            let v = s_l * t_h + t_l * ss;
            let p_h = zero_low(u + v);
            let p_l = v - (p_h - u);
            let z_h = CP_H * p_h;
            let z_l = CP_L * p_h + p_l * CP + DP_L[k];
            let t = f64::from(n);
            t1 = zero_low(((z_h + z_l) + DP_H[k]) + t);
            t2 = z_l - (((t1 - t) - DP_H[k]) - z_h);
        }
        // (y1 + y2)·(t1 + t2)
        let y1 = zero_low(y);
        let p_l = (y - y1) * t1 + y * t2;
        let mut p_h = y1 * t1;
        let z = p_l + p_h;
        let j = high_word(z);
        let i = low(z);
        if j >= 0x4090_0000 {
            if ((j - 0x4090_0000) as u32 | i) != 0 || p_l + OVT > z - p_h {
                return s * HUGE * HUGE; // overflow
            }
        } else if (j & 0x7fff_ffff) >= 0x4090_cc00
            && (((j as u32).wrapping_sub(0xc090_cc00) | i) != 0 || p_l <= z - p_h)
        {
            return s * TINY * TINY; // underflow
        }
        // 2**(p_h + p_l)
        let i = j & 0x7fff_ffff;
        let mut k = (i >> 20) - 0x3ff;
        let mut n = 0;
        if i > 0x3fe0_0000 {
            n = j + (0x0010_0000 >> (k + 1));
            k = ((n & 0x7fff_ffff) >> 20) - 0x3ff;
            let t = set_high(0.0, n & !(0x000f_ffff >> k));
            n = ((n & 0x000f_ffff) | 0x0010_0000) >> (20 - k);
            if j < 0 {
                n = -n;
            }
            p_h -= t;
        }
        let t = zero_low(p_l + p_h);
        let u = t * LG2_H;
        let v = (p_l - (t - p_h)) * LG2 + t * LG2_L;
        let mut z = u + v;
        let w = v - (z - u);
        let t = z * z;
        let t1 = z - t * (P1 + t * (P2 + t * (P3 + t * (P4 + t * P5))));
        let r = (z * t1) / ((t1 - 2.0) - (w + z * w));
        z = 1.0 - (r - z);
        let j = high_word(z).wrapping_add(((n as u32) << 20) as i32);
        if (j >> 20) <= 0 {
            z = libm::scalbn(z, n); // subnormal output
        } else {
            z = set_high(z, high_word(z).wrapping_add(((n as u32) << 20) as i32));
        }
        s * z
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_round_like_javascript_to_fixed() {
        // Expected strings from Node.js (`v.toFixed(3)`).
        let cases = [
            (125.0, "125.000"),
            (0.0625, "0.063"), // an exact tie: JavaScript rounds up, Rust's {:.3} rounds to even
            (-0.0625, "-0.063"),
            (1.0005, "1.000"), // really 1.000499999…
            (0.9995, "1.000"), // really 0.999500000…055: up, with a carry
            (9.9995, "9.999"),
            (185.8235, "185.823"),
            (1.4995, "1.500"),
            (-0.0001, "-0.000"),
            (-0.0, "0.000"),
        ];
        for (v, want) in cases {
            assert_eq!(fixed(v, 3), want, "{v}");
        }
        assert_eq!(fixed(2.675, 2), "2.67");
        assert_eq!(fixed(2.5, 0), "3");
        assert_eq!(fixed(0.00668123, 5), "0.00668");
        assert_eq!(round_to(0.1234567894, 9), 0.123456789);
    }

    #[test]
    fn hypot_matches_v8() {
        // Expected values from Node.js 22 (`Math.hypot(...)`), printed with 17 significant digits.
        let cases: [(&[f64], f64); 4] = [
            (&[3.0, 4.0], 5.0),
            (&[1.0, 1.0, 1.0], 1.7320508075688772),
            (&[0.1, 0.2, 0.3], 0.37416573867739417),
            (&[-1.4375, 0.0688, 0.005], 1.4391541578302165),
        ];
        for (args, want) in cases {
            assert_eq!(hypot(args), want, "{args:?}");
        }
        assert_eq!(hypot(&[0.0, 0.0]), 0.0);
        assert!(hypot(&[f64::NAN, 1.0]).is_nan());
        assert_eq!(hypot(&[f64::NAN, f64::INFINITY]), f64::INFINITY);
    }

    #[test]
    fn trigonometry_and_powers_match_v8_to_the_bit() {
        // Inputs where the `libm` crate (musl) and V8 (fdlibm) disagree in the last bit; expected
        // values from Node.js 22. 23,220 angles and 22,000 powers were checked the same way.
        assert_eq!(sin(8.377580409572781e-1), 7.431448254773942e-1);
        assert_eq!(sin(1.4748032179352084), 9.953961983671789e-1);
        assert_eq!(sin(2.2776546738526), 7.60405965600031e-1);
        assert_eq!(cos(6.370451769779303e-1), 8.038568606172174e-1);
        assert_eq!(cos(7.592182246175333e-1), 7.253743710122877e-1);
        assert_eq!(cos(2.4347343065320897), -7.604059656000308e-1);
        assert_eq!(
            pow(2.9049224853515626e-4, 4.329795837402344e1),
            7.260171737997079e-154
        );
        assert_eq!(
            pow(1.892460107803345e-2, -2.6427626609802246e-1),
            2.8532573287370444
        );
        assert_eq!(
            pow(6.628447294235229e-2, -1.1142838001251221e-1),
            1.3530946379054434
        );
        // Special cases.
        assert_eq!(sin(0.0), 0.0);
        assert_eq!(cos(0.0), 1.0);
        assert!(sin(f64::INFINITY).is_nan());
        assert_eq!(sin(1e300), libm::sin(1e300)); // past fdlibm's medium reduction
        assert_eq!(pow(2.0, 10.0), 1024.0);
        assert_eq!(pow(4.0, 0.5), 2.0);
        assert!(pow(-8.0, 1.0 / 3.0).is_nan());
        assert_eq!(pow(-2.0, 3.0), -8.0);
        assert_eq!(pow(0.0, -1.0), f64::INFINITY);
        assert_eq!(pow(2.0, -1080.0), 0.0);
        assert_eq!(pow(2.0, 1024.0), f64::INFINITY);
    }

    #[test]
    fn javascript_number_strings() {
        assert_eq!(js(210.0), "210");
        assert_eq!(js(0.3), "0.3");
        assert_eq!(js(-2.5), "-2.5");
        assert_eq!(round(2.5), 3);
        assert_eq!(round(-2.5), -2);
    }
}
