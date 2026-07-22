import urllib.request
import urllib.parse
import json
import sys
import os
import time

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

def main():
    print("=== STARTING ISO LIBRARY INTEGRATION TEST ===")
    
    opener = urllib.request.build_opener(NoRedirectHandler())
    urllib.request.install_opener(opener)

    base_url = 'http://127.0.0.1:8080'

    # Step 1: Detect entrance path and retrieve CSRF token
    print("\n1. Detecting entrance path and retrieving CSRF token...")
    entrance_path = '/zpanel'
    status, info, body = make_request(base_url + '/zpanel')
    if status == 404 or "Halaman tidak ditemukan." in body:
        print("GET /zpanel returned 404, trying /login...")
        entrance_path = '/login'
        status, info, body = make_request(base_url + '/login')
        if status == 404 or "Halaman tidak ditemukan." in body:
            print("❌ FAILED: Could not find login entrance path")
            sys.exit(1)

    cookie_hdr = info.get('Set-Cookie')
    csrf_token = get_cookie_value(cookie_hdr, '_csrf')
    if not csrf_token:
        print("❌ FAILED: CSRF token not found")
        sys.exit(1)
    print(f"✅ SUCCESS: Using entrance path: {entrance_path}")
    print(f"✅ SUCCESS: CSRF token retrieved: {csrf_token}")

    # Step 2: Login as default admin
    print("\n2. Logging in as admin...")
    login_payload = json.dumps({"username": "admin", "password": "admin"}).encode('utf-8')
    status, info, body_text = make_request(
        base_url + entrance_path,
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

    headers = {
        'Cookie': f'zeno_token={admin_token}; _csrf={csrf_token}',
        'X-CSRF-Token': csrf_token,
        'Content-Type': 'application/json'
    }

    # Step 3: Register ISO with URL
    print("\n3. Registering ISO with Download URL...")
    iso_name = "test-alpine.iso"
    # Small virtuall/virt alpine ISO for fast download
    download_url = "https://dl-cdn.alpinelinux.org/alpine/v3.19/releases/x86_64/alpine-virt-3.19.1-x86_64.iso"
    local_path = f"/var/lib/zeno-container/isos/{iso_name}"
    
    add_payload = json.dumps({
        "name": iso_name,
        "source_url": download_url,
        "path": local_path
    }).encode('utf-8')
    
    status, _, body_text = make_request(
        base_url + '/api/machines/isos/add',
        method='POST',
        data=add_payload,
        headers=headers
    )
    
    print(f"Add ISO response status: {status}, body: {body_text}")
    res_data = json.loads(body_text)
    if status != 201 or not res_data.get("success"):
        print("❌ FAILED: Registering ISO failed")
        sys.exit(1)
    print("✅ SUCCESS: ISO registered successfully")

    # Step 4: Monitor download status
    print("\n4. Monitoring download status...")
    iso_id = None
    download_complete = False
    
    for i in range(30): # Wait up to 150 seconds
        time.sleep(5)
        status, _, body_text = make_request(
            base_url + '/api/machines/isos/list',
            method='GET',
            headers=headers
        )
        res_data = json.loads(body_text)
        isos = res_data.get("data", [])
        target_iso = None
        for iso in isos:
            if iso.get("name") == iso_name:
                target_iso = iso
                break
                
        if target_iso:
            iso_id = target_iso.get("id")
            current_status = target_iso.get("status")
            size = target_iso.get("size_bytes")
            print(f"[{i+1}/30] ISO status: '{current_status}', size: {size} bytes")
            if current_status == "ready":
                download_complete = True
                break
            elif current_status == "error":
                print("❌ FAILED: ISO download failed with status 'error'")
                sys.exit(1)
        else:
            print("❌ FAILED: ISO not found in list")
            sys.exit(1)
            
    if not download_complete:
        print("❌ FAILED: ISO download timed out")
        sys.exit(1)
    print("✅ SUCCESS: ISO download completed and status is 'ready'")

    # Step 5: Test Attaching to Zeno Machine
    print("\n5. Creating Zeno Machine to test attachment...")
    vm_name = "test-vm-iso"
    create_payload = json.dumps({
        "name": vm_name,
        "os_type": "linux",
        "vcpus": 2,
        "memory_mb": 1024,
        "disk_size_gb": 10,
        "iso_path": local_path
    }).encode('utf-8')
    
    status, _, body_text = make_request(
        base_url + '/api/machines/create',
        method='POST',
        data=create_payload,
        headers=headers
    )
    print(f"Create Machine response: {body_text}")
    
    # Step 6: Verify deletion is blocked while attached
    print("\n6. Trying to delete ISO while attached to machine...")
    delete_payload = json.dumps({"id": iso_id}).encode('utf-8')
    status, _, body_text = make_request(
        base_url + '/api/machines/isos/delete',
        method='POST',
        data=delete_payload,
        headers=headers
    )
    print(f"Delete ISO response (should fail): {body_text}")
    res_data = json.loads(body_text)
    if status == 200 and res_data.get("success"):
        print("❌ FAILED: ISO deletion was not blocked while attached")
        sys.exit(1)
    print("✅ SUCCESS: ISO deletion blocked correctly")

    # Step 7: Detach ISO and delete ISO
    print("\n7. Detaching ISO and deleting Zeno Machine & ISO...")
    # Detach ISO
    detach_payload = json.dumps({"name": vm_name}).encode('utf-8')
    status, _, body_text = make_request(
        base_url + '/api/machines/isos/detach',
        method='POST',
        data=detach_payload,
        headers=headers
    )
    print(f"Detach ISO response: {body_text}")
    
    # Delete machine
    delete_vm_payload = json.dumps({"name": vm_name}).encode('utf-8')
    make_request(
        base_url + '/api/machines/delete',
        method='POST',
        data=delete_vm_payload,
        headers=headers
    )
    
    # Now try to delete ISO (should succeed)
    status, _, body_text = make_request(
        base_url + '/api/machines/isos/delete',
        method='POST',
        data=delete_payload,
        headers=headers
    )
    print(f"Delete ISO response: {body_text}")
    res_data = json.loads(body_text)
    if status != 200 or not res_data.get("success"):
        print("❌ FAILED: Deletion failed after detaching")
        sys.exit(1)
    print("✅ SUCCESS: ISO deleted from database")
    
    # Verify file is deleted from disk
    if os.path.exists(local_path):
        print("❌ FAILED: ISO file still exists on disk after deletion")
        sys.exit(1)
    print("✅ SUCCESS: ISO file deleted from disk")
    print("\n=== ALL ISO LIBRARY SCENARIOS VERIFIED SUCCESSFULLY ===")

if __name__ == '__main__':
    main()
