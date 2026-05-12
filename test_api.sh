#!/bin/bash
# FactoryTool API Test Script
# Tests all REST endpoints defined in factory-tool-api.md against the device

DEVICE="192.168.100.1"
BASE_URL="http://${DEVICE}/api"
export no_proxy="$DEVICE,localhost,127.0.0.1"
CURL="curl -s"
PASS=0
FAIL=0

green() { printf "\033[32m%s\033[0m\n" "$1"; }
red()   { printf "\033[31m%s\033[0m\n" "$1"; }
bold()  { printf "\033[1m%s\033[0m\n" "$1"; }

check_response() {
    local test_name="$1"
    local http_code="$2"
    local body="$3"
    
    if [ "$http_code" = "000" ]; then
        red "  ✗ $test_name — connection failed"
        FAIL=$((FAIL + 1))
        return 1
    fi
    
    local code=$(echo "$body" | python3 -c "import sys,json; print(json.load(sys.stdin).get('code', 'missing'))" 2>/dev/null)
    
    if [ "$code" = "200" ]; then
        green "  ✓ $test_name (HTTP $http_code, code=$code)"
        PASS=$((PASS + 1))
        return 0
    else
        local msg=$(echo "$body" | python3 -c "import sys,json; print(json.load(sys.stdin).get('message', 'N/A'))" 2>/dev/null)
        red "  ✗ $test_name — HTTP $http_code, code=$code, message=$msg"
        FAIL=$((FAIL + 1))
        return 1
    fi
}

print_json() {
    echo "$1" | python3 -m json.tool 2>/dev/null || echo "$1"
}

bold "══════════════════════════════════════════════"
bold "  FactoryTool API Test — $DEVICE"
bold "══════════════════════════════════════════════"
echo ""

# ──────── 1. GET /api/device_activate_info ────────
bold "▶ 1. GET /api/device_activate_info"
resp=$($CURL -s -o /tmp/api_resp.json -w "%{http_code}" "$BASE_URL/device_activate_info" --connect-timeout 5 --max-time 10)
body=$(cat /tmp/api_resp.json 2>/dev/null)
check_response "device_activate_info" "$resp" "$body"
print_json "$body"
echo ""

# ──────── 2. GET /api/device_name_get ────────
bold "▶ 2. GET /api/device_name_get"
resp=$($CURL -s -o /tmp/api_resp.json -w "%{http_code}" "$BASE_URL/device_name_get" --connect-timeout 5 --max-time 10)
body=$(cat /tmp/api_resp.json 2>/dev/null)
check_response "device_name_get" "$resp" "$body"
print_json "$body"
echo ""

# ──────── 3. POST /api/device_name_set ────────
bold "▶ 3. POST /api/device_name_set (name=V100)"
resp=$($CURL -s -o /tmp/api_resp.json -w "%{http_code}" -X POST "$BASE_URL/device_name_set" \
  -H "Content-Type: application/json" \
  -d '{"device_name": "V100"}' \
  --connect-timeout 5 --max-time 10)
body=$(cat /tmp/api_resp.json 2>/dev/null)
check_response "device_name_set" "$resp" "$body"
print_json "$body"
echo ""

# ──────── 4. GET /api/device_name_get (verify) ────────
bold "▶ 4. GET /api/device_name_get (verify set = V100)"
resp=$($CURL -s -o /tmp/api_resp.json -w "%{http_code}" "$BASE_URL/device_name_get" --connect-timeout 5 --max-time 10)
body=$(cat /tmp/api_resp.json 2>/dev/null)
check_response "device_name_get (verify)" "$resp" "$body"
name=$(echo "$body" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('device_name',''))" 2>/dev/null)
if [ "$name" = "V100" ]; then
    green "  ✓ device_name confirmed as 'V100'"
else
    red "  ✗ device_name is '$name', expected 'V100'"
fi
echo ""

