import urllib.request
import json
import sqlite3

base_url = 'http://127.0.0.1:3001'

# 0. Read entrance path from zeno.db
conn = sqlite3.connect('zeno.db')
cursor = conn.cursor()
cursor.execute("SELECT value FROM settings WHERE key = 'entrance_path'")
row = cursor.fetchone()
entrance_path = row[0] if row else '/login'
print("Detected entrance path:", entrance_path)

# 1. Fetch CSRF token
req = urllib.request.Request(base_url + entrance_path)
res = urllib.request.urlopen(req)
cookie_hdr = res.info().get('Set-Cookie')
csrf_token = None
for cookie in cookie_hdr.split(','):
    parts = cookie.strip().split(';')[0].split('=')
    if parts[0] == '_csrf':
        csrf_token = parts[1]
print("CSRF Token:", csrf_token)

# 2. Login
login_data = json.dumps({"username": "admin", "password": "admin"}).encode('utf-8')
req = urllib.request.Request(
    base_url + entrance_path,
    data=login_data,
    headers={
        'Content-Type': 'application/json',
        'X-CSRF-Token': csrf_token,
        'Cookie': f'_csrf={csrf_token}'
    },
    method='POST'
)
res = urllib.request.urlopen(req)
set_cookies = res.info().get_all('Set-Cookie')
token = None
for cookie_line in set_cookies:
    parts = cookie_line.split(';')[0].split('=')
    if parts[0].strip() == 'zeno_token':
        token = parts[1].strip()
print("Token:", token)

# 3. Get settings
req = urllib.request.Request(
    base_url + '/api/settings',
    headers={
        'Cookie': f'zeno_token={token}; _csrf={csrf_token}',
        'X-CSRF-Token': csrf_token
    }
)
res = urllib.request.urlopen(req)
print("Settings response status:", res.getcode())
print("Settings response body:", res.read().decode('utf-8'))
