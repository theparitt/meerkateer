.PHONY: bootstrap source-archive dev migrate down logs test lint integration smoke agent-doctor

bootstrap:
	./scripts/bootstrap.sh

source-archive:
	tar -czf art/meerkateer-source-preview.tar.gz \
		--exclude='**/node_modules' --exclude='**/dist' --exclude='**/target' \
		--exclude='**/.env' --exclude='**/.env.*' --exclude='**/*.pem' --exclude='**/*.key' \
		--exclude='**/__pycache__' --exclude='**/*.tsbuildinfo' \
		--exclude='art/meerkateer-source-preview.tar.gz' \
		README.md CHANGELOG.md CODE_OF_CONDUCT.md CONTRIBUTING.md DCO GOVERNANCE.md \
		LICENSE NOTICE SECURITY.md SUPPORT.md TRADEMARKS.md Cargo.toml Cargo.lock \
		rust-toolchain.toml rustfmt.toml Dockerfile compose.yaml Makefile .env.example \
		.gitignore .dockerignore .editorconfig .gitattributes crates meerkateer-agent \
		meerkateer-server meerkateer-worker meerkateer-web docs migrations openapi sdk \
		schemas scripts tests examples packaging dev art

dev: bootstrap source-archive
	docker compose up --build

migrate: bootstrap
	docker compose up -d --wait postgres
	./scripts/migrate-existing.sh

down:
	docker compose down

logs:
	docker compose logs --follow

test:
	cargo test --workspace --all-targets
	python3 tests/project/validate_repository.py
	python3 tests/contracts/validate_contracts.py
	python3 tests/project/validate_openapi.py
	python3 tests/security/scan_secrets.py
	python3 -m unittest discover -s sdk/python/tests -v
	cd meerkateer-web && npm test

lint:
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --all-features -- -D warnings
	cd meerkateer-web && npm run check:api
	cd meerkateer-web && npm run check

integration: bootstrap
	./scripts/integration.sh

smoke:
	./scripts/smoke.sh

agent-doctor:
	cargo run --locked -p meerkateer-agent -- doctor
