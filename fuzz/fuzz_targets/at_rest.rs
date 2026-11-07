#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| fogata_core::fuzzing::at_rest(data));
