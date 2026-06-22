package internal

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"syscall"

	spec "github.com/opencontainers/runtime-spec/specs-go"
)

// GenerateConfigJSON creates an OCI runtime-spec config.json for a container.
func GenerateConfigJSON(bundleDir string, cmd []string, envMap map[string]string, cwd string, mounts []string, useHostNetwork bool, memoryLimit int64, cpuLimit float64) error {
	if len(cmd) == 0 {
		cmd = []string{"/bin/sh"}
	}

	isRootless := os.Geteuid() != 0

	// Build OCI spec process
	process := spec.Process{
		Terminal: false,
		User: spec.User{
			UID: 0,
			GID: 0,
		},
		Args: cmd,
		Cwd:  "/",
		Rlimits: []spec.POSIXRlimit{
			{Type: "RLIMIT_NOFILE", Hard: 1024, Soft: 1024},
		},
	}

	// Only add capabilities when running as root
	if !isRootless {
		process.Capabilities = &spec.LinuxCapabilities{
			Bounding:    []string{"CAP_NET_BIND_SERVICE", "CAP_KILL"},
			Effective:   []string{"CAP_NET_BIND_SERVICE", "CAP_KILL"},
			Inheritable: []string{"CAP_NET_BIND_SERVICE", "CAP_KILL"},
			Permitted:   []string{"CAP_NET_BIND_SERVICE", "CAP_KILL"},
		}
	}

	// Set environment variables
	var env []string
	env = append(env, "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin")
	env = append(env, "TERM=xterm")
	env = append(env, "HOME=/root")

	if cwd != "" {
		process.Cwd = cwd
	}

	for k, v := range envMap {
		env = append(env, k+"="+v)
	}
	process.Env = env

	// Build mounts — skip privileged mounts in rootless mode
	ociMounts := []spec.Mount{
		{
			Destination: "/proc",
			Type:        "proc",
			Source:      "proc",
		},
		{
			Destination: "/dev",
			Type:        "tmpfs",
			Source:      "tmpfs",
			Options:     []string{"nosuid", "strictatime", "mode=755", "size=65536k"},
		},
	}

	if !isRootless {
		ociMounts = append(ociMounts, []spec.Mount{
			{
				Destination: "/dev/pts",
				Type:        "devpts",
				Source:      "devpts",
				Options:     []string{"nosuid", "noexec", "newinstance", "ptmxmode=0666", "mode=0620"},
			},
			{
				Destination: "/dev/shm",
				Type:        "tmpfs",
				Source:      "shm",
				Options:     []string{"nosuid", "noexec", "nodev", "mode=1777", "size=65536k"},
			},
			{
				Destination: "/sys",
				Type:        "sysfs",
				Source:      "sysfs",
				Options:     []string{"nosuid", "noexec", "nodev", "ro"},
			},
		}...)
	}

	// Add user-specified mounts (bind mounts)
	for _, m := range mounts {
		m = strings.TrimSpace(m)
		if m == "" {
			continue
		}
		parts := strings.SplitN(m, ":", 2)
		if len(parts) != 2 {
			return fmt.Errorf("invalid mount format: %s (expected hostPath:containerPath)", m)
		}
		hostPath := parts[0]
		containerPath := parts[1]

		// Resolve relative path to absolute
		if !filepath.IsAbs(hostPath) {
			absPath, err := filepath.Abs(hostPath)
			if err != nil {
				return fmt.Errorf("failed to resolve mount path %s: %w", hostPath, err)
			}
			hostPath = absPath
		}

		ociMounts = append(ociMounts, spec.Mount{
			Destination: containerPath,
			Type:        "bind",
			Source:      hostPath,
			Options:     []string{"bind", "rprivate", "rw"},
		})
	}

	// Build full OCI spec
	s := spec.Spec{
		Version: "1.0.2",
		Process: &process,
		Root: &spec.Root{
			Path:     "rootfs",
			Readonly: false,
		},
		Hostname: "zeno-container",
		Mounts:   ociMounts,
		Linux: &spec.Linux{
			Resources: func() *spec.LinuxResources {
				res := &spec.LinuxResources{}
				hasLimits := false
				if memoryLimit > 0 {
					res.Memory = &spec.LinuxMemory{
						Limit: &memoryLimit,
					}
					hasLimits = true
				}
				if cpuLimit > 0 {
					period := uint64(100000)
					quota := int64(cpuLimit * 100000)
					res.CPU = &spec.LinuxCPU{
						Period: &period,
						Quota:  &quota,
					}
					hasLimits = true
				}
				if hasLimits {
					return res
				}
				return &spec.LinuxResources{}
			}(),
			Namespaces: func() []spec.LinuxNamespace {
				ns := []spec.LinuxNamespace{
					{Type: spec.PIDNamespace},
					{Type: spec.IPCNamespace},
					{Type: spec.UTSNamespace},
					{Type: spec.MountNamespace},
				}
				// Add user namespace for rootless mode
				if isRootless {
					ns = append(ns, spec.LinuxNamespace{Type: spec.UserNamespace})
				}
				// Only isolate the network namespace when not using host networking
				if !useHostNetwork {
					ns = append(ns, spec.LinuxNamespace{Type: spec.NetworkNamespace})
				}
				return ns
			}(),
			UIDMappings: func() []spec.LinuxIDMapping {
				if isRootless {
					return []spec.LinuxIDMapping{
						{ContainerID: 0, HostID: uint32(os.Getuid()), Size: 1},
					}
				}
				return nil
			}(),
			GIDMappings: func() []spec.LinuxIDMapping {
				if isRootless {
					return []spec.LinuxIDMapping{
						{ContainerID: 0, HostID: uint32(os.Getgid()), Size: 1},
					}
				}
				return nil
			}(),
			MaskedPaths: func() []string {
				if isRootless {
					return nil
				}
				return []string{
					"/proc/acpi",
					"/proc/asound",
					"/proc/kcore",
					"/proc/keys",
					"/proc/latency_stats",
					"/proc/timer_list",
					"/proc/timer_stats",
					"/proc/sched_debug",
					"/sys/firmware",
				}
			}(),
			ReadonlyPaths: func() []string {
				if isRootless {
					return nil
				}
				return []string{
					"/proc/bus",
					"/proc/fs",
					"/proc/irq",
					"/proc/sys",
					"/proc/sysrq-trigger",
				}
			}(),
		},
	}

	// Write config.json
	configPath := filepath.Join(bundleDir, "config.json")
	data, err := json.MarshalIndent(s, "", "  ")
	if err != nil {
		return fmt.Errorf("failed to marshal config.json: %w", err)
	}

	if err := os.WriteFile(configPath, data, 0644); err != nil {
		return fmt.Errorf("failed to write config.json: %w", err)
	}

	return nil
}

