#!/bin/bash

echo "will be available at http://localhost:16686"

# Run Jaeger container
docker run -p16686:16686 -p4317:4317 -e COLLECTOR_OTLP_ENABLED=true jaegertracing/all-in-one:latest
