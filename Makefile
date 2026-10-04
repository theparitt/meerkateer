.PHONY: bootstrap source-archive images publish-images production-env production-config production-pull production-up production-logs production-down dev migrate backup restore down logs test lint integration failure-lab control-plane-lab sdk-lab sdk-lab-test sdk-lab-down sdk-lab-logs smoke agent-doctor

PRODUCTION_ENV ?= deploy/.env.production
PRODUCTION_COMPOSE ?= deploy/compose.production.yaml

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
		schemas scripts tests examples packaging deploy dev art

images:
	@test -n "$(VERSION)" || (echo 'Usage: make images VERSION=<tag>' >&2; exit 2)
	./scripts/publish-images.sh build "$(VERSION)"

publish-images:
	@test -n "$(VERSION)" || (echo 'Usage: make publish-images VERSION=<immutable-tag>' >&2; exit 2)
	./scripts/publish-images.sh push "$(VERSION)"

production-env:
	@test -n "$(VERSION)" || (echo 'Usage: make production-env VERSION=<published-tag>' >&2; exit 2)
	./scripts/bootstrap-production-env.sh "$(VERSION)"

production-config:
	docker compose --env-file "$(PRODUCTION_ENV)" -f "$(PRODUCTION_COMPOSE)" config --quiet

production-pull: production-config
	docker compose --env-file "$(PRODUCTION_ENV)" -f "$(PRODUCTION_COMPOSE)" pull

production-up: production-config
	docker compose --env-file "$(PRODUCTION_ENV)" -f "$(PRODUCTION_COMPOSE)" up -d --wait

production-logs:
	docker compose --env-file "$(PRODUCTION_ENV)" -f "$(PRODUCTION_COMPOSE)" logs --follow

production-down:
	docker compose --env-file "$(PRODUCTION_ENV)" -f "$(PRODUCTION_COMPOSE)" down

dev: bootstrap source-archive
	docker compose up --build

migrate: bootstrap
	docker compose up -d --wait postgres
	./scripts/migrate-existing.sh

backup:
	./scripts/backup-encrypted.sh

restore:
	@test -n "$(BACKUP)" || (echo 'Usage: make restore BACKUP=/path/to/backup.dump.age' >&2; exit 2)
	./scripts/restore-encrypted.sh "$(BACKUP)"

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
	cd sdk/node && npm test
	cd meerkateer-web && npm test

lint:
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --all-features -- -D warnings
	cd meerkateer-web && npm run check:api
	cd meerkateer-web && npm run check

integration: bootstrap
	./scripts/integration.sh
	./tests/integration/community_alerts.sh

failure-lab: bootstrap
	./tests/integration/community_alerts.sh

control-plane-lab: bootstrap
	./tests/integration/control_plane_faults.sh

sdk-lab: bootstrap
	./scripts/multilang-sdk-lab.sh up

sdk-lab-test: bootstrap
	./scripts/multilang-sdk-lab.sh test

sdk-lab-down:
	./scripts/multilang-sdk-lab.sh down

sdk-lab-logs:
	./scripts/multilang-sdk-lab.sh logs

smoke:
	./scripts/smoke.sh

agent-doctor:
	cargo run --locked -p meerkateer-agent -- doctor
