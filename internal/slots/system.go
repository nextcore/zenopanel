package slots

import (
	"bytes"
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"sort"
	"strconv"
	"strings"
	"syscall"
	"time"
	"zeno/pkg/engine"
	"zeno/pkg/utils/coerce"
)

func RegisterSystemSlots(eng *engine.Engine) {

	// 1. SYSTEM.INFO
	eng.Register("system.info", func(ctx context.Context, node *engine.Node, scope *engine.Scope) error {
		target := "sys_info"
		for _, c := range node.Children {
			if c.Name == "as" {
				target = strings.TrimPrefix(coerce.ToString(c.Value), "$")
			}
		}

		hostname, _ := os.Hostname()
		cores := runtime.NumCPU()
		uptime := getUptime()
		osVer := getOSVersion()
		cpuModel := getCPUModel()

		info := map[string]interface{}{
			"hostname":  hostname,
			"cores":     cores,
			"uptime":    uptime,
			"os":        osVer,
			"cpu_model": cpuModel,
			"platform":  runtime.GOOS,
			"arch":      runtime.GOARCH,
		}

		scope.Set(target, info)
		return nil
	}, engine.SlotMeta{Example: "system.info { as: $info }"})

	// 2. SYSTEM.STATS
	eng.Register("system.stats", func(ctx context.Context, node *engine.Node, scope *engine.Scope) error {
		target := "sys_stats"
		for _, c := range node.Children {
			if c.Name == "as" {
				target = strings.TrimPrefix(coerce.ToString(c.Value), "$")
			}
		}

		cpu := getCPUUsage()
		memTotal, memFree, memAvailable, memUsed, memPct := getMemoryUsage()
		diskTotal, diskFree, diskUsed, diskPct := getDiskUsage("/")
		netRx, netTx := getNetworkUsage()

		stats := map[string]interface{}{
			"cpu":        cpu,
			"mem_total":  memTotal,
			"mem_free":   memFree,
			"mem_avail":  memAvailable,
			"mem_used":   memUsed,
			"mem_pct":    memPct,
			"disk_total": diskTotal,
			"disk_free":  diskFree,
			"disk_used":  diskUsed,
			"disk_pct":   diskPct,
			"net_rx":     netRx,
			"net_tx":     netTx,
		}

		scope.Set(target, stats)
		return nil
	}, engine.SlotMeta{Example: "system.stats { as: $stats }"})

	// 3. SYSTEM.PROCESSES
	eng.Register("system.processes", func(ctx context.Context, node *engine.Node, scope *engine.Scope) error {
		target := "sys_processes"
		sortBy := "mem" // or "cpu"
		limit := 50

		for _, c := range node.Children {
			val := parseNodeValue(c, scope)
			if c.Name == "as" {
				target = strings.TrimPrefix(coerce.ToString(c.Value), "$")
			}
			if c.Name == "sort" {
				sortBy = coerce.ToString(val)
			}
			if c.Name == "limit" {
				if l, err := coerce.ToInt(val); err == nil {
					limit = l
				}
			}
		}

		if limit <= 0 {
			limit = 50
		}

		procs := getProcessesFromPs(sortBy, limit)
		scope.Set(target, procs)
		return nil
	}, engine.SlotMeta{Example: "system.processes { sort: 'cpu'; limit: 10; as: $procs }"})

	// 4. SYSTEM.KILL
	eng.Register("system.kill", func(ctx context.Context, node *engine.Node, scope *engine.Scope) error {
		var pid int
		target := "kill_success"

		if node.Value != nil {
			if p, err := coerce.ToInt(resolveValue(node.Value, scope)); err == nil {
				pid = p
			}
		}

		for _, c := range node.Children {
			val := parseNodeValue(c, scope)
			if c.Name == "pid" {
				if p, err := coerce.ToInt(val); err == nil {
					pid = p
				}
			}
			if c.Name == "as" {
				target = strings.TrimPrefix(coerce.ToString(c.Value), "$")
			}
		}

		if pid <= 0 {
			return fmt.Errorf("system.kill: invalid or missing pid")
		}

		proc, err := os.FindProcess(pid)
		if err != nil {
			scope.Set(target, false)
			return nil
		}

		err = proc.Signal(syscall.SIGKILL)
		scope.Set(target, err == nil)
		return nil
	}, engine.SlotMeta{Example: "system.kill: $pid"})

	// 5. SYSTEM.EXEC
	eng.Register("system.exec", func(ctx context.Context, node *engine.Node, scope *engine.Scope) error {
		var command string
		target := "exec_result"
		timeoutMs := 15000 // default 15s

		if node.Value != nil {
			command = coerce.ToString(resolveValue(node.Value, scope))
		}

		for _, c := range node.Children {
			val := parseNodeValue(c, scope)
			if c.Name == "cmd" || c.Name == "command" {
				command = coerce.ToString(val)
			}
			if c.Name == "timeout" {
				if t, err := coerce.ToInt(val); err == nil {
					timeoutMs = t
				}
			}
			if c.Name == "as" {
				target = strings.TrimPrefix(coerce.ToString(c.Value), "$")
			}
		}

		if command == "" {
			return fmt.Errorf("system.exec: command is required")
		}

		execCtx, cancel := context.WithTimeout(ctx, time.Duration(timeoutMs)*time.Millisecond)
		defer cancel()

		cmd := exec.CommandContext(execCtx, "bash", "-c", command)
		var stdout, stderr bytes.Buffer
		cmd.Stdout = &stdout
		cmd.Stderr = &stderr

		err := cmd.Run()
		exitCode := 0
		if err != nil {
			if exitError, ok := err.(*exec.ExitError); ok {
				exitCode = exitError.ExitCode()
			} else {
				exitCode = -1
			}
		}

		result := map[string]interface{}{
			"stdout":    stdout.String(),
			"stderr":    stderr.String(),
			"exit_code": exitCode,
			"success":   exitCode == 0,
		}

		scope.Set(target, result)
		return nil
	}, engine.SlotMeta{Example: "system.exec: 'ls -la'\n  as: $res"})

	// 6. SYSTEM.SERVICE_STATUS
	eng.Register("system.service_status", func(ctx context.Context, node *engine.Node, scope *engine.Scope) error {
		var service string
		target := "service_status"

		if node.Value != nil {
			service = coerce.ToString(resolveValue(node.Value, scope))
		}

		for _, c := range node.Children {
			val := parseNodeValue(c, scope)
			if c.Name == "service" {
				service = coerce.ToString(val)
			}
			if c.Name == "as" {
				target = strings.TrimPrefix(coerce.ToString(c.Value), "$")
			}
		}

		if service == "" {
			return fmt.Errorf("system.service_status: service name is required")
		}

		_, err := exec.LookPath("systemctl")
		if err != nil {
			scope.Set(target, map[string]interface{}{
				"service": service,
				"status":  "unknown",
				"active":  false,
				"error":   "systemctl not found on this system",
			})
			return nil
		}

		cmd := exec.Command("systemctl", "is-active", service)
		var out bytes.Buffer
		cmd.Stdout = &out
		cmd.Run() // error is expected if the service is inactive

		status := strings.TrimSpace(out.String())
		scope.Set(target, map[string]interface{}{
			"service": service,
			"status":  status, // "active", "inactive", "failed", etc.
			"active":  status == "active",
		})
		return nil
	}, engine.SlotMeta{Example: "system.service_status: 'docker'\n  as: $status"})

	// 7. SYSTEM.SERVICE_CONTROL
	eng.Register("system.service_control", func(ctx context.Context, node *engine.Node, scope *engine.Scope) error {
		var service, action string
		target := "control_result"

		for _, c := range node.Children {
			val := parseNodeValue(c, scope)
			if c.Name == "service" {
				service = coerce.ToString(val)
			}
			if c.Name == "action" {
				action = coerce.ToString(val) // "start", "stop", "restart"
			}
			if c.Name == "as" {
				target = strings.TrimPrefix(coerce.ToString(c.Value), "$")
			}
		}

		if node.Value != nil {
			service = coerce.ToString(resolveValue(node.Value, scope))
		}

		if service == "" || action == "" {
			return fmt.Errorf("system.service_control: service and action are required")
		}

		// Restrict actions to valid systemctl actions
		validActions := map[string]bool{"start": true, "stop": true, "restart": true, "reload": true, "enable": true, "disable": true}
		if !validActions[action] {
			return fmt.Errorf("system.service_control: invalid action '%s'", action)
		}

		_, err := exec.LookPath("systemctl")
		if err != nil {
			scope.Set(target, map[string]interface{}{
				"success": false,
				"error":   "systemctl not found on this system",
			})
			return nil
		}

		cmd := exec.Command("systemctl", action, service)
		runErr := cmd.Run()

		scope.Set(target, map[string]interface{}{
			"success": runErr == nil,
			"error":   func() string { if runErr != nil { return runErr.Error() }; return "" }(),
		})
		return nil
	}, engine.SlotMeta{Example: "system.service_control: 'docker'\n  action: 'restart'\n  as: $res"})

	// 8. SYSTEM.DIR_LIST
	eng.Register("system.dir_list", func(ctx context.Context, node *engine.Node, scope *engine.Scope) error {
		var path string
		target := "dir_list"

		if node.Value != nil {
			path = coerce.ToString(resolveValue(node.Value, scope))
		}

		for _, c := range node.Children {
			val := parseNodeValue(c, scope)
			if c.Name == "path" {
				path = coerce.ToString(val)
			}
			if c.Name == "as" {
				target = strings.TrimPrefix(coerce.ToString(c.Value), "$")
			}
		}

		if path == "" {
			path = "."
		}

		files, err := os.ReadDir(path)
		if err != nil {
			return err
		}

		var list []map[string]interface{}
		for _, f := range files {
			info, err := f.Info()
			if err != nil {
				continue
			}
			list = append(list, map[string]interface{}{
				"name":     f.Name(),
				"is_dir":   f.IsDir(),
				"size":     info.Size(),
				"mod_time": info.ModTime().Format(time.RFC3339),
				"mode":     info.Mode().String(),
			})
		}

		sort.Slice(list, func(i, j int) bool {
			if list[i]["is_dir"].(bool) != list[j]["is_dir"].(bool) {
				return list[i]["is_dir"].(bool)
			}
			return list[i]["name"].(string) < list[j]["name"].(string)
		})

		scope.Set(target, list)
		return nil
	}, engine.SlotMeta{Example: "system.dir_list: '/var/www'\n  as: $files"})
}

