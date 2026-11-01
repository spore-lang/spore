#![no_main]
use libfuzzer_sys::fuzz_target;
use sporec_parser::{formatter::format_module, parse};

fuzz_target!(|source: &str| {
    if let Ok(module) = parse(source) {
        let formatted = format_module(&module);
        let reparsed = parse(&formatted).expect("formatted source must parse");
        assert_eq!(format_module(&reparsed), formatted);
    }
});
