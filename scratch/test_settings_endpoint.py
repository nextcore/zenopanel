import urllib.request
base_url = 'http://127.0.0.1:3001'
req = urllib.request.Request(base_url + '/api/test-settings')
try:
    res = urllib.request.urlopen(req)
    print("Response status:", res.getcode())
    print("Response body:", res.read().decode('utf-8'))
except Exception as e:
    print("Error:", e)
