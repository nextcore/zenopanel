import urllib.request
import urllib.parse
import json
import time
import sys
import os

headers_dict = {}

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

def post_json(url, data):
    hdrs = {'Content-Type': 'application/json'}
    hdrs.update(headers_dict)
    req = urllib.request.Request(
        url,
        data=json.dumps(data).encode('utf-8'),
        headers=hdrs
    )
    try:
        with urllib.request.urlopen(req) as res:
            return json.loads(res.read().decode('utf-8'))
    except Exception as e:
        print(f"Error posting to {url}: {e}")
        if hasattr(e, 'read'):
            print("Response:", e.read().decode('utf-8'))
        raise

def get_json(url):
    try:
        req = urllib.request.Request(url, headers=headers_dict)
        with urllib.request.urlopen(req) as res:
            return json.loads(res.read().decode('utf-8'))
    except Exception as e:
        print(f"Error getting {url}: {e}")
        raise

def main():
    print("=== STARTING ZENOPANEL ALTERNATIVE DOMAIN TEST ===")
    
    # Detect port from env, then .env, or default to 3001/3000
    port_str = os.environ.get('APP_PORT')
    if port_str:
        port = int(port_str.replace(':', ''))
    else:
        port = 3001
        try:
            with open('.env', 'r') as f:
                for line in f:
                    if line.startswith('APP_PORT='):
                        val = line.split('=')[1].strip().replace(':', '')
                        port = int(val)
        except Exception as e:
            print("Could not read port from .env, defaulting to 3001:", e)
        
    base_url = f'http://127.0.0.1:{port}'
    print(f"Using base URL: {base_url}")
    
    # Authenticate first
    print("Authenticating admin...")
    entrance = '/login'
    try:
        req = urllib.request.Request(base_url + '/zpanel')
        with urllib.request.urlopen(req) as res:
            if res.getcode() == 200:
                entrance = '/zpanel'
    except Exception:
        pass
    print(f"Using entrance path: {entrance}")
    try:
        # Get CSRF token
        req = urllib.request.Request(base_url + entrance)
        with urllib.request.urlopen(req) as res:
            cookie_hdr = res.info().get('Set-Cookie')
            csrf_token = get_cookie_value(cookie_hdr, '_csrf')
            
        # Login
        login_data = json.dumps({"username": "admin", "password": "admin"}).encode('utf-8')
        req = urllib.request.Request(
            base_url + entrance,
            data=login_data,
            headers={
                'Content-Type': 'application/json',
                'X-CSRF-Token': csrf_token or '',
                'Cookie': f'_csrf={csrf_token}'
            }
        )
        with urllib.request.urlopen(req) as res:
            cookie_header = res.info().get('Set-Cookie')
            zeno_token = get_cookie_value(cookie_header, 'zeno_token')
            
        global headers_dict
        headers_dict = {
            'Cookie': f'zeno_token={zeno_token}; _csrf={csrf_token}',
            'X-CSRF-Token': csrf_token or ''
        }
        print("Authenticated successfully!")
    except Exception as e:
        print(f"Failed to authenticate: {e}")
        sys.exit(1)
    
    test_proc_name = "alt_domain_test_proc"
    test_proxy_name = "alt_domain_test_proxy"

    # Clean up existing test rules if any
    print("\nCleaning up pre-existing test data...")
    try:
        # clean proxy
        list_proxy = get_json(base_url + '/api/proxy/list')
        for prx in list_proxy.get("data", []):
            if prx["name"] == test_proxy_name:
                print(f"Removing pre-existing proxy rule {prx['id']}...")
                post_json(base_url + '/api/proxy/delete', {"id": prx["id"]})

        list_res = get_json(base_url + '/api/managed/list')
        for p in list_res.get("data", []):
            if p["name"] == test_proc_name:
                print(f"Removing pre-existing process {p['id']}...")
                post_json(base_url + '/api/managed/stop', {"id": p["id"]})
                post_json(base_url + '/api/managed/delete', {"id": p["id"]})
    except Exception as e:
        print("Cleanup failed:", e)

    # 1. Add a managed process that starts a python http server
    print(f"\n1. Adding managed process '{test_proc_name}'...")
    add_res = post_json(base_url + '/api/managed/add', {
        "name": test_proc_name,
        "command": "python3 -m http.server 9092",
        "cwd": "/home/max/Documents/PROJ/github/zenopanel",
        "env": '{"PORT": "9092"}',
        "auto_restart": False
    })
    print("Add process response:", add_res)
    proc_id = add_res.get("id")
    if not proc_id:
        print("Failed to get process ID!")
        sys.exit(1)

    # 2. Start the process
    print(f"\n2. Starting process '{test_proc_name}'...")
    start_res = post_json(base_url + '/api/managed/start', {"id": proc_id})
    print("Start process response:", start_res)

    # 3. Wait for process to start
    print("\n3. Waiting 3 seconds for process to start...")
    time.sleep(3)
    list_res = get_json(base_url + '/api/managed/list')
    found_proc = None
    for p in list_res.get("data", []):
        if p["id"] == proc_id:
            found_proc = p
            break
            
    if not found_proc:
        print("Process not found in list!")
        sys.exit(1)
        
    print(f"Process status: {found_proc['status']}")
    if found_proc['status'] != 'running':
        print(f"Expected process to be 'running', got {found_proc['status']}")
        sys.exit(1)

    # 4. Add a reverse proxy rule with alternative_domain
    print(f"\n4. Adding reverse proxy rule '{test_proxy_name}' with alternative_domain...")
    proxy_res = post_json(base_url + '/api/proxy/add', {
        "name": test_proxy_name,
        "domain": "alt-domain-test.local",
        "alternative_domain": "127.0.0.1",
        "path": "/alt_domain_test",
        "target": "http://127.0.0.1:9092",
        "strip_path": True,
        "enabled": True,
        "ssl_enabled": False,
        "managed_process_id": proc_id
    })
    print("Add proxy rule response:", proxy_res)
    proxy_id = proxy_res.get("id")
    if not proxy_id:
        print("Failed to create proxy rule!")
        sys.exit(1)

    # 5. Check api/proxy/list returns alternative_domain
    print("\n5. Checking proxy list...")
    list_proxy = get_json(base_url + '/api/proxy/list')
    found_proxy = None
    for prx in list_proxy.get("data", []):
        if prx["id"] == proxy_id:
            found_proxy = prx
            break

    if not found_proxy:
        print("Proxy rule not found in list!")
        sys.exit(1)

    print("Found proxy rule:", found_proxy)
    assert found_proxy.get("alternative_domain") == "127.0.0.1", f"Expected alternative_domain '127.0.0.1', got {found_proxy.get('alternative_domain')}"
    print("Alternative domain serialization verified successfully!")

    # 6. Request to proxy using alternative domain
    print("\n6. Accessing proxy using alternative domain...")
    req = urllib.request.Request(
        base_url + '/alt_domain_test',
        headers={'Host': '127.0.0.1'}
    )
    try:
        with urllib.request.urlopen(req) as res:
            body = res.read().decode('utf-8')
            print(f"Proxy response status: {res.status}")
            if "Directory listing for" in body or "Directory listing" in body:
                print("SUCCESS: Request successfully proxied via alternative domain!")
            else:
                print("FAILED: Proxy returned unexpected body content:", body[:200])
                sys.exit(1)
    except Exception as e:
        print("FAILED to make proxy request:", e)
        sys.exit(1)

    # 7. Clean up
    print("\n7. Cleaning up test data...")
    post_json(base_url + '/api/proxy/delete', {"id": proxy_id})
    post_json(base_url + '/api/managed/stop', {"id": proc_id})
    post_json(base_url + '/api/managed/delete', {"id": proc_id})
    print("\n=== TEST PASSED SUCCESSFULLY ===")

if __name__ == '__main__':
    main()
