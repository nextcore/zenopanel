import urllib.request
import urllib.parse
import json
import time
import sys
import os
import subprocess
import signal

headers_dict = {}

def get_cookie_value(cookie_header, name):
    if not cookie_header:
        return None
    cookies = []
    if isinstance(cookie_header, list):
        cookies = cookie_header
    else:
        cookies = [cookie_header]
    for cookie in cookies:
        parts = cookie.split(';')
        for part in parts:
            part = part.strip()
            if part.startswith(name + '='):
                return part.split('=')[1]
    return None

def make_request(url, data=None, headers=None, method='GET'):
    global headers_dict
    if headers is None:
        headers = {}
    
    # Merge session cookies
    if headers_dict:
        headers.update(headers_dict)
        
    req = urllib.request.Request(url, method=method)
    for k, v in headers.items():
        req.add_header(k, v)
        
    if data:
        data_bytes = json.dumps(data).encode('utf-8')
        req.add_header('Content-Type', 'application/json')
        req.data = data_bytes
        
    try:
        with urllib.request.urlopen(req) as res:
            res_headers = res.info()
            cookie_hdr = res_headers.get('Set-Cookie')
            if cookie_hdr:
                # Update cookies
                session_id = get_cookie_value(cookie_hdr, 'session_id')
                if session_id:
                    headers_dict['Cookie'] = f"session_id={session_id}"
            return json.loads(res.read().decode('utf-8'))
    except urllib.error.HTTPError as e:
        body = e.read().decode('utf-8')
        print(f"HTTP Error {e.code}: {body}")
        try:
            return json.loads(body)
        except:
            return {"success": False, "message": body}

def get_json(url):
    return make_request(url)

def post_json(url, data):
    return make_request(url, data, method='POST')

def generate_self_signed_cert(domain, days):
    cert_path = f"./certs/{domain}.crt"
    key_path = f"./certs/{domain}.key"
    print(f"Generating custom cert for {domain} valid for {days} days...")
    
    # Ensure certs directory exists
    os.makedirs("./certs", exist_ok=True)
    
    cmd = [
        "openssl", "req", "-x509", "-newkey", "rsa:2048",
        "-keyout", key_path, "-out", cert_path,
        "-days", str(days), "-nodes",
        "-subj", f"/CN={domain}"
    ]
    subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True)

