.DEFAULT_GOAL := install
.PHONY: install
install:
	@cargo install -f --path . --debug
