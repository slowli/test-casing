use test_casing::test_casing;

#[test_casing(2, ["test", "this"])]
fn tested_function<S: Into<String>>(_arg: S) {
    // Does nothing
}

fn main() {}
