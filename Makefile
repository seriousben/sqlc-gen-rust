examples: $(shell find src -type f) $(shell find examples -type f)
	nix build
	cd examples && sqlc -f sqlc.dev.yaml generate

default: examples