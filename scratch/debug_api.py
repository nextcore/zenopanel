import urllib.request
import json
import os

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

def main():
    port = 81
    base_url = f'http://127.0.0.1:{port}'
    print(f"Connecting to {base_url}...")
    
    # Get CSRF
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
        
    headers = {
        'Cookie': f'zeno_token={zeno_token}; _csrf={csrf_token}',
        'X-CSRF-Token': csrf_token or ''
    }
    
    req = urllib.request.Request(base_url + '/api/managed/list', headers=headers)
    with urllib.request.urlopen(req) as res:
        data = json.loads(res.read().decode('utf-8'))
        print("=== MANAGED PROCESSES LIST ===")
        print(json.dumps(data, indent=2))

if __name__ == '__main__':
    main()
