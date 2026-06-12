import urllib.request
import urllib.parse
import json
import sys
import os

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
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    try:
        res = urllib.request.urlopen(req)
        body = res.read().decode('utf-8')
        return res.getcode(), res.info(), body
    except urllib.error.HTTPError as e:
        body = e.read().decode('utf-8')
        return e.code, e.info(), body

def make_multipart_body(fields, files):
    boundary = b'----WebKitFormBoundary7MA4YWxkTrZu0gW'
    body = []
    
    for key, value in fields.items():
        body.append(b'--' + boundary)
        body.append(f'Content-Disposition: form-data; name="{key}"'.encode('utf-8'))
        body.append(b'')
        body.append(value.encode('utf-8'))
        
    for key, filename, content in files:
        body.append(b'--' + boundary)
        body.append(f'Content-Disposition: form-data; name="{key}"; filename="{filename}"'.encode('utf-8'))
        body.append(b'Content-Type: application/octet-stream')
        body.append(b'')
        body.append(content)
        
    body.append(b'--' + boundary + b'--')
    body.append(b'')
    
    return b'\r\n'.join(body), b'multipart/form-data; boundary=' + boundary

def main():
    print("=== STARTING FILE UPLOAD INTEGRATION TEST ===")
    
    opener = urllib.request.build_opener(NoRedirectHandler())
    urllib.request.install_opener(opener)

    base_url = 'http://127.0.0.1:3001'

    # Step 1: GET /zpanel to retrieve CSRF token
    print("\n1. Fetching CSRF token from /zpanel...")
    status, info, _ = make_request(base_url + '/zpanel')
    cookie_hdr = info.get('Set-Cookie')
    csrf_token = get_cookie_value(cookie_hdr, '_csrf')
    if not csrf_token:
        print("❌ FAILED: CSRF token not found")
        sys.exit(1)
    print(f"✅ SUCCESS: CSRF token retrieved: {csrf_token}")

    # Step 2: Attempt upload without token -> verify 401
    print("\n2. Attempting upload without authentication...")
    body, content_type = make_multipart_body({"path": "."}, [("file", "test.txt", b"hello")])
    status, _, body_text = make_request(
        base_url + '/api/files/upload',
        method='POST',
        data=body,
        headers={
            'Content-Type': content_type.decode('utf-8'),
            'X-CSRF-Token': csrf_token,
            'Cookie': f'_csrf={csrf_token}'
        }
    )
    print(f"Status: {status}, Body: {body_text}")
    if status != 401:
        print("❌ FAILED: Non-authenticated user not denied with 401")
        sys.exit(1)
    print("✅ SUCCESS: Upload denied with 401 Unauthorized")

    # Step 3: Login as default admin
    print("\n3. Logging in as admin...")
    login_payload = json.dumps({"username": "admin", "password": "admin"}).encode('utf-8')
    status, info, body_text = make_request(
        base_url + '/zpanel',
        method='POST',
        data=login_payload,
        headers={
            'Content-Type': 'application/json',
            'X-CSRF-Token': csrf_token,
            'Cookie': f'_csrf={csrf_token}'
        }
    )
    set_cookies = info.get_all('Set-Cookie')
    admin_token = get_cookie_value(set_cookies, 'zeno_token')
    if not admin_token:
        print("❌ FAILED: Admin login failed")
        sys.exit(1)
    print("✅ SUCCESS: Logged in as Admin")

    admin_headers = {
        'Cookie': f'zeno_token={admin_token}; _csrf={csrf_token}',
        'X-CSRF-Token': csrf_token
    }

    # Create a test directory for upload
    test_dir = os.path.abspath("./test_upload_destination")
    if not os.path.exists(test_dir):
        os.makedirs(test_dir)

    # Step 4: Upload file to test_dir as admin
    print(f"\n4. Uploading valid file to '{test_dir}' as Admin...")
    test_filename = "valid_test.txt"
    test_content = b"Upload testing content inside new ZenoLang slot!"
    
    body, content_type = make_multipart_body({"path": test_dir}, [("file", test_filename, test_content)])
    status, _, body_text = make_request(
        base_url + '/api/files/upload',
        method='POST',
        data=body,
        headers={
            'Content-Type': content_type.decode('utf-8'),
            'Cookie': f'zeno_token={admin_token}; _csrf={csrf_token}',
            'X-CSRF-Token': csrf_token
        }
    )
    print(f"Status: {status}, Body: {body_text}")
    if status != 200 or not json.loads(body_text).get("success"):
        print("❌ FAILED: File upload failed")
        sys.exit(1)
    
    # Check if file is written
    target_filepath = os.path.join(test_dir, test_filename)
    if not os.path.exists(target_filepath):
        print("❌ FAILED: Uploaded file does not exist on disk")
        sys.exit(1)
    with open(target_filepath, 'rb') as f:
        read_content = f.read()
    if read_content != test_content:
        print("❌ FAILED: Uploaded file content mismatch")
        sys.exit(1)
    print("✅ SUCCESS: File uploaded successfully and verified on disk")

    # Step 5: Test Path Traversal Protection
    print("\n5. Testing Path Traversal Protection...")
    traversal_filename = "../outside_test.txt"
    body, content_type = make_multipart_body({"path": test_dir}, [("file", traversal_filename, b"hack")])
    status, _, body_text = make_request(
        base_url + '/api/files/upload',
        method='POST',
        data=body,
        headers={
            'Content-Type': content_type.decode('utf-8'),
            'Cookie': f'zeno_token={admin_token}; _csrf={csrf_token}',
            'X-CSRF-Token': csrf_token
        }
    )
    print(f"Status: {status}, Body: {body_text}")
    # The parent check rejects filename if it has '/' or '..' because of safe name parsing:
    # safe_name = Path(filename).file_name()... which extracts only the basename.
    # If it is converted to 'outside_test.txt', it writes inside the destination directory, which is safe.
    # If the path traversal still bypasses it, the canonicalization check denies it.
    # In both cases, the parent/canonicalization check ensures we never write outside test_dir.
    outside_filepath = os.path.abspath(os.path.join(test_dir, "..", "outside_test.txt"))
    if os.path.exists(outside_filepath):
        print("❌ FAILED: Path traversal succeeded and wrote outside the target folder")
        sys.exit(1)
    print("✅ SUCCESS: Path traversal thwarted")

    # Step 6: Create viewer user, log in, and try to upload -> verify 403 Forbidden
    print("\n6. Creating viewer user...")
    viewer_payload = json.dumps({
        "username": "viewer_upload_test",
        "password_plain": "viewer123",
        "role": "viewer"
    }).encode('utf-8')
    status, info, body_text = make_request(
        base_url + '/api/users/create',
        method='POST',
        data=viewer_payload,
        headers={
            'Content-Type': 'application/json',
            'Cookie': f'zeno_token={admin_token}; _csrf={csrf_token}',
            'X-CSRF-Token': csrf_token
        }
    )
    if status != 200:
        print("❌ FAILED: Could not create viewer user")
        sys.exit(1)
        
    print("Logging in as viewer...")
    login_payload = json.dumps({"username": "viewer_upload_test", "password": "viewer123"}).encode('utf-8')
    status, info, body_text = make_request(
        base_url + '/zpanel',
        method='POST',
        data=login_payload,
        headers={
            'Content-Type': 'application/json',
            'X-CSRF-Token': csrf_token,
            'Cookie': f'_csrf={csrf_token}'
        }
    )
    set_cookies = info.get_all('Set-Cookie')
    viewer_token = get_cookie_value(set_cookies, 'zeno_token')
    
    print("Attempting upload as Viewer (expecting 403)...")
    body, content_type = make_multipart_body({"path": test_dir}, [("file", "viewer_upload.txt", b"viewer")])
    status, _, body_text = make_request(
        base_url + '/api/files/upload',
        method='POST',
        data=body,
        headers={
            'Content-Type': content_type.decode('utf-8'),
            'Cookie': f'zeno_token={viewer_token}; _csrf={csrf_token}',
            'X-CSRF-Token': csrf_token
        }
    )
    print(f"Status: {status}, Body: {body_text}")
    if status != 403:
        print("❌ FAILED: Viewer was not denied upload access with 403")
        sys.exit(1)
    print("✅ SUCCESS: Viewer upload denied with 403 Forbidden")

    # Step 7: Clean up
    print("\n7. Cleaning up test user and directory...")
    # Delete test user
    delete_payload = json.dumps({"username": "viewer_upload_test"}).encode('utf-8')
    make_request(
        base_url + '/api/users/delete',
        method='POST',
        data=delete_payload,
        headers={
            'Content-Type': 'application/json',
            'Cookie': f'zeno_token={admin_token}; _csrf={csrf_token}',
            'X-CSRF-Token': csrf_token
        }
    )
    # Delete files
    if os.path.exists(target_filepath):
        os.remove(target_filepath)
    traversal_written = os.path.join(test_dir, "outside_test.txt")
    if os.path.exists(traversal_written):
        os.remove(traversal_written)
    if os.path.exists(test_dir):
        os.rmdir(test_dir)
        
    print("\n=== ALL FILE UPLOAD AND INTEGRATION SCENARIOS VERIFIED SUCCESSFULLY ===")

if __name__ == '__main__':
    main()
