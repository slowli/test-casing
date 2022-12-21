//! Integration tests for `test_casing` macro.

#![cfg_attr(feature = "nightly", feature(test, custom_test_frameworks))]

use async_std::task;

use std::error::Error;

use test_casing::{cases, test_casing, Product, TestCases};

// Cases can be reused across multiple tests.
const CASES: TestCases<i32> = cases!([2, 3, 5, 8]);

#[test_casing(4, CASES)]
#[test]
fn numbers_are_small(number: i32) {
    assert!((0..10).contains(&number));
}

#[test]
fn another_number_is_small() {
    numbers_are_small(1);
}

#[allow(unused_variables)] // should be retained on the target fn
#[test_casing(4, CASES)]
#[ignore = "testing that `#[ignore]` attr works"]
fn numbers_are_large(number: i32) {
    unimplemented!("implement later");
}

#[test_casing(4, CASES)]
fn numbers_are_small_with_errors(number: i32) -> Result<(), Box<dyn Error>> {
    if number < 10 {
        Ok(())
    } else {
        Err("number is too large".into())
    }
}

// It's possible to specify cases with multiple args. The semantics of args
// (e.g., whether any of them are expected values) is up to the user.
const MULTI_ARG_CASES: TestCases<(i32, &str)> = cases!([(2, "2"), (3, "3"), (5, "5")]);

#[test_casing(3, MULTI_ARG_CASES)]
#[test]
fn number_can_be_converted_to_string(number: i32, expected: &str) {
    assert_eq!(number.to_string(), expected);
}

#[test_casing(3, MULTI_ARG_CASES)]
fn number_can_be_converted_to_string_with_tuple_input((number, expected): (i32, &str)) {
    assert_eq!(number.to_string(), expected);
}

// `Product` allows testing a Cartesian product of the contained cases of arity in 2..8.
#[test_casing(12, Product((CASES, ["first", "second", "third"])))]
fn cartesian_product(number: i32, s: &str) {
    assert_ne!(number.to_string(), s);
}

// If it semantically makes sense, it's possible to borrow some of the returned case args
// using a `#[map(ref)]` attr on the arg. An optional transform on the reference in a form
// of a path can be specified as well. (Here, the transform is trivial and serves the purpose
// of assisting the Rust type inference.)
#[test_casing(5, cases!{(0..5).map(|i| (i.to_string(), i))})]
fn string_conversion(#[map(ref = String::as_str)] s: &str, expected: i32) {
    let actual: i32 = s.parse().unwrap();
    assert_eq!(actual, expected);
}

#[test_casing(3, ["not a number", "-", ""])]
#[should_panic(expected = "ParseIntError")]
fn string_conversion_fail(bogus_str: &str) {
    string_conversion(bogus_str, 42);
}

const STRING_CASES: TestCases<(String, i32)> = cases!((0..5).map(|i| (i.to_string(), i)));

#[test_casing(5, STRING_CASES)]
#[async_std::test]
async fn async_string_conversion(#[map(ref)] s: &str, expected: i32) -> Result<(), Box<dyn Error>> {
    let actual: i32 = s.parse()?;
    assert_eq!(actual, expected);
    let expected_string = task::spawn_blocking(move || expected.to_string()).await;
    assert_eq!(expected_string, s);
    Ok(())
}

// Tests paths to tests in modules.
mod random {
    use rand::{rngs::StdRng, Rng, SeedableRng};

    use std::iter;

    use test_casing::{cases, test_casing, TestCases};

    // The library can be used for randomized tests as well, but it's probably not the best choice
    // if the number of test cases should be large.
    const RANDOM_NUMBERS: TestCases<u32> = cases!({
        let mut rng = StdRng::seed_from_u64(123_456);
        iter::repeat_with(move || rng.gen())
    });

    #[test_casing(10, RANDOM_NUMBERS)]
    fn randomized_tests(value: u32) {
        assert!(value.to_string().len() <= 10);
    }
}

#[test]
fn unit_test_detection_works() {
    assert!(option_env!("CARGO_TARGET_TMPDIR").is_some());
}
