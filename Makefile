EXAMPLES = $(wildcard examples/*)

examples: $(EXAMPLES)

$(EXAMPLES): $(shell find $@ -type f) .wasm.build
	@cd $@ && sqlc -f sqlc.dev.yaml generate && cargo build

.wasm.build: $(shell find src -type f)
	nix build
	echo "sentinel file" > .wasm.build

.PHONY: dev/psql
dev/psql:
	psql -d postgresql://postgres:postgres@localhost/app

.PHONY: default
default: examples
