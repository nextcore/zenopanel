import re

# SSRF regex in Python (matching the Rust one)
ssrf_regex = re.compile(
    r"(?i)(127\.0\.0\.1|localhost|0\.0\.0\.0|::1|169\.254\.169\.254|metadata\.google\.internal|100\.100\.100\.200|192\.168\.\d+\.\d+|10\.\d+\.\d+\.\d+|172\.(1[6-9]|2\d|3[01])\.\d+\.\d+|file://|gopher://|dict://|ftp://internal|sftp://)"
)

user_agents = [
    # Chrome on Windows
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/115.0.0.0 Safari/537.36",
    # Chrome on macOS
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/115.0.0.0 Safari/537.36",
    # Firefox on Windows
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:109.0) Gecko/20100101 Firefox/115.0",
    # Safari on iPhone
    "Mozilla/5.0 (iPhone; CPU iPhone OS 16_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.5 Mobile/15E148 Safari/604.1",
    # Edge on Windows
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/115.0.0.0 Safari/537.36 Edg/115.0.1901.188",
]

for ua in user_agents:
    match = ssrf_regex.search(ua)
    if match:
        print(f"MATCH: {ua} matched {match.group(0)}")
    else:
        print(f"NO MATCH: {ua}")
