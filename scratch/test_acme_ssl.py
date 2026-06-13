import urllib.request
import urllib.parse
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
    print("=== STARTING ZENOPANEL ACME SSL INTEGRATION TEST ===")
    base_url = 'http://127.0.0.1:3001'
    
    # 1. Authenticate
    print("Authenticating admin...")
    try:
        # Get CSRF token
        req = urllib.request.Request(base_url + '/zpanel')
        with urllib.request.urlopen(req) as res:
            cookie_hdr = res.info().get('Set-Cookie')
            csrf_token = get_cookie_value(cookie_hdr, '_csrf')
            
        # Login
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
        print(f"Failed to authenticate: {e}")
        sys.exit(1)

    test_proxy_name = "acme_test_proxy"
    test_domain = "zeno-acme-test.myname.com"

    # 2. Cleanup existing proxy rule
    print("\nCleaning up pre-existing proxy rules...")
    try:
        proxy_list = get_json(base_url + '/api/proxy/list')
        for r in proxy_list.get("data", []):
            if r["name"] == test_proxy_name or r["domain"] == test_domain:
                print(f"Removing pre-existing proxy {r['id']}...")
                post_json(base_url + '/api/proxy/delete', {"id": r["id"]})
        
        # Also clean up the files from disk to prevent SNI resolution cache hits
        import os
        for ext in ['.crt', '.key']:
            path = f"./certs/{test_domain}{ext}"
            if os.path.exists(path):
                print(f"Removing pre-existing certificate file: {path}")
                os.remove(path)
    except Exception as e:
        print("Error during proxy cleanup:", e)

    # 3. Add proxy rule with SSL enabled for test_domain
    print(f"\nAdding proxy rule with SSL enabled for '{test_domain}'...")
    add_res = post_json(base_url + '/api/proxy/add', {
        "name": test_proxy_name,
        "domain": test_domain,
        "path": "/",
        "target": "http://127.0.0.1:9091",
        "strip_path": True,
        "enabled": True,
        "ssl_enabled": True
    })
    print("Add proxy rule response:", add_res)
    proxy_id = add_res.get("id")
    if not proxy_id:
        print("Failed to get proxy ID!")
        sys.exit(1)

    # Trigger SNI handshake to invoke get_or_create_cert and start ACME flow
    print(f"Triggering SNI handshake to resolve '{test_domain}'...")
    import ssl
    import socket
    context = ssl.create_default_context()
    context.check_hostname = False
    context.verify_mode = ssl.CERT_NONE
    try:
        with socket.create_connection(('127.0.0.1', 8443), timeout=3) as sock:
            with context.wrap_socket(sock, server_hostname=test_domain) as ssock:
                pass
    except Exception as e:
        # Expected to fail since it is self-signed/mock, but SNI will be received
        print(f"SNI handshake triggered (received expected connection attempt: {e})")

    # 4. Monitor SSL status transitions
    print("\nMonitoring SSL status transitions...")
    max_checks = 350
    status_history = []
    
    for i in range(max_checks):
        time.sleep(0.2)
        proxy_list = get_json(base_url + '/api/proxy/list')
        found_rule = None
        for r in proxy_list.get("data", []):
            if r["id"] == proxy_id:
                found_rule = r
                break
        
        if not found_rule:
            print("Proxy rule disappeared!")
            sys.exit(1)
            
        current_status = found_rule["ssl_status"]
        if not status_history or status_history[-1] != current_status:
            status_history.append(current_status)
            print(f"[{i * 0.2:.1f}s] SSL status changed to: {current_status}")
            
        if current_status == "failed" or current_status == "active_letsencrypt":
            print(f"Reached final status '{current_status}' after {i * 0.2:.1f} seconds.")
            break
            
    print(f"SSL status transition history: {status_history}")
    
    # 5. Clean up
    print("\nCleaning up test proxy rule...")
    post_json(base_url + '/api/proxy/delete', {"id": proxy_id})
    print("Cleanup completed.")
    
    # Validation checks
    # Since Let's Encrypt Staging cannot resolve/reach zeno-acme-test.myname.com on a local system,
    # the flow should start at "none", transition to "active_self_signed", then go to "pending" (when the ACME challenge is initiated),
    # and finally transition to "failed" when the challenge times out/fails due to DNS/network reachability.
    if "active_self_signed" in status_history and "pending" in status_history and "failed" in status_history:
        print("\n=== SUCCESS: SSL ACME status transitioned exactly as expected! ===")
        sys.exit(0)
    else:
        print("\n=== FAILURE: Status transitions did not match expected path. ===")
        sys.exit(1)

if __name__ == '__main__':
    main()