// --- SYSTEM STATISTICS HELPERS ---

func getCPUUsage() float64 {
	data, err := os.ReadFile("/proc/stat")
	if err != nil {
		return 0.0
	}
	lines := strings.Split(string(data), "\n")
	if len(lines) == 0 {
		return 0.0
	}
	fields := strings.Fields(lines[0])
	if len(fields) < 5 {
		return 0.0
	}
	user, _ := strconv.ParseUint(fields[1], 10, 64)
	nice, _ := strconv.ParseUint(fields[2], 10, 64)
	system, _ := strconv.ParseUint(fields[3], 10, 64)
	idle, _ := strconv.ParseUint(fields[4], 10, 64)
	iowait, _ := strconv.ParseUint(fields[5], 10, 64)
	irq, _ := strconv.ParseUint(fields[6], 10, 64)
	softirq, _ := strconv.ParseUint(fields[7], 10, 64)

	total := user + nice + system + idle + iowait + irq + softirq
	active := total - idle - iowait

	time.Sleep(100 * time.Millisecond)

	data2, err := os.ReadFile("/proc/stat")
	if err != nil {
		return 0.0
	}
	lines2 := strings.Split(string(data2), "\n")
	if len(lines2) == 0 {
		return 0.0
	}
	fields2 := strings.Fields(lines2[0])
	if len(fields2) < 5 {
		return 0.0
	}
	user2, _ := strconv.ParseUint(fields2[1], 10, 64)
	nice2, _ := strconv.ParseUint(fields2[2], 10, 64)
	system2, _ := strconv.ParseUint(fields2[3], 10, 64)
	idle2, _ := strconv.ParseUint(fields2[4], 10, 64)
	iowait2, _ := strconv.ParseUint(fields2[5], 10, 64)
	irq2, _ := strconv.ParseUint(fields2[6], 10, 64)
	softirq2, _ := strconv.ParseUint(fields2[7], 10, 64)

	total2 := user2 + nice2 + system2 + idle2 + iowait2 + irq2 + softirq2
	active2 := total2 - idle2 - iowait2

	if total2-total == 0 {
		return 0.0
	}

	return (float64(active2-active) / float64(total2-total)) * 100.0
}

