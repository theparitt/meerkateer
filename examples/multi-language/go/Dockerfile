FROM golang:1.24-alpine@sha256:8bee1901f1e530bfb4a7850aa7a479d17ae3a18beb6e09064ed54cfd245b7191 AS builder
WORKDIR /source
COPY sdk/go /source/sdk/go
COPY examples/multi-language/go /source/examples/multi-language/go
WORKDIR /source/examples/multi-language/go
RUN CGO_ENABLED=0 go build -trimpath -o /out/demo-server .

FROM scratch
COPY --from=builder /out/demo-server /demo-server
USER 65532:65532
ENTRYPOINT ["/demo-server"]
