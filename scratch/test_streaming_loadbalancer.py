import urllib.request
import urllib.parse
import json
import time
import sys
import os
import http.server
import threading
from http import HTTPStatus

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

# Custom Mock Target Servers to track requests
class MockTargetHandler(http.server.BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        pass # Suppress logging
    def do_GET(self):
        self.send_response(HTTPStatus.OK)
        self.send_header("Content-Type", "text/plain")
        self.end_headers()
        self.wfile.write(f"Response from mock server on port {self.server.server_port}".encode('utf-8'))
    def do_POST(self):
        content_length = int(self.headers.get('Content-Length', 0))
        body = self.rfile.read(content_length)
        self.send_response(HTTPStatus.OK)
        self.send_header("Content-Type", "text/plain")
        self.end_headers()
        # Echo back the received length and port
        self.wfile.write(f"Received {len(body)} bytes on port {self.server.server_port}".encode('utf-8'))

def start_mock_server(port):
    server = http.server.HTTPServer(('127.0.0.1', port), MockTargetHandler)
    t = threading.Thread(target=server.serve_forever, daemon=True)
    t.start()
    return server

def main():
    print("=== STARTING ZENOPANEL STREAMING AND LOAD BALANCER TEST ===")

    # 1. Start two mock target servers
    port_a = 9096
    port_b = 9097
    print(f"Starting mock target servers on ports {port_a} and {port_b}...")
    server_a = start_mock_server(port_a)
    server_b = start_mock_server(port_b)

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

    test_proxy_name = "lb_test_proxy"
    proxy_port = 8089

    # Clean up existing test rules if any
    print("\nCleaning up pre-existing test data...")
    try:
        list_proxy = get_json(base_url + '/api/proxy/list')
        for prx in list_proxy.get("data", []):
            if prx["name"] == test_proxy_name:
                print(f"Removing pre-existing proxy rule {prx['id']}...")
                post_json(base_url + '/api/proxy/delete', {"id": prx["id"]})
    except Exception as e:
        print("Cleanup failed:", e)

    # 2. Add a reverse proxy rule with comma-separated targets for Load Balancing
    print(f"\n2. Adding load-balanced proxy rule '{test_proxy_name}'...")
    proxy_res = post_json(base_url + '/api/proxy/add', {
        "name": test_proxy_name,
        "domain": "lb-test.local",
        "alternative_domain": f"127.0.0.1:{proxy_port}",
        "path": "/",
        "target": f"http://127.0.0.1:{port_a}, http://127.0.0.1:{port_b}",
        "strip_path": True,
        "enabled": True,
        "ssl_enabled": False,
        "managed_process_id": None
    })
    print("Add proxy rule response:", proxy_res)
    proxy_id = proxy_res.get("id")
    if not proxy_id:
        print("Failed to create proxy rule!")
        sys.exit(1)

    # Wait for dynamic port listener to spin up
    print(f"\n3. Waiting 3 seconds for dynamic listener on port {proxy_port} to bind...")
    time.sleep(3)

    # 4. Perform multiple requests and verify Round-Robin distribution
    print("\n4. Verifying Load Balancing (Round-Robin)...")
    results = []
    for i in range(4):
        req = urllib.request.Request(
            f"http://127.0.0.1:{proxy_port}/",
            headers={'Host': f'127.0.0.1:{proxy_port}'}
        )
        try:
            with urllib.request.urlopen(req) as res:
                body = res.read().decode('utf-8')
                print(f"Request {i+1}: {body}")
                results.append(body)
        except Exception as e:
            print(f"Request {i+1} failed:", e)
            sys.exit(1)

    # Check that it alternated
    expected_responses = [
        f"Response from mock server on port {port_a}",
        f"Response from mock server on port {port_b}"
    ]
    
    if results[0] == expected_responses[0] and results[1] == expected_responses[1] and \
       results[2] == expected_responses[0] and results[3] == expected_responses[1]:
        print("✅ SUCCESS: Load balancing successfully distributed requests in Round-Robin order!")
    else:
        print("❌ FAILED: Load balancing did not distribute requests as expected.")
        sys.exit(1)

    # 5. Verify streaming file upload/download proxy forwarding
    print("\n5. Verifying streaming/forwarding of large payload...")
    large_payload = b"X" * (5 * 1024 * 1024) # 5 MB payload
    req = urllib.request.Request(
        f"http://127.0.0.1:{proxy_port}/",
        data=large_payload,
        headers={
            'Host': f'127.0.0.1:{proxy_port}',
            'Content-Type': 'application/octet-stream'
        }
    )
    try:
        with urllib.request.urlopen(req) as res:
            body = res.read().decode('utf-8')
            print("Response:", body)
            if "Received 5242880 bytes" in body:
                print("✅ SUCCESS: Large streaming payload forwarded successfully!")
            else:
                print("❌ FAILED: Unexpected response for large payload:", body)
                sys.exit(1)
    except Exception as e:
        print("❌ FAILED: Request with large payload failed:", e)
        sys.exit(1)

    # 6. Clean up
    print("\n6. Cleaning up proxy rule...")
    post_json(base_url + '/api/proxy/delete', {"id": proxy_id})

    # Close mock servers
    server_a.shutdown()
    server_b.shutdown()

    print("\n=== ALL TESTS PASSED SUCCESSFULLY ===")

if __name__ == '__main__':
    main()
