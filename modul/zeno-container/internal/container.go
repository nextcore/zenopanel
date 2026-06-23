package internal

import (
	"bufio"
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"os"
	"os/exec"
	"strings"
	"syscall"
	"time"
)

// ContainerManager handles container lifecycle via runc.
type ContainerManager struct {
	DataDir string
	runcBin string
}

func NewContainerManager(dataDir string) *ContainerManager {
	return &ContainerManager{DataDir: dataDir}
}

func (cm *ContainerManager) runcRoot() string { return cm.DataDir + "/runc" }

func (cm *ContainerManager) getRuncBin() string {
	if cm.runcBin == "" {
		cm.runcBin = EnsureRuncBin()
	}
	return cm.runcBin
}

func (cm *ContainerManager) EnsureDirs() error {
	for _, d := range []string{
		cm.DataDir + "/containers",
		cm.DataDir + "/images",
		cm.DataDir + "/runc",
	} {
		if err := os.MkdirAll(d, 0755); err != nil {
			return fmt.Errorf("mkdir %s: %w", d, err)
		}
	}
	return nil
}

// runcExec runs a runc command and discards output. Uses cmd.Run() to avoid pipe deadlocks.
func (cm *ContainerManager) runcExec(args ...string) error {
	runcArgs := append([]string{"--root", cm.runcRoot()}, args...)
	cmd := exec.Command(cm.getRuncBin(), runcArgs...)
	cmd.Stdin = nil
	cmd.Stdout = nil
	cmd.Stderr = nil
	return cmd.Run()
}

// runcExecOutput runs a runc command and returns combined output. Uses buffer, not pipe.
func (cm *ContainerManager) runcExecOutput(args ...string) (string, error) {
	runcArgs := append([]string{"--root", cm.runcRoot()}, args...)
	cmd := exec.Command(cm.getRuncBin(), runcArgs...)
	cmd.Stdin = nil
	var buf bytes.Buffer
	cmd.Stdout = &buf
	cmd.Stderr = &buf
	err := cmd.Run()
	return buf.String(), err
}

func (cm *ContainerManager) saveState(state *ContainerState) error {
	dir := ContainerDir(cm.DataDir, state.ID)
	os.MkdirAll(dir, 0755)
	data, _ := json.MarshalIndent(state, "", "  ")
	return os.WriteFile(StateFile(cm.DataDir, state.ID), data, 0644)
}

func (cm *ContainerManager) loadState(id string) (*ContainerState, error) {
	data, err := os.ReadFile(StateFile(cm.DataDir, id))
	if err != nil {
		return nil, fmt.Errorf("container %s not found", id)
	}
	var state ContainerState
	json.Unmarshal(data, &state)
	return &state, nil
}

// ContainerCreate — setup bundle only (NO runc command)
func (cm *ContainerManager) ContainerCreate(id, image string, cmd []string, env map[string]string,
	cwd string, mounts []string, ports []string, useHostNetwork bool, restartPolicy string, healthConfig *HealthCheckConfig, memoryLimit int64, cpuLimit float64, oomScoreAdj *int, readonlyRootfs bool) error {

	if _, err := os.Stat(StateFile(cm.DataDir, id)); err == nil {
		return fmt.Errorf("container %s already exists", id)
	}

	bundleDir := BundleDir(cm.DataDir, id)
	os.MkdirAll(bundleDir, 0755)

	if err := CopyRootfs(image, cm.DataDir, id); err != nil {
		return fmt.Errorf("copy rootfs: %w", err)
	}
	if err := GenerateConfigJSON(bundleDir, cmd, env, cwd, mounts, useHostNetwork, memoryLimit, cpuLimit, oomScoreAdj, readonlyRootfs); err != nil {
		return fmt.Errorf("config: %w", err)
	}

	state := NewContainerState(id, image, cmd)
	state.Env = env
	state.Cwd = cwd
	state.Mounts = mounts
	state.Ports = ports
	state.HostNetwork = useHostNetwork
	state.RestartPolicy = restartPolicy
	state.DesiredStatus = StatusStopped
	state.MemoryLimit = memoryLimit
	state.CPULimit = cpuLimit
	state.OOMScoreAdj = oomScoreAdj
	state.ReadOnly = readonlyRootfs
	state.HealthCheck = healthConfig
	state.LogPath = ContainerLogPath(cm.DataDir, id)
	return cm.saveState(state)
}

