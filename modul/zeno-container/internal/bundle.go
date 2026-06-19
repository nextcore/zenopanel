package internal

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"

	spec "github.com/opencontainers/runtime-spec/specs-go"
)

// GenerateConfigJSON creates an OCI runtime-spec config.json for a container.
func GenerateConfigJSON(bundleDir string, cmd []string, envMap map[string]string, cwd string, mounts []string, useHostNetwork bool) error {
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
			Resources: &spec.LinuxResources{},
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

// CopyRootfs copies the cached image rootfs to the container's bundle rootfs.
func CopyRootfs(image string, dataDir, containerID string) error {
	ref := parseImageRef(image)
	srcRootfs := ImageCacheDir(dataDir) + "/" + cacheDirName(ref.Repository, ref.Tag) + "/rootfs"
	dstRootfs := RootfsDir(dataDir, containerID)

	// Check source exists
	if _, err := os.Stat(srcRootfs); os.IsNotExist(err) {
		return fmt.Errorf("image %s not found in cache. pull it first", image)
	}

	// Check if destination already has content
	if _, err := os.Stat(dstRootfs); err == nil {
		return nil // Already exists
	}

	fmt.Printf("  ▶ Copying rootfs...\n")
	return copyDir(srcRootfs, dstRootfs)
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
