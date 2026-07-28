import yaml
import json

yaml_str = """services:
  mysql56:
    image: mysql:5.6
    container_name: mysql56
    environment:
      - "MYSQL_ROOT_PASSWORD=SM6mwSqbFFGRbviZ"
    ports:
      - '127.0.0.1:3306:3306'
    volumes:
      - mysql56_data:/var/lib/mysql
      - /var/lib/zenopanel/db/mysql56_conf:/etc/mysql/conf.d
    restart: unless-stopped
    oom_score_adj: -1000

volumes:
  mysql56_data:
"""

try:
    data = yaml.safe_load(yaml_str)
    print("Parsed JSON:")
    print(json.dumps(data, indent=2))
except Exception as e:
    print("Error:", e)