// ContainerRun — runs "runc run" SYNCHRONOUSLY. Captures stdout/stderr to console.log.
// Blocks until container exits. Updates state to "stopped" on exit.
func (cm *ContainerManager) ContainerRun(id string) error {
	state, err := cm.loadState(id)
	if err != nil {
		return err
	}
	if state.Status == StatusRunning {
		return fmt.Errorf("container %s is already running", id)
	}

	bundleDir := BundleDir(cm.DataDir, id)
	logPath := ContainerLogPath(cm.DataDir, id)
	logFile, err := os.Create(logPath)
	if err != nil {
		return fmt.Errorf("create log: %w", err)
	}
	defer logFile.Close()

	state.Status = StatusRunning
	state.DesiredStatus = StatusRunning
	cm.saveState(state)

	runcBin := cm.getRuncBin()
	cmd := exec.Command(runcBin, "--root", cm.runcRoot(), "run", "-b", bundleDir, id)
	cmd.Stdin = nil
	cmd.Stdout = logFile
	cmd.Stderr = logFile

	err = cmd.Run()

	// Update state after run
	state, _ = cm.loadState(id)
	state.Status = StatusStopped
	state.ExitedAt = time.Now().UTC().Format(time.RFC3339)
	if err != nil {
		if ee, ok := err.(*exec.ExitError); ok {
			state.ExitCode = ee.ExitCode()
		} else {
			// Non-exit error but container might have run (rootless mode)
			// Check if log file has content to determine success
			if fi, statErr := os.Stat(logPath); statErr == nil && fi.Size() > 0 {
				state.ExitCode = 0
			} else {
				state.ExitCode = -1
			}
		}
	} else {
		state.ExitCode = 0
	}
	state.PID = 0
	state.DesiredStatus = StatusStopped
	cm.saveState(state)
	return nil
}

// ContainerStart — starts container DETACHED via "runc create" + "runc start".
// Does NOT capture logs (use ContainerRun for log capture).
func (cm *ContainerManager) ContainerStart(id string) error {
	state, err := cm.loadState(id)
	if err != nil {
		return err
	}
	if state.Status == StatusRunning {
		return fmt.Errorf("container %s is already running", id)
	}

	bundleDir := BundleDir(cm.DataDir, id)

	// Clean up any previous runc state
	cm.runcExec("delete", "--force", id)

	// runc create
	if err := cm.runcExec("create", "-b", bundleDir, id); err != nil {
		state.Status = StatusFailed
		state.ExitCode = -1
		cm.saveState(state)
		return fmt.Errorf("runc create failed: %w", err)
	}

	// Configure networking for isolated network namespace before runc start
	runcStateJSON, err := cm.runcExecOutput("state", id)
	if err == nil {
		var runcSt struct {
			PID int `json:"pid"`
		}
		if json.Unmarshal([]byte(runcStateJSON), &runcSt) == nil && runcSt.PID > 0 {
			state.PID = runcSt.PID
			if !state.HostNetwork {
				ip, netErr := ConfigureContainerNetwork(cm.DataDir, id, runcSt.PID, state.Ports)
				if netErr == nil {
					if state.Env == nil {
						state.Env = make(map[string]string)
					}
					state.Env["ZENO_IP"] = ip
				} else {
					fmt.Printf("  ⚠ Network configuration failed: %v\n", netErr)
				}
			}
		}
	}

	// runc start
	if err := cm.runcExec("start", id); err != nil {
		state.Status = StatusFailed
		cm.saveState(state)
		return fmt.Errorf("runc start failed: %w", err)
	}

	state.Status = StatusRunning
	state.DesiredStatus = StatusRunning
	state.ExitCode = 0
	cm.saveState(state)

	// TCP proxy for port mappings (only for legacy HostNetwork mode)
	if state.HostNetwork && len(state.Ports) > 0 {
		quit := make(chan struct{})
		for _, p := range state.Ports {
			parts := strings.SplitN(p, ":", 2)
			if len(parts) == 2 {
				go StartPortProxy(parts[0], parts[1], quit)
				fmt.Printf("  ▶ TCP proxy on 0.0.0.0:%s\n", parts[0])
			}
		}
		_ = quit
	}
	// Sync hosts entries for name-based communication
	_ = SyncHostsEntries(cm.DataDir)
	return nil
}

