import urllib.request
import urllib.parse
import json
import sys

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
    base_url = 'http://127.0.0.1:3000'

    print("0. Testing access to root / without credentials (should be 404/not found)...")
    status, _, _ = make_request(base_url + '/')
    print(f"Status: {status}")
    if status != 404:
        print("❌ FAILED: Root path allowed or redirected unauthenticated user!")
        sys.exit(1)
    print("✅ Unauthenticated access to root returns 404 correctly")

    print("\n1. Fetching CSRF token from /login...")
    status, info, _ = make_request(base_url + '/login')
    cookie_hdr = info.get('Set-Cookie')
    csrf_token = get_cookie_value(cookie_hdr, '_csrf')
    if not csrf_token:
        print("❌ FAILED: CSRF token not found")
        sys.exit(1)
    print(f"✅ CSRF token: {csrf_token}")

    print("\n2. Logging in as admin...")
    status, info, body = make_request(
        base_url + '/login',
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

    print("\n3. Testing GET /api/settings...")
    status, info, body = make_request(base_url + '/api/settings', headers=admin_headers)
    print(f"Status: {status}, Body: {body}")
    data = json.loads(body)
    if status != 200 or not data.get("success") or data.get("entrance_path") != "/login":
        print("❌ FAILED: GET /api/settings returned incorrect data")
        sys.exit(1)
    print("✅ GET settings works")

    print("\n4. Testing POST /api/settings to change entrance path to /test-login...")
    status, info, body = make_request(
        base_url + '/api/settings',
        method='POST',
        data={"entrance_path": "/test-login"},
        headers=admin_headers
    )
    print(f"Status: {status}, Body: {body}")
    data = json.loads(body)
    if status != 200 or not data.get("success") or data.get("entrance_path") != "/test-login":
        print("❌ FAILED: POST /api/settings failed")
        sys.exit(1)
    print("✅ POST settings works")

    print("\n5. Testing access to old entrance path /login (should be 404/not found)...")
    status, info, body = make_request(base_url + '/login')
    print(f"Status: {status}")
    if status != 404:
        print("❌ FAILED: Old entrance path is still active!")
        sys.exit(1)
    print("✅ Old entrance path is correctly deactivated")

    print("\n6. Testing access to new entrance path /test-login (should be 200)...")
    status, info, body = make_request(base_url + '/test-login')
    print(f"Status: {status}")
    if status != 200 or "username" not in body:
        print("❌ FAILED: New entrance path not serving login page!")
        sys.exit(1)
    print("✅ New entrance path works and serves login page")

    print("\n7. Restoring entrance path back to /login...")
    status, info, body = make_request(
        base_url + '/api/settings',
        method='POST',
        data={"entrance_path": "/login"},
        headers=admin_headers
    )
    print(f"Status: {status}, Body: {body}")
    print("✅ Successfully restored settings to original state")

if __name__ == '__main__':
    main()
