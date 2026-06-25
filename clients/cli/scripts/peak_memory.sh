#!/bin/bash

# This script measures the peak memory usage of the nexus-compute-cli binary.
# It starts the binary in headless mode and monitors the memory usage for specified seconds.
# Usage: ./peak_memory.sh [SECONDS] (default: 60)

DURATION=${1:-60}

echo "Starting nexus-compute-cli and monitoring memory for ${DURATION} seconds..."

# Start the process in background and capture its PID
nexus-compute-cli start --headless &
PID=$!

# Record start time
START_TIME=$(date +%s)

# Monitor memory usage
MAX_MEMORY=0
for i in $(seq 1 $DURATION); do
	if ps -p $PID >/dev/null 2>&1; then
		MEMORY=$(ps -o rss= -p $PID 2>/dev/null | tr -d " ")
		if [ "$MEMORY" ] && [ "$MEMORY" -gt "$MAX_MEMORY" ]; then
			MAX_MEMORY=$MEMORY
		fi
		# Calculate MB for display
		MAX_MEMORY_MB=$((MAX_MEMORY / 1024))
		CURRENT_MEMORY_MB=$((MEMORY / 1024))
		echo "[$i/${DURATION}s] Current: ${CURRENT_MEMORY_MB} MB, Peak: ${MAX_MEMORY_MB} MB"
	else
		echo "Process ended early at ${i}s"
		break
	fi
	sleep 1
done

# Calculate total time
END_TIME=$(date +%s)
TOTAL_TIME=$((END_TIME - START_TIME))

# Kill the process
kill $PID 2>/dev/null
wait $PID 2>/dev/null

echo "================================"
echo "Peak Memory Usage: ${MAX_MEMORY_MB} MB"
echo "Total Runtime: ${TOTAL_TIME} seconds"
echo "================================"
