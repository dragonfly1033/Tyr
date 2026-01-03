
build:
	cargo build

.PHONY: test
test: build
	./run_integration_test.sh

.PHONY: coverage
coverage:
	cargo tarpaulin --out Html --engine llvm

clean:
	cargo clean