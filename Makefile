examples: examples/authors examples/kitchen-sink-riverqueue

examples/authors: $(shell find examples/authors -type f)
	@cd examples/authors && sqlc -f sqlc.dev.yaml generate

examples/kitchen-sink-riverqueue: $(shell find examples/kitchen-sink-riverqueue -type f)
	@cd examples/kitchen-sink-riverqueue/queries && sqlc -f sqlc.dev.yaml generate

.PHONY: build
build: $(shell find src -type f)
	nix build

.PHONY: examples
default: examples