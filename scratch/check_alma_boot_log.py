import urllib.request
import urllib.parse
import json
import sqlite3
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
    for cookie in cookie_header.split(','):
        parts = cookie.strip().split(';')[0].split('=')
        if len(parts) == 2 and parts[0].strip() == name:
            return parts[1].strip()
    return None

def main():
    base_url = 'http://127.0.0.1:8080'
    opener = urllib.request.build_opener(NoRedirectHandler())
    urllib.request.install_opener(opener)

    conn = sqlite3.connect('dist/zenopanel-v1.7.14/zeno.db')
    cursor = conn.cursor()
    cursor.execute("SELECT value FROM settings WHERE key = 'entrance_path'")
    row = cursor.fetchone()
    entrance_path = row[0] if row else '/login'
    if not entrance_path.startswith('/'):
        entrance_path = '/' + entrance_path
    conn.close()
    
    # 1. Access login page to get CSRF
    req = urllib.request.Request(base_url + entrance_path)
    res = urllib.request.urlopen(req)
    cookie_hdr = res.info().get('Set-Cookie')
    csrf_token = get_cookie_value(cookie_hdr, '_csrf')
    
    # 2. Login
    login_data = json.dumps({"username": "admin", "password": "admin"}).encode('utf-8')
    headers = {
        'Content-Type': 'application/json',
        'X-CSRF-Token': csrf_token,
        'Cookie': f'_csrf={csrf_token}'
    }
    req = urllib.request.Request(base_url + entrance_path, data=login_data, headers=headers)
    res = urllib.request.urlopen(req)
    cookie_header = res.info().get('Set-Cookie')
    zeno_token = get_cookie_value(cookie_header, 'zeno_token')
    
    session_cookie = f'zeno_token={zeno_token}; _csrf={csrf_token}'
    api_headers = {
        'Content-Type': 'application/json',
        'X-CSRF-Token': csrf_token,
        'Cookie': session_cookie
    }
    
    vm_name = "test-alma-3gb"
    print(f"\n1. Creating OCI VM '{vm_name}'...")
    create_payload = json.dumps({
        "name": vm_name,
        "os_type": "linux",
        "vcpus": 1,
        "memory_mb": 3072,
        "disk_size_gb": 2,
        "iso_path": "oci://library/almalinux:10-kitten",
        "ssh_key": "",
        "root_password": ""
    }).encode('utf-8')
    
    req = urllib.request.Request(base_url + '/api/machines/create', data=create_payload, headers=api_headers)
    try:
        res = urllib.request.urlopen(req)
        print("Create response:", res.read().decode('utf-8'))
    except Exception as e:
        print("Create failed:", e)
        return

    print(f"\n2. Starting OCI VM '{vm_name}'...")
    start_payload = json.dumps({"name": vm_name}).encode('utf-8')
    req = urllib.request.Request(base_url + '/api/machines/start', data=start_payload, headers=api_headers)
    try:
        res = urllib.request.urlopen(req)
        print("Start response:", res.read().decode('utf-8'))
    except Exception as e:
        print("Start failed:", e)
        
    print("\nWaiting 12 seconds for VM boot logs to populate...")
    time.sleep(12)
    
    print("\n3. Fetching VM Boot Logs...")
    req = urllib.request.Request(base_url + f'/api/machines/logs/boot?name={vm_name}', headers=api_headers)
    try:
        res = urllib.request.urlopen(req)
        logs = res.read().decode('utf-8')
        print("--- BOOT LOGS ---")
        print(logs)
        print("-----------------")
    except Exception as e:
        print("Failed to fetch boot logs:", e)
        
    print(f"\n4. Cleaning up VM '{vm_name}'...")
    delete_payload = json.dumps({"name": vm_name}).encode('utf-8')
    req = urllib.request.Request(base_url + '/api/machines/delete', data=delete_payload, headers=api_headers)
    res = urllib.request.urlopen(req)
    print("Delete response:", res.read().decode('utf-8'))

if __name__ == '__main__':
    main()
