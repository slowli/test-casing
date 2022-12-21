use test_casing::test_casing;

#[test_casing(2, ["test", "this"])]
fn tested_function(_arg: i32) {
    // Does nothing
}

fn main() {}
