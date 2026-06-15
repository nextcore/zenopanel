import urllib.request
import urllib.parse
import json
import sys
import sqlite3
import time
import os

class NoRedirectHandler(urllib.request.HTTPRedirectHandler):
    def http_error_302(self, req, fp, code, msg, headers):
        return fp
    def http_error_303(self, req, fp, code, msg, headers):
        return fp

def get_cookie_value(cookie_header, name):
    if not cookie_header:
        return None
    cookies = []
    if isinstance(cookie_header, list):
        cookies = cookie_header
    else:
        cookies = [cookie_header]
    for cookie_line in cookies:
        for cookie in cookie_line.split(','):
            parts = cookie.strip().split(';')[0].split('=')
            if len(parts) == 2 and parts[0].strip() == name:
                return parts[1].strip()
    return None

def make_request(url, method='GET', data=None, headers=None):
    if headers is None:
        headers = {}
    encoded_data = None
    if data is not None:
        encoded_data = json.dumps(data).encode('utf-8')
        headers['Content-Type'] = 'application/json'
    req = urllib.request.Request(url, data=encoded_data, headers=headers, method=method)
    try:
        res = urllib.request.urlopen(req)
        body = res.read().decode('utf-8')
        return res.getcode(), res.info(), body
    except urllib.error.HTTPError as e:
        body = e.read().decode('utf-8')
        return e.code, e.info(), body