func getMemoryUsage() (total, free, available, used, percent float64) {
	data, err := os.ReadFile("/proc/meminfo")
	if err != nil {
		return
	}
	lines := strings.Split(string(data), "\n")
	var memTotal, memFree, memAvailable, buffers, cached uint64
	for _, line := range lines {
		fields := strings.Fields(line)
		if len(fields) < 3 {
			continue
		}
		name := strings.TrimSuffix(fields[0], ":")
		val, _ := strconv.ParseUint(fields[1], 10, 64)
		switch name {
		case "MemTotal":
			memTotal = val
		case "MemFree":
			memFree = val
		case "MemAvailable":
			memAvailable = val
		case "Buffers":
			buffers = val
		case "Cached":
			cached = val
		}
	}
	if memTotal == 0 {
		return
	}
	var memUsed uint64
	if memAvailable > 0 {
		memUsed = memTotal - memAvailable
	} else {
		memUsed = memTotal - memFree - buffers - cached
	}
	total = float64(memTotal) * 1024
	used = float64(memUsed) * 1024
	free = float64(memTotal-memUsed) * 1024
	available = float64(memAvailable) * 1024
	percent = (used / total) * 100.0
	return
}

func getDiskUsage(path string) (total, free, used, percent float64) {
	var stat syscall.Statfs_t
	err := syscall.Statfs(path, &stat)
	if err != nil {
		return
	}
	total = float64(stat.Blocks) * float64(stat.Bsize)
	free = float64(stat.Bfree) * float64(stat.Bsize)
	used = total - free
	if total > 0 {
		percent = (used / total) * 100.0
	}
	return
}

