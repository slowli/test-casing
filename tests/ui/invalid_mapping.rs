use test_casing::test_casing;

#[test_casing(2, ["test", "this"].map(String::from))]
fn tested_function(#[map] _arg: &str) {
    // Does nothing
}

#[test_casing(2, ["test", "this"].map(String::from))]
fn other_tested_function(#[map(mut)] _arg: &str) {
    // Does nothing
}

#[test_casing(2, ["test", "this"].map(String::from))]
fn another_tested_function(#[map(ref = "String::as_str")] _arg: &str) {
    // Does nothing
}

fn main() {}
