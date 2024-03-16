#![cfg_attr(not(any(feature = "export-abi", test)), no_main)]

#[cfg(feature = "export-abi")]
fn main() {
    geocab::main();
}
