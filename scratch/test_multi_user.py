import urllib.request
import urllib.parse
import json
import sys

class NoRedirectHandler(urllib.request.HTTPRedirectHandler):
    def http_error_302(self, req, fp, code, msg, headers):
        return fp
    def http_error_303(self, req, fp, code, msg, headers):
        return fp
    def http_error_307(self, req, fp, code, msg, headers):
        return fp

def get_cookie_value(cookie_header, name):
    if not cookie_header:
        return None
    # urllib might join multiple Set-Cookie headers with commas, or return a list if using info().get_all()
    # Let's support both list and comma-separated string formats
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
    print("=== STARTING MULTI-USER & RBAC VERIFICATION SUITE ===")
    
    # Configure urllib to not follow redirects
    opener = urllib.request.build_opener(NoRedirectHandler())
    urllib.request.install_opener(opener)

    base_url = 'http://127.0.0.1:3001'

    # Step 1: GET /zpanel to retrieve CSRF token
    print("\n1. Fetching CSRF token from /zpanel...")
    status, info, _ = make_request(base_url + '/zpanel')
    cookie_hdr = info.get('Set-Cookie')
    csrf_token = get_cookie_value(cookie_hdr, '_csrf')
    if not csrf_token:
        print("❌ FAILED: CSRF token not found in cookies")
        sys.exit(1)
    print(f"✅ SUCCESS: CSRF token retrieved: {csrf_token}")

    # Step 2: Login as default admin
    print("\n2. Logging in as default admin...")
    login_payload = {"username": "admin", "password": "admin"}
    status, info, body = make_request(
        base_url + '/zpanel',
        method='POST',
        data=login_payload,
        headers={
            'X-CSRF-Token': csrf_token,
            'Cookie': f'_csrf={csrf_token}'
        }
    )
    
    print(f"Response Status: {status}")
    print(f"Response Body: {body}")
    set_cookies = info.get_all('Set-Cookie')
    admin_token = get_cookie_value(set_cookies, 'zeno_token')
    admin_role = get_cookie_value(set_cookies, 'zeno_role')
    
    print(f"Admin Token: {admin_token[:15]}...")
    print(f"Admin Role Cookie: {admin_role}")
    
    if status != 200 or admin_role != 'admin' or not admin_token:
        print("❌ FAILED: Admin login failed or role cookie mismatch")
        sys.exit(1)
    print("✅ SUCCESS: Logged in as Admin")

    admin_headers = {
        'Cookie': f'zeno_token={admin_token}; _csrf={csrf_token}',
        'X-CSRF-Token': csrf_token
    }

    # Step 3: GET /api/users/list as admin
    print("\n3. Listing users as admin...")
    status, info, body = make_request(base_url + '/api/users/list', headers=admin_headers)
    print(f"Status: {status}, Body: {body}")
    if status != 200:
        print("❌ FAILED: Could not list users as admin")
        sys.exit(1)
    users_list = json.loads(body)
    print(f"✅ SUCCESS: Retrieved {len(users_list)} users")

    # Step 4: Create editor user as admin
    print("\n4. Creating editor user (editor_test)...")
    editor_payload = {
        "username": "editor_test",
        "password_plain": "editor123",
        "role": "editor"
    }
    status, info, body = make_request(
        base_url + '/api/users/create',
        method='POST',
        data=editor_payload,
        headers=admin_headers
    )
    print(f"Status: {status}, Body: {body}")
    if status != 200 or not json.loads(body).get("success"):
        print("❌ FAILED: Could not create editor user")
        sys.exit(1)
    print("✅ SUCCESS: Editor user created")

    # Step 5: Create viewer user as admin
    print("\n5. Creating viewer user (viewer_test)...")
    viewer_payload = {
        "username": "viewer_test",
        "password_plain": "viewer123",
        "role": "viewer"
    }
    status, info, body = make_request(
        base_url + '/api/users/create',
        method='POST',
        data=viewer_payload,
        headers=admin_headers
    )
    print(f"Status: {status}, Body: {body}")
    if status != 200 or not json.loads(body).get("success"):
        print("❌ FAILED: Could not create viewer user")
        sys.exit(1)
    print("✅ SUCCESS: Viewer user created")

    # Step 6: Log in as editor_test
    print("\n6. Logging in as editor_test...")
    status, info, body = make_request(
        base_url + '/zpanel',
        method='POST',
        data={"username": "editor_test", "password": "editor123"},
        headers={
            'X-CSRF-Token': csrf_token,
            'Cookie': f'_csrf={csrf_token}'
        }
    )
    print(f"Status: {status}, Body: {body}")
    set_cookies = info.get_all('Set-Cookie')
    editor_token = get_cookie_value(set_cookies, 'zeno_token')
    editor_role = get_cookie_value(set_cookies, 'zeno_role')
    print(f"Editor Role Cookie: {editor_role}")
    
    if status != 200 or editor_role != 'editor' or not editor_token:
        print("❌ FAILED: Editor login failed or role cookie mismatch")
        sys.exit(1)
    print("✅ SUCCESS: Logged in as Editor")

    editor_headers = {
        'Cookie': f'zeno_token={editor_token}; _csrf={csrf_token}',
        'X-CSRF-Token': csrf_token
    }

    # Step 7: Access /api/users/list as editor (Should be 403)
    print("\n7. Accessing users list as Editor (expecting 403)...")
    status, info, body = make_request(base_url + '/api/users/list', headers=editor_headers)
    print(f"Status: {status}, Body: {body}")
    if status != 403:
        print("❌ FAILED: Editor was not denied access to user list (expected 403)")
        sys.exit(1)
    print("✅ SUCCESS: Editor access correctly denied with 403")

    # Step 8: Access /api/database/query as editor (Should be 403)
    print("\n8. Accessing database console as Editor (expecting 403)...")
    status, info, body = make_request(base_url + '/api/database/query', headers=editor_headers)
    print(f"Status: {status}, Body: {body}")
    if status != 403:
        print("❌ FAILED: Editor was not denied access to database console (expected 403)")
        sys.exit(1)
    print("✅ SUCCESS: Editor database console access correctly denied with 403")

    # Step 9: Log in as viewer_test
    print("\n9. Logging in as viewer_test...")
    status, info, body = make_request(
        base_url + '/zpanel',
        method='POST',
        data={"username": "viewer_test", "password": "viewer123"},
        headers={
            'X-CSRF-Token': csrf_token,
            'Cookie': f'_csrf={csrf_token}'
        }
    )
    print(f"Status: {status}, Body: {body}")
    set_cookies = info.get_all('Set-Cookie')
    viewer_token = get_cookie_value(set_cookies, 'zeno_token')
    viewer_role = get_cookie_value(set_cookies, 'zeno_role')
    print(f"Viewer Role Cookie: {viewer_role}")
    
    if status != 200 or viewer_role != 'viewer' or not viewer_token:
        print("❌ FAILED: Viewer login failed or role cookie mismatch")
        sys.exit(1)
    print("✅ SUCCESS: Logged in as Viewer")

    viewer_headers = {
        'Cookie': f'zeno_token={viewer_token}; _csrf={csrf_token}',
        'X-CSRF-Token': csrf_token
    }

    # Step 10: Access GET stats as viewer (Should be 200)
    print("\n10. Accessing GET /api/stats as Viewer...")
    status, info, body = make_request(base_url + '/api/stats', headers=viewer_headers)
    print(f"Status: {status}")
    if status != 200:
        print("❌ FAILED: Viewer was denied read-only stats access")
        sys.exit(1)
    print("✅ SUCCESS: Viewer stats access allowed")

    # Step 11: Attempt mutation (POST /api/proxy/add) as viewer (Should be 403)
    print("\n11. Attempting proxy add mutation as Viewer (expecting 403)...")
    status, info, body = make_request(
        base_url + '/api/proxy/add',
        method='POST',
        data={"name": "test"},
        headers=viewer_headers
    )
    print(f"Status: {status}, Body: {body}")
    if status != 403:
        print("❌ FAILED: Viewer mutation allowed (expected 403)")
        sys.exit(1)
    print("✅ SUCCESS: Viewer mutation denied with 403")

    # Step 12: Clean up users as admin
    print("\n12. Deleting editor_test and viewer_test users as admin...")
    for user_to_delete in ["editor_test", "viewer_test"]:
        status, info, body = make_request(
            base_url + '/api/users/delete',
            method='POST',
            data={"username": user_to_delete},
            headers=admin_headers
        )
        print(f"Delete {user_to_delete} - Status: {status}, Body: {body}")
        if status != 200 or not json.loads(body).get("success"):
            print(f"❌ FAILED: Could not delete {user_to_delete}")
            sys.exit(1)
    print("✅ SUCCESS: Created test users deleted successfully")

    print("\n=== ALL MULTI-USER AND RBAC SCENARIOS VERIFIED SUCCESSFULLY ===")

if __name__ == '__main__':
    main()
