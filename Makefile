
build:
	cargo build

.PHONY: test
test: build
	./run_integration_tests.sh

.PHONY: coverage
coverage:
	cargo tarpaulin --out Html --engine llvm

clean:
	cargo clean