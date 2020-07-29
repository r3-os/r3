//! Compute-intensive code.
//!
//! This is designed to be register-hungry to increase the likelihood of
//! discovering a bug in context switching.
use constance::utils::Init;
use core::ops;

use super::trig::sincos;

/// The number of signal streams. The large it is, the more registers the code
/// will use.
const CHANNELS: usize = 16;

macro_rules! for_each_channel {
    ([|$i:ident| $x:expr]) => {{
        let mut array = [Init::INIT; CHANNELS];
        for_each_channel!(|$i| {
            array[$i] = $x;
        });
        array
    }};
    (|$i:ident| $x:expr) => {{
        macro_rules! inner {
            ($i2:expr) => {{
                let $i = $i2;
                $x
            }};
        }
        for_each_channel!(inner!);
    }};
    ($m:ident!) => {{
        $m!(0);
        $m!(1);
        $m!(2);
        $m!(3);
        $m!(4);
        $m!(5);
        $m!(6);
        $m!(7);
        $m!(8);
        $m!(9);
        $m!(10);
        $m!(11);
        $m!(12);
        $m!(13);
        $m!(14);
        $m!(15);
    }};
}

/// The number of signal samples.
const LEN: usize = 16;

/// The working space for the computation kernel.
pub struct KernelState {
    filter_states1: [BiquadKernelState<f32>; CHANNELS],
    filter_states2: [BiquadKernelState<f32>; CHANNELS],
}

/// The output of the computation kernel.
#[derive(Debug, PartialEq)]
pub struct KernelOutput {
    samples: [[f32; CHANNELS]; LEN],
}

impl Init for KernelState {
    const INIT: Self = Self {
        filter_states1: Init::INIT,
        filter_states2: Init::INIT,
    };
}

impl Init for KernelOutput {
    const INIT: Self = Self {
        samples: Init::INIT,
    };
}

impl KernelState {
    /// Perform computation. After the function call, the contents of `out` is
    /// guaranteed to be identical for every run.
    #[inline(never)]
    pub fn run(&mut self, out: &mut KernelOutput) {
        let filter_states1 = &mut self.filter_states1;
        let filter_states2 = &mut self.filter_states2;

        // Reset the filter state
        *filter_states1 = Init::INIT;
        *filter_states2 = Init::INIT;

        // The input signal
        const PRIMES: [u32; CHANNELS] =
            [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53];
        const AMP: [f32; CHANNELS] = for_each_channel!([|i| 1.0 / PRIMES[i] as f32]);

        let mut rng = Xorshift32(0xc0ffee);
        for sample in out.samples.iter_mut() {
            let rand = rng.next();
            for (i, (value, &amp)) in sample.iter_mut().zip(AMP.iter()).enumerate() {
                *value = if (rand & (1 << i)) != 0 { amp } else { -amp };
            }
        }

        // Process the signal
        const FILTER1: [BiquadCoefs<f32>; CHANNELS] =
            for_each_channel!([
                |i| low_pass_filter(i as f64 / CHANNELS as f64 * 0.04 + 0.45, 0.5).to_f32()
            ]);

        const FILTER2: [BiquadCoefs<f32>; CHANNELS] =
            for_each_channel!([
                |i| low_pass_filter(i as f64 / CHANNELS as f64 * 0.07 + 0.4, 0.7)
                    .gain(1.0 / CHANNELS as f64)
                    .to_f32()
            ]);

        for _ in 0..8 {
            for sample in out.samples.iter_mut() {
                let mut x = *sample;

                // Apply the filter once. Do this in an unrolled loop to ensure
                // `x` stays in CPU registers as much as possible
                for_each_channel!(|channel| {
                    x[channel] =
                        filter_states1[channel].apply_to_sample(x[channel], &FILTER1[channel]);
                });

                // Force the live ranges of the variables to overlap
                hadamard_transform(&mut x);

                // Apply the filter again
                for_each_channel!(|channel| {
                    x[channel] =
                        filter_states2[channel].apply_to_sample(x[channel], &FILTER2[channel]);
                });

                *sample = x;
            }
        }
    }
}

