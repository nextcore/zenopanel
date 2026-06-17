import urllib.request
import urllib.parse
import urllib.error
import json
import time
import sys

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
    print("=== STARTING ZENOPANEL VERIFICATION ===")
    base_url = 'http://127.0.0.1:3000'

    import sqlite3
    try:
        conn = sqlite3.connect('zeno.db')
        cursor = conn.cursor()
        cursor.execute("SELECT value FROM settings WHERE key = 'entrance_path'")
        row = cursor.fetchone()
        entrance_path = row[0] if row else '/login'
        conn.close()
    except Exception:
        entrance_path = '/login'
    if not entrance_path.startswith('/'):
        entrance_path = '/' + entrance_path

    print(f"Using entrance path: {entrance_path}")
    
    # Authenticate first
    print("Authenticating admin...")
    try:
        # Get CSRF token
        req = urllib.request.Request(base_url + entrance_path)
        with urllib.request.urlopen(req) as res:
            cookie_hdr = res.info().get('Set-Cookie')
            csrf_token = get_cookie_value(cookie_hdr, '_csrf')
            
        # Login
        login_data = json.dumps({"username": "admin", "password": "admin"}).encode('utf-8')
        req = urllib.request.Request(
            base_url + entrance_path,
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
    except urllib.error.HTTPError as e:
        print(f"Failed to authenticate: {e}")
        try:
            print("Response body:", e.read().decode('utf-8'))
        except Exception:
            pass
        sys.exit(1)
    except Exception as e:
        print(f"Failed to authenticate: {e}")
        sys.exit(1)
    
    test_proc_name = "webkota_test"
    test_proxy_name = "webkota_proxy_test"

    # Clean up existing test rules if any
    print("\nCleaning up pre-existing test data...")
    try:
        list_res = get_json('http://127.0.0.1:3000/api/managed/list')
        for p in list_res.get("data", []):
            if p["name"] == test_proc_name:
                print(f"Removing pre-existing process {p['id']}...")
                post_json('http://127.0.0.1:3000/api/managed/stop', {"id": p["id"]})
                post_json('http://127.0.0.1:3000/api/managed/delete', {"id": p["id"]})
    except Exception as e:
        print("Error during process cleanup:", e)

    try:
        proxy_list = get_json('http://127.0.0.1:3000/api/proxy/list')
        for r in proxy_list.get("data", []):
            if r["name"] == test_proxy_name:
                print(f"Removing pre-existing proxy {r['id']}...")
                post_json('http://127.0.0.1:3000/api/proxy/delete', {"id": r["id"]})
    except Exception as e:
        print("Error during proxy cleanup:", e)

    # 1. Add a managed process
    print(f"\n1. Adding managed process '{test_proc_name}'...")
    add_res = post_json('http://127.0.0.1:3000/api/managed/add', {
        "name": test_proc_name,
        "command": "python3 -m http.server 9091",
        "cwd": "/home/max/Documents/PROJ/github/zenopanel",
        "auto_restart": True
    })
    print("Add process response:", add_res)
    proc_id = add_res.get("id")
    if not proc_id:
        print("Failed to get process ID!")
        sys.exit(1)

    # 2. Start the process
    print(f"\n2. Starting process '{test_proc_name}'...")
    start_res = post_json('http://127.0.0.1:3000/api/managed/start', {"id": proc_id})
    print("Start process response:", start_res)

    # 3. Wait for process to start and verify resource usage metrics
    print("\n3. Waiting 3 seconds and fetching process list for metrics...")
    time.sleep(3)
    list_res = get_json('http://127.0.0.1:3000/api/managed/list')
    found_proc = None
    for p in list_res.get("data", []):
        if p["id"] == proc_id:
            found_proc = p
            break
            
    if not found_proc:
        print("Process not found in list!")
        sys.exit(1)
        
    print(f"Process status: {found_proc['status']}")
    print(f"Process PID: {found_proc['pid']}")
    print(f"Process CPU usage: {found_proc['cpu_usage']}%")
    print(f"Process memory usage: {found_proc['memory_usage']} MB")
    
    if found_proc['status'] != 'running':
        print(f"Expected process to be 'running', got {found_proc['status']}")
        sys.exit(1)

    # 4. Add a reverse proxy rule linked to this process
    print(f"\n4. Adding reverse proxy rule linked to '{test_proc_name}'...")
    proxy_res = post_json('http://127.0.0.1:3000/api/proxy/add', {
        "name": test_proxy_name,
        "domain": "localhost",
        "path": "/webkota_test_proxy",
        "target": "http://127.0.0.1:9091",
        "strip_path": True,
        "enabled": True,
        "ssl_enabled": False,
        "managed_process_id": proc_id
    })
    print("Add proxy rule response:", proxy_res)
    proxy_id = proxy_res.get("id")
    
    # 5. Access via proxy to verify forwarding
    print("\n5. Testing HTTP forwarding to the proxy...")
    req = urllib.request.Request(
        'http://127.0.0.1:3000/webkota_test_proxy',
        headers={'Host': 'localhost'}
    )
    with urllib.request.urlopen(req) as res:
        body = res.read().decode('utf-8')
        print(f"Proxy response status: {res.status}")
        if "Directory listing for" in body:
            print("Successfully proxied request to python http server!")
        else:
            print("Response did not contain directory listing:")
            print(body[:200])
            sys.exit(1)

    # 6. Stop the process and verify 503 custom error page
    print(f"\n6. Stopping process '{test_proc_name}'...")
    stop_res = post_json('http://127.0.0.1:3000/api/managed/stop', {"id": proc_id})
    print("Stop process response:", stop_res)
    
    time.sleep(1) # wait briefly
    
    print("\n7. Testing HTTP proxy request when process is stopped...")
    req_err = urllib.request.Request(
        'http://127.0.0.1:3000/webkota_test_proxy',
        headers={'Host': 'localhost'}
    )
    try:
        urllib.request.urlopen(req_err)
        print("Expected HTTP error response, but got successful connection!")
        sys.exit(1)
    except urllib.error.HTTPError as e:
        print(f"Got HTTP status code: {e.code} (expected 503)")
        err_body = e.read().decode('utf-8')
        if test_proxy_name in err_body and "is Unavailable" in err_body and "Stopped" in err_body:
            print("Successfully verified process-aware custom error page!")
        else:
            print("Incorrect custom error page content:")
            print(err_body[:500])
            sys.exit(1)

    # 8. Check and download logs
    print("\n8. Downloading full log file...")
    download_url = f'http://127.0.0.1:3000/api/processes/download_log?id={proc_id}'
    req = urllib.request.Request(download_url, headers=headers_dict)
    with urllib.request.urlopen(req) as res:
        log_content = res.read().decode('utf-8')
        print(f"Log content length: {len(log_content)} bytes")
        print("Log contents preview:")
        print(log_content)
        if "[ZenoPanel] Process started" in log_content and "[ZenoPanel] Process stopped" in log_content:
            print("Successfully verified persistent log content!")
        else:
            print("Logs missing expected supervisor markers!")
            sys.exit(1)

    # 9. Clean up database
    print("\n9. Cleaning up proxy rule and managed process...")
    del_proxy = post_json('http://127.0.0.1:3000/api/proxy/delete', {"id": proxy_id})
    print("Delete proxy response:", del_proxy)
    del_proc = post_json('http://127.0.0.1:3000/api/managed/delete', {"id": proc_id})
    print("Delete process response:", del_proc)

    print("\n=== ZENOPANEL VERIFICATION SUCCESSFUL ===")

if __name__ == '__main__':
    main()
