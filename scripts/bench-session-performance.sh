#!/bin/bash

# Performance benchmark for session management
# Tests session reuse performance under various concurrency levels

set -e

echo "=== Session Management Performance Benchmark ==="
echo ""

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Server URL
URL="http://localhost:8080"

# Test credentials
USER="admin"
PASS="admin123"

echo -e "${BLUE}Scenario 1: Sequential Requests (Baseline)${NC}"
echo "Testing 50 sequential requests to measure session reuse..."
echo ""

START=$(date +%s%3N)
for i in {1..50}; do
  curl -s -u $USER:$PASS -X PUT "$URL/cache/test/key$i" \
    -H "Content-Type: application/json" \
    -d '{"value": "test"}' > /dev/null
done
END=$(date +%s%3N)

SEQUENTIAL_TIME=$((END - START))
SEQUENTIAL_AVG=$((SEQUENTIAL_TIME / 50))

echo -e "${GREEN}✓ Sequential: $SEQUENTIAL_TIME ms total ($SEQUENTIAL_AVG ms avg per request)${NC}"
echo ""

echo -e "${BLUE}Scenario 2: Low Concurrency (10 parallel requests)${NC}"
echo "Testing lock contention with 10 concurrent requests..."
echo ""

START=$(date +%s%3N)
seq 1 10 | xargs -P 10 -I {} curl -s -u $USER:$PASS -X PUT "$URL/cache/test/key{}" \
  -H "Content-Type: application/json" \
  -d '{"value": "test"}' > /dev/null
END=$(date +%s%3N)

LOW_CONC_TIME=$((END - START))
LOW_CONC_AVG=$((LOW_CONC_TIME / 10))

echo -e "${GREEN}✓ Low Concurrency (10): $LOW_CONC_TIME ms total ($LOW_CONC_AVG ms avg per request)${NC}"
echo ""

echo -e "${BLUE}Scenario 3: Medium Concurrency (50 parallel requests)${NC}"
echo "Testing lock contention with 50 concurrent requests..."
echo ""

START=$(date +%s%3N)
seq 1 50 | xargs -P 50 -I {} curl -s -u $USER:$PASS -X PUT "$URL/cache/test/key{}" \
  -H "Content-Type: application/json" \
  -d '{"value": "test"}' > /dev/null
END=$(date +%s%3N)

MED_CONC_TIME=$((END - START))
MED_CONC_AVG=$((MED_CONC_TIME / 50))

echo -e "${GREEN}✓ Medium Concurrency (50): $MED_CONC_TIME ms total ($MED_CONC_AVG ms avg per request)${NC}"
echo ""

echo -e "${BLUE}Scenario 4: High Concurrency (100 parallel requests)${NC}"
echo "Testing lock contention with 100 concurrent requests..."
echo ""

START=$(date +%s%3N)
seq 1 100 | xargs -P 100 -I {} curl -s -u $USER:$PASS -X PUT "$URL/cache/test/key{}" \
  -H "Content-Type: application/json" \
  -d '{"value": "test"}' > /dev/null
END=$(date +%s%3N)

HIGH_CONC_TIME=$((END - START))
HIGH_CONC_AVG=$((HIGH_CONC_TIME / 100))

echo -e "${GREEN}✓ High Concurrency (100): $HIGH_CONC_TIME ms total ($HIGH_CONC_AVG ms avg per request)${NC}"
echo ""

echo -e "${BLUE}Scenario 5: Very High Concurrency (200 parallel requests)${NC}"
echo "Testing extreme lock contention..."
echo ""

START=$(date +%s%3N)
seq 1 200 | xargs -P 200 -I {} curl -s -u $USER:$PASS -X PUT "$URL/cache/test/key{}" \
  -H "Content-Type: application/json" \
  -d '{"value": "test"}' > /dev/null
END=$(date +%s%3N)

VHIGH_CONC_TIME=$((END - START))
VHIGH_CONC_AVG=$((VHIGH_CONC_TIME / 200))

echo -e "${GREEN}✓ Very High Concurrency (200): $VHIGH_CONC_TIME ms total ($VHIGH_CONC_AVG ms avg per request)${NC}"
echo ""

# Summary
echo -e "${YELLOW}=== Performance Summary ===${NC}"
echo ""
printf "%-30s %10s %15s\n" "Scenario" "Total (ms)" "Avg/Req (ms)"
printf "%-30s %10s %15s\n" "--------" "----------" "-------------"
printf "%-30s %10d %15d\n" "Sequential (50 req)" $SEQUENTIAL_TIME $SEQUENTIAL_AVG
printf "%-30s %10d %15d\n" "Low Concurrency (10 req)" $LOW_CONC_TIME $LOW_CONC_AVG
printf "%-30s %10d %15d\n" "Medium Concurrency (50 req)" $MED_CONC_TIME $MED_CONC_AVG
printf "%-30s %10d %15d\n" "High Concurrency (100 req)" $HIGH_CONC_TIME $HIGH_CONC_AVG
printf "%-30s %10d %15d\n" "Very High Concurrency (200)" $VHIGH_CONC_TIME $VHIGH_CONC_AVG
echo ""

# Calculate throughput
SEQUENTIAL_THROUGHPUT=$((50000 / SEQUENTIAL_TIME))
LOW_THROUGHPUT=$((10000 / LOW_CONC_TIME))
MED_THROUGHPUT=$((50000 / MED_CONC_TIME))
HIGH_THROUGHPUT=$((100000 / HIGH_CONC_TIME))
VHIGH_THROUGHPUT=$((200000 / VHIGH_CONC_TIME))

echo -e "${YELLOW}=== Throughput (requests/second) ===${NC}"
echo ""
printf "%-30s %15s\n" "Scenario" "Req/Sec"
printf "%-30s %15s\n" "--------" "-------"
printf "%-30s %15d\n" "Sequential" $SEQUENTIAL_THROUGHPUT
printf "%-30s %15d\n" "Low Concurrency" $LOW_THROUGHPUT
printf "%-30s %15d\n" "Medium Concurrency" $MED_THROUGHPUT
printf "%-30s %15d\n" "High Concurrency" $HIGH_THROUGHPUT
printf "%-30s %15d\n" "Very High Concurrency" $VHIGH_THROUGHPUT
echo ""

# Performance analysis
if [ $MED_CONC_AVG -lt $((SEQUENTIAL_AVG * 2)) ]; then
  echo -e "${GREEN}✓ GOOD: Medium concurrency shows minimal latency degradation${NC}"
else
  echo -e "${YELLOW}⚠ WARNING: Medium concurrency showing significant latency increase${NC}"
fi

if [ $HIGH_CONC_AVG -lt $((SEQUENTIAL_AVG * 3)) ]; then
  echo -e "${GREEN}✓ GOOD: High concurrency handling well (< 3x sequential latency)${NC}"
else
  echo -e "${YELLOW}⚠ WARNING: High concurrency showing significant lock contention${NC}"
fi

echo ""
echo -e "${BLUE}Note: First request includes Argon2 password hashing (~250ms)${NC}"
echo -e "${BLUE}Subsequent requests should use cached session (<5ms)${NC}"
echo ""
echo -e "${GREEN}Benchmark complete!${NC}"
