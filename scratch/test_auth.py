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
    for cookie in cookie_header.split(','):
        # Cookie headers can contain multiple cookies or parameters
        parts = cookie.strip().split(';')[0].split('=')
        if len(parts) == 2 and parts[0].strip() == name:
            return parts[1].strip()
    return None

def main():
    print("=== STARTING JWT AUTHENTICATION VERIFICATION ===")
    
    # Configure urllib to not follow redirects automatically so we can check the status code and headers
    opener = urllib.request.build_opener(NoRedirectHandler())
    urllib.request.install_opener(opener)

    base_url = 'http://127.0.0.1:3000'
    
    # 1. Access root "/" and verify 404 Not Found
    print("\n1. Accessing root '/' unauthenticated...")
    try:
        req = urllib.request.Request(base_url + '/')
        res = urllib.request.urlopen(req)
        status = res.getcode()
        print(f"Response code: {status}")
        print("❌ FAILED: Root did not return 404")
        sys.exit(1)
    except urllib.error.HTTPError as e:
        status = e.code
        print(f"Response code: {status} (Expected: 404)")
        if status != 404:
            print("❌ FAILED: Root did not return 404")
            sys.exit(1)
        print("✅ SUCCESS: Root correctly returns 404")

    # 1b. Fetch "/login" page to get the _csrf cookie
    print("\n1b. Accessing '/login' GET to retrieve CSRF token...")
    try:
        req = urllib.request.Request(base_url + '/login')
        res = urllib.request.urlopen(req)
        status = res.getcode()
        cookie_hdr = res.info().get('Set-Cookie')
        csrf_token = get_cookie_value(cookie_hdr, '_csrf')
        print(f"Response code: {status}")
        print(f"Set-Cookie Header: {cookie_hdr}")
        print(f"Extracted CSRF token: {csrf_token}")
        if not csrf_token:
            print("❌ FAILED: Could not retrieve CSRF token from /login")
            sys.exit(1)
        print("✅ SUCCESS: Extracted CSRF token successfully")
    except Exception as e:
        print(f"❌ FAILED: Error accessing /login GET: {e}")
        sys.exit(1)
    
    # 2. Access "/api/stats" without token and verify 401
    print("\n2. Accessing API '/api/stats' without token...")
    try:
        req = urllib.request.Request(base_url + '/api/stats')
        urllib.request.urlopen(req)
        print("❌ FAILED: Accessed API without authorization header")
        sys.exit(1)
    except urllib.error.HTTPError as e:
        print(f"Response code: {e.code} (Expected: 401)")
        if e.code != 401:
            print("❌ FAILED: Code is not 401")
            sys.exit(1)
        body = json.loads(e.read().decode('utf-8'))
        print("Response body:", body)
        if body.get("success") is not False:
            print("❌ FAILED: success field should be false")
            sys.exit(1)
        print("✅ SUCCESS: Access denied with 401")

    # 3. Post login with incorrect credentials
    print("\n3. Attempting login with invalid credentials...")
    login_data = json.dumps({"username": "admin", "password": "wrong_password"}).encode('utf-8')
    try:
        req = urllib.request.Request(
            base_url + '/login',
            data=login_data,
            headers={
                'Content-Type': 'application/json',
                'X-CSRF-Token': csrf_token or '',
                'Cookie': f'_csrf={csrf_token}'
            }
        )
        urllib.request.urlopen(req)
        print("❌ FAILED: Login succeeded with invalid credentials")
        sys.exit(1)
    except urllib.error.HTTPError as e:
        print(f"Response code: {e.code} (Expected: 401)")
        if e.code != 401:
            print("❌ FAILED: Code is not 401")
            sys.exit(1)
        body = json.loads(e.read().decode('utf-8'))
        print("Response body:", body)
        if body.get("success") is not False:
            print("❌ FAILED: success field should be false")
            sys.exit(1)
        print("✅ SUCCESS: Login failed correctly with 401")

    # 4. Post login with correct credentials
    print("\n4. Attempting login with correct credentials...")
    login_data = json.dumps({"username": "admin", "password": "admin"}).encode('utf-8')
    try:
        headers = {
            'Content-Type': 'application/json',
            'X-CSRF-Token': csrf_token,
            'Cookie': f'_csrf={csrf_token}'
        }
            
        req = urllib.request.Request(
            base_url + '/login',
            data=login_data,
            headers=headers
        )
        res = urllib.request.urlopen(req)
        status = res.getcode()
        print(f"Response code: {status}")
        body = json.loads(res.read().decode('utf-8'))
        print("Response body:", body)
        
        cookie_header = res.info().get('Set-Cookie')
        print(f"Set-Cookie Header: {cookie_header}")
        
        zeno_token = get_cookie_value(cookie_header, 'zeno_token')
        print(f"Extracted JWT token: {zeno_token}")
        
        if status != 200 or not body.get("success") or not zeno_token:
            print("❌ FAILED: Login failed or token not returned")
            sys.exit(1)
        print("✅ SUCCESS: Successfully logged in and received JWT token")
    except Exception as e:
        print(f"❌ FAILED: Error logging in: {e}")
        sys.exit(1)

    # 5. Access "/api/stats" with token and verify 200
    print("\n5. Accessing API '/api/stats' with JWT token...")
    try:
        req = urllib.request.Request(
            base_url + '/api/stats',
            headers={'Cookie': f'zeno_token={zeno_token}'}
        )
        res = urllib.request.urlopen(req)
        status = res.getcode()
        print(f"Response code: {status}")
        body = json.loads(res.read().decode('utf-8'))
        print("Response body type:", type(body))
        if status != 200:
            print("❌ FAILED: API access denied even with valid token")
            sys.exit(1)
        print("✅ SUCCESS: Successfully called protected API with JWT")
    except Exception as e:
        print(f"❌ FAILED: Error accessing protected API: {e}")
        sys.exit(1)

    # 5b. Access "/api/files/upload" without token and verify 401
    print("\n5b. Accessing native upload API '/api/files/upload' without token...")
    try:
        req = urllib.request.Request(base_url + '/api/files/upload', data=b'', method='POST')
        urllib.request.urlopen(req)
        print("❌ FAILED: Accessed native upload API without token")
        sys.exit(1)
    except urllib.error.HTTPError as e:
        print(f"Response code: {e.code} (Expected: 401)")
        if e.code != 401:
            print("❌ FAILED: Code is not 401")
            sys.exit(1)
        print("✅ SUCCESS: Native upload API access denied with 401")

    # 6. Test logout
    print("\n6. Accessing '/logout'...")
    try:
        req = urllib.request.Request(
            base_url + '/logout',
            headers={'Cookie': f'zeno_token={zeno_token}'}
        )
        res = urllib.request.urlopen(req)
        status = res.getcode()
        loc = res.info().get('Location')
        cookie_header = res.info().get('Set-Cookie')
        print(f"Response code: {status}")
        print(f"Redirect Location: {loc}")
        print(f"Set-Cookie: {cookie_header}")
        
        if status != 303 or loc != '/login':
            print("❌ FAILED: Logout did not redirect to '/login'")
            sys.exit(1)
        
        # Verify token cookie is cleared (has Max-Age=0 or empty value)
        cleared_token = get_cookie_value(cookie_header, 'zeno_token')
        if cleared_token and cleared_token != "":
            print("❌ FAILED: Token cookie was not cleared")
            sys.exit(1)
            
        print("✅ SUCCESS: Successfully logged out and token cookie cleared")
    except Exception as e:
        print(f"❌ FAILED: Error during logout: {e}")
        sys.exit(1)

    print("\n=== ALL JWT AUTHENTICATION FLOWS VERIFIED SUCCESSFULLY ===")

if __name__ == '__main__':
    main()
