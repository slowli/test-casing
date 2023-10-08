use test_casing::test_casing;

#[test_casing(2, ["test", "this"])]
#[ignore(_arg > 1)]
fn tested_function(_arg: &str) {
    // Does nothing
}

#[test_casing(2, ["test", "this"])]
#[ignore(message = "???")]
fn other_tested_function(_arg: &str) {
    // Does nothing
}

fn main() {}