// ContainerStop — "runc kill" with SIGTERM, fallback to SIGKILL. Idempotent: returns nil if already stopped.
func (cm *ContainerManager) ContainerStop(id string) error {
	state, err := cm.loadState(id)
	if err != nil {
		return err
	}
	if state.Status != StatusRunning {
		return nil // idempotent — already stopped
	}

	if err := cm.runcExec("kill", id, "SIGTERM"); err != nil {
		cm.runcExec("kill", id, "SIGKILL")
	}

	// Clean up port forwarding rules
	ip := ""
	if state.Env != nil {
		ip = state.Env["ZENO_IP"]
	}
	CleanContainerNetwork(id, ip, state.Ports)

	state.Status = StatusStopped
	state.DesiredStatus = StatusStopped
	err = cm.saveState(state)
	if err == nil {
		_ = SyncHostsEntries(cm.DataDir)
	}
	return err
}

// ContainerDelete — "runc delete --force" + cleanup. Idempotent.
func (cm *ContainerManager) ContainerDelete(id string) error {
	state, err := cm.loadState(id)
	if err == nil {
		ip := ""
		if state.Env != nil {
			ip = state.Env["ZENO_IP"]
		}
		CleanContainerNetwork(id, ip, state.Ports)
	}

	// Best effort: try to clean up runc state
	cm.runcExec("kill", id, "SIGKILL")
	cm.runcExec("delete", "--force", id)
	// Lazily unmount the OverlayFS mountpoint before removing files
	_ = syscall.Unmount(RootfsDir(cm.DataDir, id), syscall.MNT_DETACH)
	// Always try to remove files
	os.RemoveAll(ContainerDir(cm.DataDir, id))

	// Sync hosts entries for name-based communication
	_ = SyncHostsEntries(cm.DataDir)
	return nil
}

// ContainerList — read state files, sync status with runc.
func (cm *ContainerManager) ContainerList() ([]ContainerState, error) {
	entries, err := os.ReadDir(cm.DataDir + "/containers")
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, err
	}
	var list []ContainerState
	for _, e := range entries {
		if e.IsDir() {
			if s, err := cm.loadState(e.Name()); err == nil {
				// Sync status with runc for non-stopped containers
				if s.Status != StatusStopped {
					if st, err := cm.runcExecOutput("state", s.ID); err == nil {
						var runcSt struct {
							Status string `json:"status"`
							PID    int    `json:"pid"`
						}
						if json.Unmarshal([]byte(st), &runcSt) == nil {
							if s.Status != runcSt.Status || s.PID != runcSt.PID {
								s.Status = runcSt.Status
								s.PID = runcSt.PID
								cm.saveState(s)
							}
						}
					} else {
						// runc no longer manages this container
						if s.Status == StatusRunning || s.Status == StatusFailed {
							s.Status = StatusStopped
							cm.saveState(s)
						}
					}
				}
				list = append(list, *s)
			}
		}
	}
	return list, nil
}

// ContainerLogs — read console.log, with optional tail.
func (cm *ContainerManager) ContainerLogs(id string, tail int) ([]string, error) {
	f, err := os.Open(ContainerLogPath(cm.DataDir, id))
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, err
	}
	defer f.Close()

	var lines []string
	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		lines = append(lines, scanner.Text())
	}
	if tail > 0 && len(lines) > tail {
		lines = lines[len(lines)-tail:]
	}
	return lines, scanner.Err()
}

// ContainerInspect — load and return state.
func (cm *ContainerManager) ContainerInspect(id string) (*ContainerState, error) {
	return cm.loadState(id)
}

// ContainerExec — "runc exec" with stdin/stdout passthrough.
func (cm *ContainerManager) ContainerExec(id, command string) error {
	state, err := cm.loadState(id)
	if err != nil {
		return err
	}
	if state.Status != StatusRunning {
		return fmt.Errorf("container %s is not running", id)
	}
	cmd := exec.Command(cm.getRuncBin(), "--root", cm.runcRoot(), "exec", "-t", id, command)
	cmd.Stdin = os.Stdin
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	return cmd.Run()
}

// ContainerStateFromRunc — queries runc state JSON.
func (cm *ContainerManager) ContainerStateFromRunc(id string) (string, error) {
	return cm.runcExecOutput("state", id)
}

