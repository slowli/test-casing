use test_casing::test_casing;

#[test_casing(1, [(1, 2, 3, 4, 5, 6, 7, 8)])]
fn tested_function(
    _arg0: i32,
    _arg1: i32,
    _arg2: i32,
    _arg3: i32,
    _arg4: i32,
    _arg5: i32,
    _arg6: i32,
    _arg7: i32,
) {
    // Does nothing
}

fn main() {}
