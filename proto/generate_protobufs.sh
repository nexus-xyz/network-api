#!/bin/sh

mkdir -p clients/flutter/lib/src/generated
protoc --experimental_allow_proto3_optional --dart_out=grpc:clients/flutter/lib/src/generated -Iproto proto/orchestrator.proto
dart format clients/flutter/lib/src/generated
(cd clients/dummy_client && cargo build || echo clients/dummy_client not found, possibly due to a sparse checkout.)
(cd clients/cli && cargo build || echo clients/cli not found, possibly due to a sparse checkout.)
(cd orchestrator && cargo build || echo orchestrator/ not found, possibly due a sparse checkout.)
