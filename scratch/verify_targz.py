import urllib.request
import urllib.parse
import urllib.error
import json
import sys
import os
import shutil

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

def main():
    print("=== STARTING TAR.GZ VERIFICATION ===")
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

    # Authenticate
    try:
        req = urllib.request.Request(base_url + entrance_path)
        with urllib.request.urlopen(req) as res:
            cookie_hdr = res.info().get('Set-Cookie')
            csrf_token = get_cookie_value(cookie_hdr, '_csrf')
            
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
    except Exception as e:
        print(f"Failed to authenticate: {e}")
        sys.exit(1)

    # 1. Create directory structure for testing
    test_dir = 'scratch/targz_test'
    os.makedirs(os.path.join(test_dir, 'subdir'), exist_ok=True)
    with open(os.path.join(test_dir, 'file1.txt'), 'w') as f:
        f.write('Hello World 1')
    with open(os.path.join(test_dir, 'subdir', 'file2.txt'), 'w') as f:
        f.write('Hello World 2')

    # 2. Call /api/files/archive to create .tar.gz
    archive_dest = 'scratch/targz_test.tar.gz'
    if os.path.exists(archive_dest):
        os.remove(archive_dest)

    print("Archiving to .tar.gz...")
    res = post_json(base_url + '/api/files/archive', {
        'path': test_dir,
        'dest': archive_dest
    })
    print("Archive response:", res)
    assert res.get('success'), "Archive failed"
    assert os.path.exists(archive_dest), "Archive file not found on disk"

    # Remove the original test dir to prove extraction works
    shutil.rmtree(test_dir)

    # 3. Call /api/files/extract to extract .tar.gz
    extract_dest = 'scratch/targz_extracted'
    if os.path.exists(extract_dest):
        shutil.rmtree(extract_dest)

    print("Extracting .tar.gz...")
    res = post_json(base_url + '/api/files/extract', {
        'path': archive_dest,
        'dest': extract_dest
    })
    print("Extract response:", res)
    assert res.get('success'), "Extract failed"

    # Verify extracted contents
    file1_path = os.path.join(extract_dest, 'targz_test', 'file1.txt')
    file2_path = os.path.join(extract_dest, 'targz_test', 'subdir', 'file2.txt')
    
    assert os.path.exists(file1_path), "file1.txt not found in extracted output"
    assert os.path.exists(file2_path), "file2.txt not found in extracted output"

    with open(file1_path, 'r') as f:
        content1 = f.read()
    with open(file2_path, 'r') as f:
        content2 = f.read()

    assert content1 == 'Hello World 1', f"Content mismatch: {content1}"
    assert content2 == 'Hello World 2', f"Content mismatch: {content2}"

    print("=== TAR.GZ VERIFICATION SUCCESSFUL ===")
    
    # Cleanup
    os.remove(archive_dest)
    shutil.rmtree(extract_dest)

if __name__ == '__main__':
    main()
