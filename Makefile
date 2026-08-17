.DEFAULT_GOAL := help

.PHONY: help build test check fmt run menu

help:
	@echo "Targets: build, test, check, fmt, run, menu"

build:
	cargo build --release

test:
	cargo test --locked

check:
	cargo check --locked

fmt:
	cargo fmt --check

run:
	cargo run --release --bin capshift

menu:
	cargo run --release --bin capshift-menu