// ResolveImageCmd — pull if needed, return default command.
func (cm *ContainerManager) ResolveImageCmd(image string) ([]string, error) {
	ref := parseImageRef(image)
	imgDir := ImageCacheDir(cm.DataDir) + "/" + cacheDirName(ref.Repository, ref.Tag)

	if _, err := os.Stat(imgDir + "/rootfs"); os.IsNotExist(err) {
		fmt.Printf("  ▶ Image %s not found locally. Pulling...\n", image)
		return PullImage(image, cm.DataDir)
	}

	data, err := os.ReadFile(imgDir + "/image-config.json")
	if err != nil {
		return nil, nil
	}
	var imgCfg imageConfig
	if json.Unmarshal(data, &imgCfg) == nil {
		return append(imgCfg.Config.Entrypoint, imgCfg.Config.Cmd...), nil
	}
	return nil, nil
}

func (cm *ContainerManager) ListLocalImages() ([]string, error) { return ListCachedImages(cm.DataDir) }
func (cm *ContainerManager) RemoveLocalImage(image string) error {
	return RemoveCachedImage(image, cm.DataDir)
}

// ContainerUpdate — updates container resource limits dynamically via "runc update".
func (cm *ContainerManager) ContainerUpdate(id string, memoryLimit int64, cpuLimit float64) error {
	state, err := cm.loadState(id)
	if err != nil {
		return err
	}

	// Update active container resource constraints
	var runcArgs []string
	runcArgs = append(runcArgs, "update")

	if memoryLimit > 0 {
		runcArgs = append(runcArgs, "--memory", fmt.Sprintf("%d", memoryLimit))
	}
	if cpuLimit > 0 {
		period := int64(100000)
		quota := int64(cpuLimit * 100000)
		runcArgs = append(runcArgs, "--cpu-period", fmt.Sprintf("%d", period))
		runcArgs = append(runcArgs, "--cpu-quota", fmt.Sprintf("%d", quota))
	}

	runcArgs = append(runcArgs, id)

	// If container is running, call runc update.
	if state.Status == StatusRunning {
		if err := cm.runcExec(runcArgs...); err != nil {
			return fmt.Errorf("runc update failed: %w", err)
		}
	}

	// Update OCI config.json in bundle dir so the limits persist across restarts
	configPath := BundleDir(cm.DataDir, id) + "/config.json"
	if configData, err := os.ReadFile(configPath); err == nil {
		var configMap map[string]interface{}
		if json.Unmarshal(configData, &configMap) == nil {
			var linuxMap map[string]interface{}
			if l, ok := configMap["linux"].(map[string]interface{}); ok {
				linuxMap = l
			} else {
				linuxMap = make(map[string]interface{})
				configMap["linux"] = linuxMap
			}

			var resourcesMap map[string]interface{}
			if r, ok := linuxMap["resources"].(map[string]interface{}); ok {
				resourcesMap = r
			} else {
				resourcesMap = make(map[string]interface{})
				linuxMap["resources"] = resourcesMap
			}

			if memoryLimit > 0 {
				resourcesMap["memory"] = map[string]interface{}{
					"limit": memoryLimit,
				}
			}
			if cpuLimit > 0 {
				period := uint64(100000)
				quota := int64(cpuLimit * 100000)
				resourcesMap["cpu"] = map[string]interface{}{
					"period": period,
					"quota":  quota,
				}
			}

			if newData, err := json.MarshalIndent(configMap, "", "  "); err == nil {
				_ = os.WriteFile(configPath, newData, 0644)
			}
		}
	}

	// Update container state file
	if memoryLimit > 0 {
		state.MemoryLimit = memoryLimit
	}
	if cpuLimit > 0 {
		state.CPULimit = cpuLimit
	}
	return cm.saveState(state)
}

// StartPortProxy — simple TCP forwarder.
func StartPortProxy(hostPort, containerPort string, quit chan struct{}) {
	ln, err := net.Listen("tcp", "0.0.0.0:"+hostPort)
	if err != nil {
		fmt.Fprintf(os.Stderr, "  ⚠ Proxy port %s: %v\n", hostPort, err)
		return
	}
	defer ln.Close()
	for {
		conn, err := ln.Accept()
		if err != nil {
			return
		}
		go func(c net.Conn) {
			defer c.Close()
			target, err := net.DialTimeout("tcp", "127.0.0.1:"+containerPort, 5*time.Second)
			if err != nil {
				return
			}
			defer target.Close()
			go io.Copy(target, c)
			io.Copy(c, target)
		}(conn)
	}
}