# ──────── 5. POST /api/device_sn_set ────────
SN=$(python3 -c "import random; print(''.join(random.choices('0123456789', k=12)))")
bold "▶ 5. POST /api/device_sn_set (SN=$SN)"
resp=$($CURL -s -o /tmp/api_resp.json -w "%{http_code}" -X POST "$BASE_URL/device_sn_set" \
  -H "Content-Type: application/json" \
  -d "{\"serial_number\": \"$SN\"}" \
  --connect-timeout 5 --max-time 10)
body=$(cat /tmp/api_resp.json 2>/dev/null)
check_response "device_sn_set" "$resp" "$body"
print_json "$body"
echo ""

# ──────── 6. GET /api/device_sn_get (verify) ────────
bold "▶ 6. GET /api/device_sn_get (verify set SN)"
resp=$($CURL -s -o /tmp/api_resp.json -w "%{http_code}" "$BASE_URL/device_sn_get" --connect-timeout 5 --max-time 10)
body=$(cat /tmp/api_resp.json 2>/dev/null)
check_response "device_sn_get" "$resp" "$body"
readback=$(echo "$body" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('serial_number',''))" 2>/dev/null)
if [ "$readback" = "$SN" ]; then
    green "  ✓ SN readback = $readback (matches)"
else
    red "  ✗ SN readback='$readback', expected='$SN'"
fi
print_json "$body"
echo ""

# ──────── 7. GET /api/license_get ────────
bold "▶ 7. GET /api/license_get"
resp=$($CURL -s -o /tmp/api_resp.json -w "%{http_code}" "$BASE_URL/license_get" --connect-timeout 5 --max-time 10)
body=$(cat /tmp/api_resp.json 2>/dev/null)
check_response "license_get" "$resp" "$body"
print_json "$body"
echo ""

# ──────── 8. POST /api/device_activate ────────
bold "▶ 8. POST /api/device_activate (generate JWT and activate)"
activate_info=$($CURL -s "$BASE_URL/device_activate_info" --connect-timeout 5 --max-time 10)
imei=$(echo "$activate_info" | python3 -c "import sys,json; print(json.load(sys.stdin).get('data',{}).get('imei',''))" 2>/dev/null)
if [ -n "$imei" ]; then
    bold "   IMEI from device: $imei"
    
    jwt=""
    if python3 -c "import jwt; print('ok')" 2>/dev/null; then
        jwt=$(python3 -c "
import jwt, time
claims = {
    'iat': int(time.time()),
    'exp': int(time.time()) + 365*86400,
    'imei': '$imei',
    'level': 1
}
token = jwt.encode(claims, 'AARRIIXXOO22001177', algorithm='HS256')
print(token)
" 2>/dev/null)
    fi
    
    if [ -n "$jwt" ]; then
        bold "   JWT generated: ${jwt:0:40}..."
        resp=$($CURL -s -o /tmp/api_resp.json -w "%{http_code}" -X POST "$BASE_URL/device_activate" \
          -H "Content-Type: application/json" \
          -d "{\"lic_content\": \"$jwt\"}" \
          --connect-timeout 5 --max-time 10)
        body=$(cat /tmp/api_resp.json 2>/dev/null)
        check_response "device_activate" "$resp" "$body"
        print_json "$body"
        
        pretty_body=$(echo "$body" | python3 -m json.tool 2>/dev/null)
        bold "   → license_get after activation:"
        resp2=$($CURL -s "$BASE_URL/license_get" --connect-timeout 5 --max-time 10)
        echo "$resp2" | python3 -m json.tool 2>/dev/null
    else
        bold "   ⚠ pyjwt not available, installing..."
        pip3 install pyjwt -q 2>/dev/null
        if python3 -c "import jwt; print('ok')" 2>/dev/null; then
            bold "   ✅ pyjwt installed, rerun to test activation"
        else
            bold "   ⚠ Could not install pyjwt"
        fi
    fi
else
    bold "   ⚠ Could not get IMEI from device, skipping activation test"
fi
echo ""

# ──────── Summary ────────
bold "══════════════════════════════════════════════"
bold "  Results: $PASS passed, $FAIL failed"
bold "══════════════════════════════════════════════"

rm -f /tmp/api_resp.json
exit $FAIL
