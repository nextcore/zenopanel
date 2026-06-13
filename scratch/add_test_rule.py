import urllib.request
import urllib.parse
import json
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
    base_url = 'http://127.0.0.1:3001'
    
    # 1. Authenticate
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
        print(f"Failed to authenticate: {e}")
        sys.exit(1)

    test_proxy_name = "acme_test_proxy"
    test_domain = "zeno-acme-test.myname.com"

    # Cleanup existing proxy rule
    try:
        proxy_list = get_json(base_url + '/api/proxy/list')
        for r in proxy_list.get("data", []):
            if r["name"] == test_proxy_name or r["domain"] == test_domain:
                print(f"Removing pre-existing proxy {r['id']}...")
                post_json(base_url + '/api/proxy/delete', {"id": r["id"]})
    except Exception as e:
        print("Error during proxy cleanup:", e)

    # Add proxy rule
    print(f"Adding proxy rule with SSL enabled for '{test_domain}'...")
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

if __name__ == '__main__':
    main()
