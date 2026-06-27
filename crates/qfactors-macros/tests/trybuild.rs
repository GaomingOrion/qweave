#[test]
fn factor_macro_ui() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/pass_basic.rs");
    cases.pass("tests/ui/pass_params.rs");
    cases.pass("tests/ui/pass_windows_outputs_result.rs");
    cases.compile_fail("tests/ui/fail_input_type.rs");
    cases.compile_fail("tests/ui/fail_param_mismatch.rs");
    cases.compile_fail("tests/ui/fail_return_type.rs");
    cases.compile_fail("tests/ui/fail_tuple_outputs.rs");
    cases.compile_fail("tests/ui/fail_window_zero.rs");
}