// CopyRootfs sets up the container's bundle rootfs using OverlayFS.
func CopyRootfs(image string, dataDir, containerID string) error {
	ref := parseImageRef(image)
	imgDir := ImageCacheDir(dataDir) + "/" + cacheDirName(ref.Repository, ref.Tag)
	dstRootfs := RootfsDir(dataDir, containerID)

	// Check if already mounted / exists
	if _, err := os.Stat(dstRootfs); err == nil {
		return nil // Already exists/mounted
	}

	// 1. Read layers list
	layersData, err := os.ReadFile(imgDir + "/layers.json")
	if err != nil {
		// Fallback to legacy non-overlay method if layers.json is missing
		legacySrcRootfs := imgDir + "/rootfs"
		if _, err := os.Stat(legacySrcRootfs); os.IsNotExist(err) {
			return fmt.Errorf("image %s not found in cache. pull it first", image)
		}
		if err := os.MkdirAll(dstRootfs, 0755); err != nil {
			return err
		}
		fmt.Printf("  ▶ Fallback: copying rootfs for legacy image...\n")
		return copyDir(legacySrcRootfs, dstRootfs)
	}

	var layers []string
	if err := json.Unmarshal(layersData, &layers); err != nil {
		return fmt.Errorf("failed to parse layers.json: %w", err)
	}

	// 2. Build lowerdir string (reverse order: top layer first)
	var lowerdirs []string
	layersCacheDir := ImageCacheDir(dataDir) + "/layers"
	for i := len(layers) - 1; i >= 0; i-- {
		lowerdirs = append(lowerdirs, layersCacheDir+"/"+layers[i]+"/rootfs")
	}
	lowerdirStr := strings.Join(lowerdirs, ":")

	// 3. Setup container write layer (upperdir) and workdir
	containerPath := ContainerDir(dataDir, containerID)
	upperdir := containerPath + "/diff"
	workdir := containerPath + "/work"

	if err := os.MkdirAll(upperdir, 0755); err != nil {
		return fmt.Errorf("failed to create upperdir: %w", err)
	}
	if err := os.MkdirAll(workdir, 0755); err != nil {
		return fmt.Errorf("failed to create workdir: %w", err)
	}
	if err := os.MkdirAll(dstRootfs, 0755); err != nil {
		return fmt.Errorf("failed to create mountpoint: %w", err)
	}

	// 4. Perform overlay mount
	opts := fmt.Sprintf("lowerdir=%s,upperdir=%s,workdir=%s", lowerdirStr, upperdir, workdir)
	fmt.Printf("  ▶ Mounting OverlayFS on %s...\n", dstRootfs)
	if err := syscall.Mount("overlay", dstRootfs, "overlay", 0, opts); err != nil {
		return fmt.Errorf("failed to mount overlayfs (opts: %s): %w", opts, err)
	}

	return nil
}

// copyDir recursively copies a directory tree.
func copyDir(src, dst string) error {
	if err := os.MkdirAll(dst, 0755); err != nil {
		return err
	}

	entries, err := os.ReadDir(src)
	if err != nil {
		return err
	}

	for _, entry := range entries {
		srcPath := filepath.Join(src, entry.Name())
		dstPath := filepath.Join(dst, entry.Name())

		info, err := entry.Info()
		if err != nil {
			return err
		}

		if entry.IsDir() {
			if err := copyDir(srcPath, dstPath); err != nil {
				return err
			}
		} else if entry.Type()&os.ModeSymlink != 0 {
			target, err := os.Readlink(srcPath)
			if err != nil {
				return err
			}
			os.Remove(dstPath)
			if err := os.Symlink(target, dstPath); err != nil {
				return err
			}
		} else {
			if err := copyFile(srcPath, dstPath, info.Mode()); err != nil {
				return err
			}
		}
	}

	return nil
}

// copyFile copies a single file with the specified mode.
func copyFile(src, dst string, mode os.FileMode) error {
	srcFile, err := os.Open(src)
	if err != nil {
		return err
	}
	defer srcFile.Close()

	if err := os.MkdirAll(filepath.Dir(dst), 0755); err != nil {
		return err
	}

	dstFile, err := os.OpenFile(dst, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, mode)
	if err != nil {
		return err
	}
	defer dstFile.Close()

	_, err = io.Copy(dstFile, srcFile)
	return err
}
