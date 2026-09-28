# syntax=docker/dockerfile:1.7
FROM node:22.22.2-alpine3.23
WORKDIR /source/meerkateer-web
COPY meerkateer-web/package.json meerkateer-web/package-lock.json ./
RUN npm ci --ignore-scripts
COPY meerkateer-web ./
COPY art ../art
COPY README.md CHANGELOG.md CONTRIBUTING.md GOVERNANCE.md LICENSE SECURITY.md SUPPORT.md ../
COPY docs ../docs
COPY sdk/python/README.md ../sdk/python/
COPY sdk/rust/README.md ../sdk/rust/
COPY sdk/node/README.md ../sdk/node/
COPY sdk/go/README.md ../sdk/go/
USER node
EXPOSE 5173
CMD ["npm", "run", "dev", "--", "--host", "0.0.0.0", "--configLoader", "runner"]
