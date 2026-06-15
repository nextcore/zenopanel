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

def make_request(url, method='GET', data=None, headers=None):
    if headers is None:
        headers = {}
    encoded_data = None
    if data is not None:
        encoded_data = json.dumps(data).encode('utf-8')
        headers['Content-Type'] = 'application/json'
    req = urllib.request.Request(url, data=encoded_data, headers=headers, method=method)
    try:
        res = urllib.request.urlopen(req)
        body = res.read().decode('utf-8')
        return res.getcode(), res.info(), body
    except urllib.error.HTTPError as e:
        body = e.read().decode('utf-8')
        return e.code, e.info(), body

def main():
    opener = urllib.request.build_opener(NoRedirectHandler())
    urllib.request.install_opener(opener)
    base_url = 'http://127.0.0.1:3001'

    # Get active entrance path from DB
    conn = sqlite3.connect('zeno.db')
    cursor = conn.cursor()
    cursor.execute("SELECT value FROM settings WHERE key = 'entrance_path'")
    row = cursor.fetchone()
    db_entrance_path = row[0] if row else '/login'
    if not db_entrance_path.startswith('/'):
        db_entrance_path = '/' + db_entrance_path
    print(f"Detected active entrance path: {db_entrance_path}")

    # Fetch CSRF token
    status, info, _ = make_request(base_url + db_entrance_path)
    cookie_hdr = info.get('Set-Cookie')
    csrf_token = get_cookie_value(cookie_hdr, '_csrf')
    if not csrf_token:
        print("❌ FAILED: CSRF token not found")
        sys.exit(1)

    # Login as admin
    status, info, body = make_request(
        base_url + db_entrance_path,
        method='POST',
        data={"username": "admin", "password": "admin"},
        headers={'X-CSRF-Token': csrf_token, 'Cookie': f'_csrf={csrf_token}'}
    )
    set_cookies = info.get_all('Set-Cookie')
    admin_token = get_cookie_value(set_cookies, 'zeno_token')
    if not admin_token:
        print("❌ FAILED: Admin login failed")
        sys.exit(1)

    # Retrieve main page HTML
    headers = {
        'Cookie': f'zeno_token={admin_token}; _csrf={csrf_token}',
        'X-CSRF-Token': csrf_token
    }
    status, _, html = make_request(base_url + '/', headers=headers)
    if status != 200:
        print(f"❌ FAILED: Could not fetch dashboard HTML (status {status})")
        sys.exit(1)

    # Check for sidebar elements
    has_nav_security = 'id="nav-security"' in html
    has_nav_settings = 'id="nav-settings"' in html
    
    # Check for viewports
    has_tab_security = 'id="tab-security"' in html
    has_tab_settings = 'id="tab-settings"' in html

    # Check that settings tab does NOT contain security elements
    has_waf_in_settings = 'settings-waf-enabled' in html
    # But settings-waf-enabled should be present inside tab-security!
    # Let's inspect where settings-waf-enabled is located.
    # It should be in the html because tab-security is included in the page.
    # Let's check the position or slice.
    
    print("Verification results:")
    print(f" - nav-security in sidebar: {has_nav_security}")
    print(f" - nav-settings in sidebar: {has_nav_settings}")
    print(f" - tab-security viewport: {has_tab_security}")
    print(f" - tab-settings viewport: {has_tab_settings}")
    
    if not has_nav_security:
        print("❌ FAILED: nav-security not found in sidebar")
        sys.exit(1)
        
    if not has_tab_security:
        print("❌ FAILED: tab-security viewport not found in main page")
        sys.exit(1)

    # Let's find settings-waf-enabled to ensure it is in the page
    if 'id="settings-waf-enabled"' not in html:
        print("❌ FAILED: settings-waf-enabled input element not found in HTML")
        sys.exit(1)

    # Confirm that 'settings-waf-enabled' is NOT inside 'tab-settings' but inside 'tab-security'
    # We can split by viewport divs
    parts = html.split('id="tab-')
    settings_part = None
    security_part = None
    for part in parts:
        if part.startswith('settings"'):
            settings_part = part
        elif part.startswith('security"'):
            security_part = part

    if settings_part is None:
        print("❌ FAILED: Could not isolate tab-settings in HTML")
        sys.exit(1)
    if security_part is None:
        print("❌ FAILED: Could not isolate tab-security in HTML")
        sys.exit(1)

    if 'settings-waf-enabled' in settings_part:
        print("❌ FAILED: settings-waf-enabled still present in tab-settings viewport")
        sys.exit(1)
    else:
        print("✅ SUCCESS: settings-waf-enabled removed from tab-settings viewport")

    if 'settings-waf-enabled' in security_part:
        print("✅ SUCCESS: settings-waf-enabled correctly placed in tab-security viewport")
    else:
        print("❌ FAILED: settings-waf-enabled not found in tab-security viewport")
        sys.exit(1)

    print("🎉 All menu segregation verifications passed successfully!")

if __name__ == '__main__':
    main()
