use test_casing::test_casing;

#[test_casing(2, ["test", "this"])]
#[should_panic(expected = "!", bogus = true)]
fn tested_function(_arg: &str) {
    // Does nothing
}

#[test_casing(2, ["test", "this"])]
#[should_panic(expected = 777)]
fn other_tested_function(_arg: &str) {
    // Does nothing
}

fn main() {}
