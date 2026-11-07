#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| fogata_core::fuzzing::invite_text(data));
