use test_casing::test_casing;

#[test_casing("2", ["test", "this"])]
fn tested_function(_arg: &str) {
    // Does nothing
}

#[test_casing(0, [])]
fn other_tested_function(_arg: &str) {
    // Does nothing
}

fn main() {}
