#!/bin/bash

echo "will be available at http://localhost:16686"

# Run Jaeger container
docker run -p 16686:16686 -p 4317:4317 -e COLLECTOR_OTLP_ENABLED=true jaegertracing/all-in-one:latest
