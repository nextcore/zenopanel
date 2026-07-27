import urllib.request
import urllib.parse
import json
import sqlite3
import sys

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

    conn = sqlite3.connect('dist/zenopanel-v1.7.15/zeno.db')
    cursor = conn.cursor()
    cursor.execute("SELECT value FROM settings WHERE key = 'entrance_path'")
    row = cursor.fetchone()
    entrance_path = row[0] if row else '/login'
    if not entrance_path.startswith('/'):
        entrance_path = '/' + entrance_path
    conn.close()
    
    print(f"Connecting to {base_url} with entrance path: {entrance_path}")
    
    # 1. Get CSRF Token
    req = urllib.request.Request(base_url + entrance_path)
    res = urllib.request.urlopen(req)
    cookie_hdr = res.info().get('Set-Cookie')
    csrf_token = get_cookie_value(cookie_hdr, '_csrf')
    print(f"CSRF Token: {csrf_token}")
    
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
    print(f"Zeno Token: {zeno_token}")
    
    # 3. Fetch tags for library/alpine
    req = urllib.request.Request(
        base_url + '/api/containers/docker-tags?repo=library/alpine',
        headers={'Cookie': f'zeno_token={zeno_token}'}
    )
    res = urllib.request.urlopen(req)
    print("AlmaLinux Tags Status Code:", res.getcode())
    print("AlmaLinux Response Body:", res.read().decode('utf-8'))

if __name__ == '__main__':
    main()
