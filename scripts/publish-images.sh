#!/usr/bin/env sh
set -eu

action="${1:-}"
version="${2:-}"
case "$action" in
    build|push) ;;
    *)
        echo 'Usage: scripts/publish-images.sh <build|push> <version>' >&2
        exit 2
        ;;
esac
case "$version" in
    ''|*[!A-Za-z0-9._-]*|[.-]*|*..*)
        echo 'Version must be a non-empty Docker tag using letters, numbers, dots, underscores, or dashes.' >&2
        exit 2
        ;;
esac
if [ "${#version}" -gt 128 ]; then
    echo 'Version must be at most 128 characters.' >&2
    exit 2
fi

registry="${MEERKATEER_REGISTRY:-ghcr.io/theparitt}"
platform="${MEERKATEER_IMAGE_PLATFORM:-linux/amd64}"
case "$registry" in
    ''|*[!A-Za-z0-9./:_-]*|[-.]*|*/)
        echo 'MEERKATEER_REGISTRY is not a valid container registry path.' >&2
        exit 2
        ;;
esac
server_image="${registry}/meerkateer-server"
worker_image="${registry}/meerkateer-worker"
revision="$(git rev-parse --verify HEAD 2>/dev/null || printf unknown)"
created="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

# The hardened image contains both binaries. Build once, then create the worker
# tag locally; production Compose selects the worker with an entrypoint override.
docker build \
    --platform "$platform" \
    --label "org.opencontainers.image.source=https://github.com/theparitt/meerkateer" \
    --label "org.opencontainers.image.version=$version" \
    --label "org.opencontainers.image.revision=$revision" \
    --label "org.opencontainers.image.created=$created" \
    --tag "${server_image}:${version}" \
    .
docker tag "${server_image}:${version}" "${worker_image}:${version}"

if [ "${MEERKATEER_PUBLISH_LATEST:-false}" = "true" ]; then
    docker tag "${server_image}:${version}" "${server_image}:latest"
    docker tag "${server_image}:${version}" "${worker_image}:latest"
fi

if [ "$action" = "push" ]; then
    docker push "${server_image}:${version}"
    docker push "${worker_image}:${version}"
    if [ "${MEERKATEER_PUBLISH_LATEST:-false}" = "true" ]; then
        docker push "${server_image}:latest"
        docker push "${worker_image}:latest"
    fi
    echo "Published Meerkateer server and worker images with tag $version."
else
    echo "Built Meerkateer server and worker images locally with tag $version."
fi