def main():
    base_url = "http://localhost:3001"
    test_domain = "zeno-expiry-test.myname.com"
    test_proxy_name = "expiry_test_proxy"
    
    print("=== STARTING SSL AUTO-RENEWAL TESTING ===")
    
    # Helper to clean up certificate files
    def cleanup_certs():
        for ext in ['.crt', '.key']:
            path = f"./certs/{test_domain}{ext}"
            if os.path.exists(path):
                os.remove(path)
                
    cleanup_certs()

    # 1. Start a temporary zeno server to configure database
    print("Starting Zeno server to setup proxy rule...")
    server_proc = subprocess.Popen(
        ["./target/debug/zeno"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        preexec_fn=os.setsid
    )
    
    # Wait for server to bind
    time.sleep(2)
    
    # Authenticate using /zpanel and CSRF
    print("Authenticating admin...")
    try:
        req = urllib.request.Request(base_url + '/zpanel')
        with urllib.request.urlopen(req) as res:
            cookie_hdr = res.info().get('Set-Cookie')
            csrf_token = get_cookie_value(cookie_hdr, '_csrf')
            
        login_data = json.dumps({"username": "admin", "password": "admin"}).encode('utf-8')
        req = urllib.request.Request(
            base_url + '/zpanel',
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
        print(f"Authentication failed: {e}")
        os.killpg(os.getpgid(server_proc.pid), signal.SIGTERM)
        sys.exit(1)
        
    # Cleanup pre-existing rules
    proxy_list = get_json(base_url + '/api/proxy/list')
    for r in proxy_list.get("data", []):
        if r["name"] == test_proxy_name or r["domain"] == test_domain:
            post_json(base_url + '/api/proxy/delete', {"id": r["id"]})
            
    # Add proxy rule
    print(f"Adding proxy rule with SSL enabled for {test_domain}...")
    add_res = post_json(base_url + '/api/proxy/add', {
        "name": test_proxy_name,
        "domain": test_domain,
        "path": "/",
        "target": "http://127.0.0.1:9091",
        "strip_path": True,
        "enabled": True,
        "ssl_enabled": True
    })
    
    proxy_id = add_res.get("id")
    if not proxy_id:
        print("Failed to add proxy rule:", add_res)
        os.killpg(os.getpgid(server_proc.pid), signal.SIGTERM)
        sys.exit(1)
        
    # Set status to active_letsencrypt initially
    # This prevents the initial SNI handshake from thinking it is pending
    # and lets us test the background worker renewal logic specifically!
    post_json(base_url + f"/api/proxy/update", {
        "id": proxy_id,
        "name": test_proxy_name,
        "domain": test_domain,
        "path": "/",
        "target": "http://127.0.0.1:9091",
        "strip_path": True,
        "enabled": True,
        "ssl_enabled": True,
        "ssl_status": "active_letsencrypt"
    })
    
    # Kill the server process to start test cases
    os.killpg(os.getpgid(server_proc.pid), signal.SIGTERM)
    server_proc.wait()
    
    # -------------------------------------------------------------
    # CASE 1: Cert valid for 90 days (should NOT trigger renewal)
    # -------------------------------------------------------------
    print("\n--- CASE 1: Certificate valid for 90 days (Should NOT renew) ---")
    cleanup_certs()
    generate_self_signed_cert(test_domain, 90)
    
    print("Starting Zeno server...")
    server_proc = subprocess.Popen(
        ["./target/debug/zeno"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        preexec_fn=os.setsid
    )
    
    # Wait for the initial 10-second worker sleep + 5 seconds for check
    print("Waiting 15 seconds for the background renewal worker to run...")
    time.sleep(15)
    
    # Read server logs to check if renewal was triggered
    os.killpg(os.getpgid(server_proc.pid), signal.SIGTERM)
    stdout, stderr = server_proc.communicate()
    
    triggered_case1 = "[SSL Renewal] Cert for '{}' needs renewal".format(test_domain) in stdout
    print("Server Log Excerpt (Case 1):")
    for line in stdout.splitlines():
        if "[SSL Renewal]" in line:
            print("  ", line)
            
    if triggered_case1:
        print("❌ FAILED: Renewal was unexpectedly triggered for a 90-day valid certificate!")
        sys.exit(1)
    else:
        print("✅ SUCCESS: 90-day certificate was correctly ignored by the renewal worker.")
        
    # -------------------------------------------------------------
    # CASE 2: Cert valid for 1 day (should trigger renewal)
    # -------------------------------------------------------------
    print("\n--- CASE 2: Certificate valid for 1 day (Should trigger renewal) ---")
    cleanup_certs()
    generate_self_signed_cert(test_domain, 1)
    
    print("Starting Zeno server...")
    server_proc = subprocess.Popen(
        ["./target/debug/zeno"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        preexec_fn=os.setsid
    )
    
    # Wait for the initial 10-second worker sleep + 5 seconds for check
    print("Waiting 15 seconds for the background renewal worker to run...")
    time.sleep(15)
    
    # Read server logs to check if renewal was triggered
    os.killpg(os.getpgid(server_proc.pid), signal.SIGTERM)
    stdout, stderr = server_proc.communicate()
    
    triggered_case2 = "needs renewal" in stdout and test_domain in stdout
    print("Server Log Excerpt (Case 2):")
    for line in stdout.splitlines():
        if "[SSL Renewal]" in line or "needs renewal" in line:
            print("  ", line)
            
    cleanup_certs()
    
    if triggered_case2:
        print("✅ SUCCESS: 1-day certificate was correctly identified as needing renewal and triggered ACME!")
        print("\n=== ALL AUTO-RENEWAL TESTS PASSED SUCCESSFULLY! ===")
    else:
        print("❌ FAILED: 1-day certificate did not trigger renewal!")
        sys.exit(1)

if __name__ == "__main__":
    main()