def main():
    opener = urllib.request.build_opener(NoRedirectHandler())
    urllib.request.install_opener(opener)
    base_url = 'http://127.0.0.1:3001'

    # Get active entrance path from DB
    conn = sqlite3.connect('zeno.db')
    cursor = conn.cursor()
    cursor.execute("SELECT value FROM settings WHERE key = 'entrance_path'")
    row = cursor.fetchone()
    db_entrance_path = row[0] if row else '/login'
    if not db_entrance_path.startswith('/'):
        db_entrance_path = '/' + db_entrance_path
    print(f"Detected active entrance path in DB: {db_entrance_path}")

    # Fetch CSRF token
    print(f"\n1. Fetching CSRF token from {db_entrance_path}...")
    status, info, _ = make_request(base_url + db_entrance_path)
    cookie_hdr = info.get('Set-Cookie')
    csrf_token = get_cookie_value(cookie_hdr, '_csrf')
    if not csrf_token:
        print("❌ FAILED: CSRF token not found")
        sys.exit(1)
    print(f"✅ CSRF token: {csrf_token}")

    # Login as admin
    print(f"\n2. Logging in as admin via {db_entrance_path}...")
    status, info, body = make_request(
        base_url + db_entrance_path,
        method='POST',
        data={"username": "admin", "password": "admin"},
        headers={'X-CSRF-Token': csrf_token, 'Cookie': f'_csrf={csrf_token}'}
    )
    set_cookies = info.get_all('Set-Cookie')
    admin_token = get_cookie_value(set_cookies, 'zeno_token')
    admin_role = get_cookie_value(set_cookies, 'zeno_role')
    if status != 200 or admin_role != 'admin' or not admin_token:
        print("❌ FAILED: Login failed")
        sys.exit(1)
    print("✅ Logged in successfully")

    admin_headers = {
        'Cookie': f'zeno_token={admin_token}; _csrf={csrf_token}',
        'X-CSRF-Token': csrf_token
    }

    # Retrieve current security settings
    print("\n3. GET current security settings...")
    status, _, body = make_request(base_url + '/api/settings/security', headers=admin_headers)
    print(f"Status: {status}, Body: {body}")
    data = json.loads(body)
    if status != 200 or not data.get("success") or "settings" not in data:
        print("❌ FAILED: GET /api/settings/security failed")
        sys.exit(1)
    print("✅ GET security settings works")

    orig_settings = data["settings"]

    # Test updating security settings (setting low rate limit threshold to test blocking)
    print("\n4. POST to configure rate limiter aggressively...")
    test_settings = {
        "waf_enabled": True,
        "rate_limit_enabled": True,
        "rate_limit_max": 2, # max 2 requests per window
        "rate_limit_window": 10
    }
    status, _, body = make_request(
        base_url + '/api/settings/security',
        method='POST',
        data=test_settings,
        headers=admin_headers
    )
    print(f"Status: {status}, Body: {body}")
    data = json.loads(body)
    if status != 200 or not data.get("success"):
        print("❌ FAILED: POST /api/settings/security failed")
        sys.exit(1)
    print("✅ WAF & Rate Limiter configured successfully")

    # Make rapid requests to trigger rate limiting
    print("\n5. Testing rate limiter triggers (making 4 requests to dashboard)...")
    blocked_at_least_once = False
    for i in range(1, 5):
        status, _, body = make_request(base_url + '/api/settings/security', headers=admin_headers)
        print(f"Request {i} status: {status}")
        if status == 429:
            print(f"✅ Rate limiter triggered on request {i}: 429 Too Many Requests")
            blocked_at_least_once = True
            break
        time.sleep(0.1)

    if not blocked_at_least_once:
        print("❌ FAILED: Rate Limiter did not trigger block as expected")
        sys.exit(1)

    # Let's wait 11 seconds for the rate limiter window to clear
    print("Waiting 11 seconds for the rate limit window to clear...")
    time.sleep(11.0)

    # Retrieve security settings again (which includes waf audit logs)
    print("\n6. Checking WAF/Rate Limiting audit logs in DB via API...")
    status, _, body = make_request(base_url + '/api/settings/security', headers=admin_headers)
    print(f"Status: {status}")
    data = json.loads(body)
    logs = data.get("logs", [])
    print(f"Retrieved {len(logs)} security log(s)")
    if len(logs) == 0:
        print("❌ FAILED: Audit logs not populated in DB")
        sys.exit(1)
    
    found_block = False
    for log in logs:
        print(f" - Log ID: {log.get('id')}, IP: {log.get('ip')}, Reason: {log.get('reason')}, Target: {log.get('target')}")
        if "rate limit" in log.get("reason", "").lower() or "too many requests" in log.get("reason", "").lower():
            found_block = True
    
    if not found_block:
        print("❌ FAILED: Rate limit block not logged in audit trail")
        sys.exit(1)
    print("✅ Audit log verification passed")

    # Check structured JSON access log file
    print("\n7. Verifying structured access logs in logs/access.log...")
    if not os.path.exists("logs/access.log"):
        print("❌ FAILED: logs/access.log file does not exist")
        sys.exit(1)
    
    with open("logs/access.log", "r") as f:
        log_lines = f.readlines()
        
    print(f"Read {len(log_lines)} lines from logs/access.log. Printing last line:")
    if len(log_lines) > 0:
        last_line = log_lines[-1].strip()
        print(f" - {last_line}")
        # Try parsing JSON to ensure it is valid JSON
        try:
            parsed = json.loads(last_line)
            if "ip" in parsed and "latency_ms" in parsed and "status" in parsed:
                print("✅ Access log entry is valid structured JSON")
            else:
                print("❌ FAILED: JSON structure missing keys")
                sys.exit(1)
        except Exception as e:
            print(f"❌ FAILED: Access log is not valid JSON: {e}")
            sys.exit(1)
    else:
        print("❌ FAILED: Access log is empty")
        sys.exit(1)

    # Check Traffic Analytics Endpoint
    print("\n8. Checking traffic stats analytics history endpoint...")
    status, _, body = make_request(base_url + '/api/settings/analytics', headers=admin_headers)
    print(f"Status: {status}")
    data = json.loads(body)
    if status != 200 or not data.get("success") or "history" not in data:
        print("❌ FAILED: GET /api/settings/analytics failed")
        sys.exit(1)
    
    history = data["history"]
    print(f"Retrieved {len(history)} telemetry data points")
    if len(history) > 0:
        print("Sample telemetry point:", history[-1])
        print("✅ Telemetry endpoints and state verified successfully")
    else:
        print("⚠️ Warning: Telemetry history is empty (ticker might not have ticked yet)")

    # Wait another 11 seconds to clear rate limit before POST restore settings
    print("Waiting 11 seconds to clear rate limit window before restoring settings...")
    time.sleep(11.0)

    # Restore original settings
    print("\n9. Restoring original security settings...")
    status, _, body = make_request(
        base_url + '/api/settings/security',
        method='POST',
        data=orig_settings,
        headers=admin_headers
    )
    print(f"Status: {status}, Body: {body}")
    data = json.loads(body)
    if status != 200 or not data.get("success"):
        print("❌ FAILED: Restoring original settings failed")
        sys.exit(1)
    print("✅ Successfully cleaned up settings. Integration Test PASSED!")

if __name__ == '__main__':
    main()
