#!/bin/bash

# Test login and logout functionality
# This script tests explicit session management via /auth/login and /auth/logout

set -e

echo "=== Testing Login and Logout Functionality ==="
echo ""

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Test 1: Login with JSON body
echo "Test 1: Login with JSON body"
LOGIN_RESPONSE=$(curl -s -i -X POST "http://localhost:8080/auth/login" \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "admin123"}')

echo "$LOGIN_RESPONSE" | head -20

# Extract token from JSON response
TOKEN=$(echo "$LOGIN_RESPONSE" | grep -o '"token":"[^"]*"' | cut -d'"' -f4)

if [ -z "$TOKEN" ]; then
  echo -e "${RED}✗ Failed to get token from login${NC}"
  exit 1
fi

echo -e "${GREEN}✓ Login successful, token: ${TOKEN:0:20}...${NC}"
echo ""

# Test 2: Login with Basic Auth header
echo "Test 2: Login with Basic Auth header"
LOGIN_RESPONSE2=$(curl -s -i -X POST "http://localhost:8080/auth/login" \
  -u admin:admin123)

TOKEN2=$(echo "$LOGIN_RESPONSE2" | grep -o '"token":"[^"]*"' | cut -d'"' -f4)

if [ -z "$TOKEN2" ]; then
  echo -e "${RED}✗ Failed to get token from Basic Auth login${NC}"
  exit 1
fi

echo -e "${GREEN}✓ Login with Basic Auth successful, token: ${TOKEN2:0:20}...${NC}"
echo ""

# Test 3: Use token to access protected endpoint
echo "Test 3: Use session token to access protected cache endpoint"
CACHE_RESPONSE=$(curl -s -w "\n%{http_code}" -X PUT "http://localhost:8080/cache/test/key1" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"value": "test123", "ttl_ms": 3600000}')

HTTP_CODE=$(echo "$CACHE_RESPONSE" | tail -1)

if [ "$HTTP_CODE" = "200" ]; then
  echo -e "${GREEN}✓ Successfully used session token to access cache${NC}"
else
  echo -e "${RED}✗ Failed to access cache with session token (HTTP $HTTP_CODE)${NC}"
  exit 1
fi
echo ""

# Test 4: Logout session
echo "Test 4: Logout session"
LOGOUT_RESPONSE=$(curl -s -i -X POST "http://localhost:8080/auth/logout" \
  -H "Authorization: Bearer $TOKEN")

echo "$LOGOUT_RESPONSE" | head -15

LOGOUT_MESSAGE=$(echo "$LOGOUT_RESPONSE" | grep -o '"message":"[^"]*"')

if [ -z "$LOGOUT_MESSAGE" ]; then
  echo -e "${RED}✗ Logout failed${NC}"
  exit 1
fi

echo -e "${GREEN}✓ Logout successful${NC}"
echo ""

# Test 5: Try to use logged out token (should fail)
echo "Test 5: Try to use logged out token (should fail with 401)"
CACHE_RESPONSE2=$(curl -s -w "\n%{http_code}" -X GET "http://localhost:8080/cache/test/key1" \
  -H "Authorization: Bearer $TOKEN")

HTTP_CODE2=$(echo "$CACHE_RESPONSE2" | tail -1)

if [ "$HTTP_CODE2" = "401" ]; then
  echo -e "${GREEN}✓ Correctly rejected logged out token (HTTP 401)${NC}"
else
  echo -e "${RED}✗ Should have rejected logged out token, got HTTP $HTTP_CODE2${NC}"
  exit 1
fi
echo ""

# Test 6: Regular API calls still use transparent session reuse
echo "Test 6: Regular API calls with Basic Auth (transparent session reuse)"
REGULAR_RESPONSE=$(curl -s -i -u admin:admin123 -X GET "http://localhost:8080/cache/test/key1")

SESSION_TOKEN=$(echo "$REGULAR_RESPONSE" | grep -i "x-session-token:" | head -1 | cut -d' ' -f2 | tr -d '\r\n')
SESSION_REUSED=$(echo "$REGULAR_RESPONSE" | grep -i "x-session-reused:" | head -1 | cut -d' ' -f2 | tr -d '\r\n')

if [ -z "$SESSION_TOKEN" ]; then
  echo -e "${RED}✗ No session token in response${NC}"
  exit 1
fi

echo -e "${GREEN}✓ Regular API call with Basic Auth works (session token: ${SESSION_TOKEN:0:20}...)${NC}"
echo -e "${GREEN}✓ Session reused: $SESSION_REUSED${NC}"
echo ""

echo -e "${GREEN}=== All Tests Passed ===${NC}"
