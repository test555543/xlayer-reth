.PHONY: build-docker
build-docker:
	docker build -t xlayer-reth-node:latest -f Dockerfile .

.PHONY: run-hello
run-hello:
	docker run --rm xlayer-reth-node:latest --help

.PHONY: check
check:
	just check

.PHONY: test
test:
	just test

.PHONY: fix
fix:
	just fix