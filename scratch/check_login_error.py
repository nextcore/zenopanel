import urllib.request
import urllib.parse
import json
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
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
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

    base_url = 'http://127.0.0.1:8080'
    print("Fetching CSRF token...")
    status, info, body = make_request(base_url + '/zpanel')
    cookie_hdr = info.get('Set-Cookie')
    csrf_token = get_cookie_value(cookie_hdr, '_csrf')
    print("CSRF token:", csrf_token)
    print("Cookie header:", cookie_hdr)

    print("\nAttempting login...")
    login_payload = json.dumps({"username": "admin", "password": "admin"}).encode('utf-8')
    status, info, body_text = make_request(
        base_url + '/zpanel',
        method='POST',
        data=login_payload,
        headers={
            'Content-Type': 'application/json',
            'X-CSRF-Token': csrf_token or '',
            'Cookie': f'_csrf={csrf_token}'
        }
    )
    print("Login Status:", status)
    print("Login Headers:", info)
    print("Login Body:", body_text)

if __name__ == '__main__':
    main()
