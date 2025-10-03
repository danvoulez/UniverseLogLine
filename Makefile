# Convenience Make targets for LogLine testing, fuzzing, benchmarking and docs.

.PHONY: test fuzz bench doc

test:
	cargo test

fuzz:
	cargo fuzz run fuzz_envelope

bench:
	cargo bench

doc:
	cargo doc --open
