# Professional Build Infrastructure
# Limit concurrency to nproc / 2 for stability
JOBS := $(shell echo $$(( $$(nproc) / 2 )))
CARGO = cargo --config 'build.jobs=$(JOBS)'

# Colors for output
GREEN  := $(shell tput -Txterm setaf 2)
YELLOW := $(shell tput -Txterm setaf 3)
RESET  := $(shell tput -Txterm sgr0)

# Project metadata
PROJECT_NAME := ivaldi-mcp
IMAGE_NAME := daryltucker/ivaldi-mcp
TAG := latest

.PHONY: all build release install test test-coverage check clean

all: build

build:
	@echo "$(GREEN)Building $(PROJECT_NAME)...$(RESET)"
	$(CARGO) build --workspace

release:
	@echo "$(GREEN)Building release binaries...$(RESET)"
	$(CARGO) build --workspace --release

install: release
	@echo "$(GREEN)Installing to ~/.cargo/bin...$(RESET)"
	install -m 755 target/release/ivaldi ~/.cargo/bin/ivaldi
	install -m 755 target/release/ivaldi-server ~/.cargo/bin/ivaldi-server
	@echo "$(GREEN)Installed to ~/.cargo/bin$(RESET)"

test-unit:
	@echo "$(GREEN)Running unit/integration tests with nextest (jobs: $(JOBS))...$(RESET)"
	$(CARGO) nextest run --workspace

test-coverage:
	@echo "$(GREEN)Generating coverage report...$(RESET)"
	$(CARGO) llvm-cov nextest --workspace --html

test-tier3: build
	@echo "$(GREEN)Running Tier 3: Real World Tests...$(RESET)"
	python3 tests/tier3_fresh_install.py

test: test-unit test-tier3

test-all: test test-tier3

check:
	@echo "$(GREEN)Running cargo check + clippy (jobs: $(JOBS))...$(RESET)"
	$(CARGO) check --workspace
	$(CARGO) clippy --workspace -- -D warnings
	@echo "$(GREEN)Running coverage enforcement...$(RESET)"
	./tests/scripts/enforce_coverage.py

# Docker targets
docker-build:
	@echo "$(GREEN)Building Docker image...$(RESET)"
	docker build -t $(IMAGE_NAME):$(TAG) .

docker-run:
	@echo "$(GREEN)Running in Docker...$(RESET)"
	docker run -it --rm $(IMAGE_NAME):$(TAG)

clean:
	@echo "$(YELLOW)Cleaning build artifacts...$(RESET)"
	$(CARGO) clean