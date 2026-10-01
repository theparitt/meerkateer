FROM node:22.22.2-alpine3.23@sha256:8ea2348b068a9544dae7317b4f3aafcdc032df1647bb7d768a05a5cad1a7683f
WORKDIR /app
COPY sdk/node /app/sdk/node
COPY examples/multi-language/node/server.mjs /app/examples/multi-language/node/server.mjs
USER 65532:65532
ENTRYPOINT ["node", "/app/examples/multi-language/node/server.mjs"]
