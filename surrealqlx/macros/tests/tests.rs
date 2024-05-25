#[test]
#[cfg_attr(tarpaulin, ignore)]
pub fn pass() {
    macrotest::expand("tests/expand/*.rs");
}
