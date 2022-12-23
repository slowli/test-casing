use test_casing::{cases, test_casing, TestCases};

const CASES: TestCases<i32> = cases!([1, 2]);

#[test_casing(2, CASS)]
fn tested_function(_arg: i32) {
    // Does nothing
}

fn main() {}
