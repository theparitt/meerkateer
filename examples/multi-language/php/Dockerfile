FROM php:8.3-cli-alpine@sha256:8dbeb51c352cf1f70d5d50f3642c9fbed311e914475ea2382f5bc1525bef7bb3
WORKDIR /app
COPY sdk/php /app/sdk/php
COPY examples/multi-language/php /app/examples/multi-language/php
RUN chmod 0555 /app/examples/multi-language/php/entrypoint.sh
USER 65532:65532
ENTRYPOINT ["/app/examples/multi-language/php/entrypoint.sh"]