func getNetworkUsage() (rx, tx uint64) {
	data, err := os.ReadFile("/proc/net/dev")
	if err != nil {
		return
	}
	lines := strings.Split(string(data), "\n")
	for _, line := range lines {
		if !strings.Contains(line, ":") {
			continue
		}
		parts := strings.SplitN(line, ":", 2)
		if len(parts) < 2 {
			continue
		}
		iface := strings.TrimSpace(parts[0])
		if iface == "lo" {
			continue
		}
		fields := strings.Fields(parts[1])
		if len(fields) < 9 {
			continue
		}
		rBytes, _ := strconv.ParseUint(fields[0], 10, 64)
		tBytes, _ := strconv.ParseUint(fields[8], 10, 64)
		rx += rBytes
		tx += tBytes
	}
	return
}

func getUptime() string {
	data, err := os.ReadFile("/proc/uptime")
	if err != nil {
		return "Unknown"
	}
	fields := strings.Fields(string(data))
	if len(fields) < 1 {
		return "Unknown"
	}
	uptimeSec, err := strconv.ParseFloat(fields[0], 64)
	if err != nil {
		return "Unknown"
	}
	duration := time.Duration(uptimeSec) * time.Second
	days := int(duration.Hours() / 24)
	hours := int(duration.Hours()) % 24
	minutes := int(duration.Minutes()) % 60
	if days > 0 {
		return fmt.Sprintf("%d days, %d hours, %d mins", days, hours, minutes)
	}
	if hours > 0 {
		return fmt.Sprintf("%d hours, %d mins", hours, minutes)
	}
	return fmt.Sprintf("%d mins", minutes)
}