#[inline]
fn hadamard_transform(x: &mut [f32; 16]) {
    macro_rules! butterfly {
        ($a:expr, $b:expr) => {{
            let (a, b) = (x[$a], x[$b]);
            x[$a] = a + b;
            x[$b] = a - b;
        }};
    }
    butterfly!(0, 4);
    butterfly!(1, 5);
    butterfly!(2, 6);
    butterfly!(3, 7);

    butterfly!(0, 2);
    butterfly!(1, 3);
    butterfly!(4, 6);
    butterfly!(5, 7);

    butterfly!(0, 1);
    butterfly!(2, 3);
    butterfly!(4, 5);
    butterfly!(6, 7);
}

struct Xorshift32(u32);

impl Xorshift32 {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
}

/// Coefficients for a normalized biquad filter with the difference equation
/// `y[n] = b0 x[n] + b1 x[n-1] + b2 x[n-2] - a1 y[n-1] - a2 y[n-2]`.
///
/// The transfer function is given by the following equation:
///
/// ```text
///       b0 + b1^(-z) + b2^(-2z)
/// Y/X = -----------------------
///        1 + a1^(-z) + a2^(-2z)
/// ```
///
///
#[derive(Clone, Copy)]
struct BiquadCoefs<T> {
    b0: T,
    b1: T,
    b2: T,
    a1: T,
    a2: T,
}

impl<T: Init> Init for BiquadCoefs<T> {
    const INIT: Self = Self {
        b0: T::INIT,
        b1: T::INIT,
        b2: T::INIT,
        a1: T::INIT,
        a2: T::INIT,
    };
}

impl BiquadCoefs<f64> {
    const fn gain(self, x: f64) -> Self {
        Self {
            b0: self.b0 * x,
            b1: self.b1 * x,
            b2: self.b2 * x,
            ..self
        }
    }

    const fn to_f32(&self) -> BiquadCoefs<f32> {
        BiquadCoefs {
            b0: self.b0 as f32,
            b1: self.b1 as f32,
            b2: self.b2 as f32,
            a1: self.a1 as f32,
            a2: self.a2 as f32,
        }
    }
}

/// Construct a `BiquadCoefs` for a low-pass filter with a given cutoff
/// frequency `f0` and Q value `q`.
///
/// This filter is derived from the following analog prototype in the s-domain
/// (for normalized frequency):
///
/// ```text
///              1
/// H(s) = ---------------
///         s^2 + s/q + 1
/// ```
const fn low_pass_filter(f0: f64, q: f64) -> BiquadCoefs<f64> {
    use core::f64::consts::PI;

    debug_assert!(f0 >= 0.0 && f0 <= 0.5);
    debug_assert!(q > 0.0);
    let w0 = f0 * (PI * 2.0);
    let (sin, cos) = sincos(w0);
    let alpha = sin / (2.0 * q);
    let b0 = (1.0 - cos) * 0.5;
    let b1 = 1.0 - cos;
    let b2 = (1.0 - cos) * 0.5;
    let a0 = 1.0 + alpha;
    let a1 = -2.0 * cos;
    let a2 = 1.0 - alpha;
    BiquadCoefs {
        b0: b0 / a0,
        b1: b1 / a0,
        b2: b2 / a0,
        a1: a1 / a0,
        a2: a2 / a0,
    }
}

#[derive(Clone, Copy, Default)]
struct BiquadKernelState<T>(T, T);

impl<T: Init> Init for BiquadKernelState<T> {
    const INIT: Self = Self(T::INIT, T::INIT);
}

impl<T> BiquadKernelState<T>
where
    T: Default + Copy + ops::Mul<Output = T> + ops::Add<Output = T> + ops::Sub<Output = T>,
{
    #[inline]
    fn apply_to_sample(&mut self, x: T, coefs: &BiquadCoefs<T>) -> T {
        // Direct form 2 implementation
        let t = x - (self.0 * coefs.a1 + self.1 * coefs.a2);
        let y = t * coefs.b0 + self.0 * coefs.b1 + self.1 * coefs.b2;
        self.1 = self.0;
        self.0 = t;
        y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanity() {
        let _ = env_logger::builder().is_test(true).try_init();

        let mut out = KernelOutput::INIT;
        let mut state = KernelState::INIT;
        state.run(&mut out);

        log::trace!("out = {:#?}", out);

        for sample in out.samples.iter() {
            for &value in sample.iter() {
                // The output must not include NaN or infinity
                assert!(value.is_finite());

                // The output must be non-zero
                assert_ne!(value, 0.0);
            }
        }
    }
}
