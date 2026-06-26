import json
import os

data_dir = "/var/lib/zeno-container"
image = "mysql:8.4"
id = "mysql-84"

# Read layers.json
layers_path = f"{data_dir}/images/library_mysql_8.4/layers.json"
with open(layers_path, "r") as f:
    layers = json.load(f)

# Reversing layers to match mount_overlayfs logic
lowerdirs = [f"{data_dir}/images/layers/{layer}/rootfs" for layer in reversed(layers)]
lowerdir_str = ":".join(lowerdirs)

upperdir = f"{data_dir}/containers/{id}/diff"
workdir = f"{data_dir}/containers/{id}/work"
dst_rootfs = f"{data_dir}/containers/{id}/bundle/rootfs"

opts = f"lowerdir={lowerdir_str},upperdir={upperdir},workdir={workdir}"

cmd = f"sudo mount -t overlay overlay -o {opts} {dst_rootfs}"
print("="*80)
print("PROPOSED MOUNT COMMAND:")
print(cmd)
print("="*80)