func getOSVersion() string {
	data, err := os.ReadFile("/etc/os-release")
	if err == nil {
		lines := strings.Split(string(data), "\n")
		for _, line := range lines {
			if strings.HasPrefix(line, "PRETTY_NAME=") {
				return strings.Trim(strings.TrimPrefix(line, "PRETTY_NAME="), "\"")
			}
		}
	}
	return runtime.GOOS + " " + runtime.GOARCH
}

func getCPUModel() string {
	data, err := os.ReadFile("/proc/cpuinfo")
	if err == nil {
		lines := strings.Split(string(data), "\n")
		for _, line := range lines {
			if strings.HasPrefix(line, "model name") {
				parts := strings.Split(line, ":")
				if len(parts) > 1 {
					return strings.TrimSpace(parts[1])
				}
			}
		}
	}
	return runtime.GOARCH
}

// --- PROCESS MANAGER HELPERS ---

type ProcessInfo struct {
	PID    int     `json:"pid"`
	Name   string  `json:"name"`
	CPU    float64 `json:"cpu"`
	Memory float64 `json:"memory"`
	Status string  `json:"status"`
}

func getProcessesFromPs(sortBy string, limit int) []ProcessInfo {
	sortOpt := "--sort=-%mem"
	if sortBy == "cpu" {
		sortOpt = "--sort=-%cpu"
	}

	cmd := exec.Command("ps", "axo", "pid,%cpu,%mem,stat,comm", sortOpt)
	var out bytes.Buffer
	cmd.Stdout = &out
	err := cmd.Run()
	if err != nil {
		// Fallback to minimal listing if ps fails or is not available
		return getProcessesFallback(limit)
	}

	lines := strings.Split(out.String(), "\n")
	var list []ProcessInfo
	for i, line := range lines {
		if i == 0 || strings.TrimSpace(line) == "" {
			continue
		}
		fields := strings.Fields(line)
		if len(fields) < 5 {
			continue
		}
		pid, _ := strconv.Atoi(fields[0])
		cpu, _ := strconv.ParseFloat(fields[1], 64)
		mem, _ := strconv.ParseFloat(fields[2], 64)
		status := fields[3]
		name := strings.Join(fields[4:], " ")

		list = append(list, ProcessInfo{
			PID:    pid,
			Name:   name,
			CPU:    cpu,
			Memory: mem,
			Status: status,
		})
	}

	if len(list) > limit {
		return list[:limit]
	}
	return list
}

func getProcessesFallback(limit int) []ProcessInfo {
	files, err := os.ReadDir("/proc")
	if err != nil {
		return nil
	}
	var list []ProcessInfo
	memTotal, _, _, _, _ := getMemoryUsage()

	for _, file := range files {
		if !file.IsDir() {
			continue
		}
		pid, err := strconv.Atoi(file.Name())
		if err != nil {
			continue
		}

		statBytes, err := os.ReadFile(filepath.Join("/proc", file.Name(), "stat"))
		if err != nil {
			continue
		}

		statStr := string(statBytes)
		openParen := strings.Index(statStr, "(")
		closeParen := strings.LastIndex(statStr, ")")
		if openParen == -1 || closeParen == -1 || closeParen <= openParen {
			continue
		}
		name := statStr[openParen+1 : closeParen]
		afterName := statStr[closeParen+2:]
		fields := strings.Fields(afterName)

		if len(fields) < 22 {
			continue
		}

		state := fields[0]
		rssPages, _ := strconv.ParseUint(fields[21], 10, 64)
		rssBytes := rssPages * 4096

		var memPct float64
		if memTotal > 0 {
			memPct = (float64(rssBytes) / memTotal) * 100.0
		}

		list = append(list, ProcessInfo{
			PID:    pid,
			Name:   name,
			CPU:    0.0,
			Memory: memPct,
			Status: state,
		})
	}

	sort.Slice(list, func(i, j int) bool {
		return list[i].Memory > list[j].Memory
	})

	if len(list) > limit {
		return list[:limit]
	}
	return list
}
