FROM python:3.13-alpine@sha256:79e7a9b9ff1cbceff819f856fb374477792a5967759d94df266de7b7b4120e6f
WORKDIR /app
COPY sdk/python /app/sdk/python
COPY examples/multi-language/python/server.py /app/server.py
ENV PYTHONPATH=/app/sdk/python/src PYTHONUNBUFFERED=1
USER 65532:65532
ENTRYPOINT ["python3", "/app/server.py"]
