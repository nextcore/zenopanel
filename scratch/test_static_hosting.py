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
    print(f"\n2. Logging in as admin...")
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

    # Setup temporary static directory
    static_dir = os.path.abspath('scratch/temp_static_site')
    os.makedirs(static_dir, exist_ok=True)
    os.makedirs(os.path.join(static_dir, 'nested'), exist_ok=True)

    with open(os.path.join(static_dir, 'index.html'), 'w') as f:
        f.write("<h1>Hello from Zeno Static Site</h1>")
    
    with open(os.path.join(static_dir, 'nested', 'page.html'), 'w') as f:
        f.write("<h2>Nested Page</h2>")

    print(f"\n3. Temporary static site created at: {static_dir}")

    # Add static site rule
    print("\n4. Adding static site hosting rule...")
    rule_payload = {
        "name": "Integration Test Static Site",
        "domain": "*",
        "alternative_domain": "",
        "path": "/teststatic",
        "target": static_dir,
        "strip_path": True,
        "enabled": True,
        "ssl_enabled": False,
        "managed_process_id": "",
        "rule_type": "static"
    }

    status, _, body = make_request(
        base_url + '/api/proxy/add',
        method='POST',
        data=rule_payload,
        headers=admin_headers
    )
    print(f"Status: {status}, Response: {body}")
    res_data = json.loads(body)
    if status != 200 or not res_data.get("success"):
        print("❌ FAILED: Could not add static site rule")
        sys.exit(1)
    
    rule_id = res_data["id"]
    print(f"✅ Rule added successfully with ID: {rule_id}")

    # Test serving files E2E
    # Wait briefly for in-memory sync if needed (though it is immediate)
    time.sleep(0.5)

    print("\n5. Verifying root index.html E2E...")
    status, _, body = make_request(base_url + '/teststatic/index.html')
    print(f"Status: {status}, Body: {body}")
    if status != 200 or "Hello from Zeno Static Site" not in body:
        print("❌ FAILED: Root index.html was not served correctly")
        sys.exit(1)
    print("✅ index.html served correctly")

    print("\n6. Verifying nested page.html E2E...")
    status, _, body = make_request(base_url + '/teststatic/nested/page.html')
    print(f"Status: {status}, Body: {body}")
    if status != 200 or "Nested Page" not in body:
        print("❌ FAILED: Nested file page.html was not served correctly")
        sys.exit(1)
    print("✅ nested/page.html served correctly")

    print("\n7. Verifying Single Page App (SPA) fallback E2E...")
    # Any route that doesn't exist should fall back to index.html
    status, _, body = make_request(base_url + '/teststatic/some/random/route')
    print(f"Status: {status}, Body: {body}")
    if status != 200 or "Hello from Zeno Static Site" not in body:
        print("❌ FAILED: SPA fallback to index.html did not work")
        sys.exit(1)
    print("✅ SPA fallback worked correctly")

    # Clean up rule
    print("\n8. Deleting test static site rule...")
    status, _, body = make_request(
        base_url + '/api/proxy/delete',
        method='POST',
        data={"id": rule_id},
        headers=admin_headers
    )
    print(f"Status: {status}, Response: {body}")
    
    # Clean up files
    import shutil
    shutil.rmtree(static_dir, ignore_errors=True)
    print("✅ Cleaned up temporary files")
    print("\n🎉 ALL STATIC SITE HOSTING INTEGRATION TESTS PASSED SUCCESSFULLY!")

if __name__ == '__main__':
    main()
