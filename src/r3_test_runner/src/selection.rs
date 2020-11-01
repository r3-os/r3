//! Test case selection
use itertools::iproduct;
use std::fmt;
use thiserror::Error;

use crate::utils::Joined;

pub struct TestSource {
    pub driver_kernel_tests: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TestRun<'a> {
    pub case: TestCase<'a>,
    pub cpu_lock_by_basepri: bool,
}

/// The CLI name of [`TestRun::cpu_lock_by_basepri`].
const FEAT_CPU_LOCK_BY_BASEPRI: &str = "basepri";

#[derive(Debug, Clone)]
pub enum TestCase<'a> {
    KernelTest(&'static str),
    KernelBenchmark(&'static str),
    /// Driver-defined kernel test
    DriverKernelTest(&'a str),
}

impl fmt::Display for TestRun<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}{}",
            self.case,
            if self.cpu_lock_by_basepri {
                Joined(Some(Joined(("+", FEAT_CPU_LOCK_BY_BASEPRI))))
            } else {
                Joined(None)
            },
        )
    }
}

impl fmt::Display for TestCase<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::KernelTest(name) => write!(f, "kernel_tests::{}", name),
            Self::DriverKernelTest(name) => write!(f, "kernel_tests::{}", name),
            Self::KernelBenchmark(name) => write!(f, "kernel_benchmarks::{}", name),
        }
    }
}

fn all_test_runs(test_source: &TestSource) -> impl Iterator<Item = TestRun<'_>> {
    let cases = r3_test_suite::kernel_tests::TEST_NAMES
        .iter()
        .cloned()
        .map(TestCase::KernelTest);

    let cases = cases.chain(
        r3_test_suite::kernel_benchmarks::TEST_NAMES
            .iter()
            .cloned()
            .map(TestCase::KernelBenchmark),
    );

    let cases = cases.chain(
        test_source
            .driver_kernel_tests
            .iter()
            .map(|s| TestCase::DriverKernelTest(&**s)),
    );

    iproduct!(cases, &[false, true]).map(|(case, &cpu_lock_by_basepri)| TestRun {
        case,
        cpu_lock_by_basepri,
    })
}

#[derive(Debug, Clone)]
pub enum TestFilter {
    Pass,
    CaseNameContains(String),
    CpuLockByBasepri(bool),
    IsBenchmark(bool),
    Conjunction(Vec<TestFilter>),
    Disjuction(Vec<TestFilter>),
}

impl TestFilter {
    fn matches(&self, run: &TestRun) -> bool {
        match self {
            Self::Pass => true,
            Self::CaseNameContains(needle) => run.case.to_string().contains(needle),
            Self::CpuLockByBasepri(value) => run.cpu_lock_by_basepri == *value,
            Self::IsBenchmark(value) => *value == matches!(run.case, TestCase::KernelBenchmark(_)),
            Self::Conjunction(subfilters) => {
                subfilters.iter().all(|subfilter| subfilter.matches(run))
            }
            Self::Disjuction(subfilters) => {
                subfilters.iter().any(|subfilter| subfilter.matches(run))
            }
        }
    }

    pub fn all_matching_test_runs<'a>(
        &'a self,
        test_source: &'a TestSource,
    ) -> impl Iterator<Item = TestRun> + 'a {
        all_test_runs(test_source).filter(move |r| self.matches(r))
    }
}

#[derive(Error, Debug)]
pub enum TestFilterParseError {
    #[error("Unknown feature: '{0}'")]
    UnknownFeature(String),
}

impl std::str::FromStr for TestFilter {
    type Err = TestFilterParseError;

    /// Parse a filter string.
    ///
    /// A filter string should be specified in the following form:
    /// `needle+feat1-feat2`
    ///
    ///  - `needle` chooses the test cases whose names contain `needle`.
    ///  - `+feat1` requires the feature `feat1`.
    ///  - `-feat2` excludes the feature `feat1`.
    ///  - `-prop=value` constrains the property `prop` to `value`. (This is not
    ///    support yet.)
    ///
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut i = s.find(&['-', '+'][..]).unwrap_or_else(|| s.len());
        let mut flt = TestFilter::CaseNameContains(s[0..i].to_owned());

        while i < s.len() {
            let incl = match s.as_bytes()[i] {
                b'+' => true,
                b'-' => false,
                _ => unreachable!(),
            };
            i += 1;

            // Find the next `-` or `+`
            let k = s[i..]
                .find(&['-', '+'][..])
                .map(|k| k + i)
                .unwrap_or_else(|| s.len());

            let feature = &s[i..k];

            if feature == FEAT_CPU_LOCK_BY_BASEPRI {
                flt = TestFilter::Conjunction(vec![TestFilter::CpuLockByBasepri(incl), flt]);
            } else {
                return Err(TestFilterParseError::UnknownFeature(feature.to_owned()));
            }

            i = k;
        }

        Ok(flt)
    }
}
