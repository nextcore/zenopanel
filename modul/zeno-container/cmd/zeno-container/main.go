package main

import (
	"encoding/json"
	"fmt"
	"os"
	"strconv"
	"strings"

	"github.com/nextcore/zeno-container/internal"
)

func main() {
	dataDir := internal.DefaultDataDir
	var command string
	var cmdArgs []string

	args := os.Args[1:]
	for i := 0; i < len(args); i++ {
		if args[i] == "--data-dir" {
			if i+1 < len(args) {
				dataDir = args[i+1]
				i++
				continue
			}
		}
		if args[i] == "--help" || args[i] == "-h" || args[i] == "help" {
			printUsage()
			return
		}
		if command == "" && !strings.HasPrefix(args[i], "--") && args[i] != "" {
			command = args[i]
		} else if command != "" {
			cmdArgs = append(cmdArgs, args[i])
		}
	}

	if command == "" {
		printUsage()
		os.Exit(1)
	}

	cm := internal.NewContainerManager(dataDir)
	if err := cm.EnsureDirs(); err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}

	switch command {
	case "login":
		cmdLogin(cm, cmdArgs)
	case "pull":
		cmdPull(cm, cmdArgs)
	case "create":
		cmdCreate(cm, cmdArgs)
	case "run":
		cmdRun(cm, cmdArgs)
	case "start":
		cmdStart(cm, cmdArgs)
	case "stop":
		cmdStop(cm, cmdArgs)
	case "rm", "delete":
		cmdDelete(cm, cmdArgs)
	case "ps", "list":
		cmdList(cm, cmdArgs)
	case "images":
		cmdImages(cm, cmdArgs)
	case "rmi":
		cmdRmi(cm, cmdArgs)
	case "inspect":
		cmdInspect(cm, cmdArgs)
	case "compose":
		cmdCompose(cm, cmdArgs)
	case "exec":
		cmdExec(cm, cmdArgs)
	case "logs":
		cmdLogs(cm, cmdArgs)
	case "daemon":
		cmdDaemon(cm, cmdArgs)
	default:
		fmt.Fprintf(os.Stderr, "Unknown command: %s\n\n", command)
		printUsage()
		os.Exit(1)
	}
}

func printUsage() {
	fmt.Print(`zeno-container — Lightweight container manager using runc

Usage:
  zeno-container [--data-dir <path>] <command> [options]

Commands:
  login <registry>          Login to a registry (saves credentials)
  pull <image>              Pull an image from registry (e.g. nginx:alpine)
  create <name>             Create a container from an image
    --image <image>           Required: image name (e.g. nginx:alpine)
    --cmd <command>           Command to run (default: image's default CMD)
    --port <host:container>   Port mapping (can be specified multiple times)
    --env <KEY=value>         Environment variable (can be specified multiple times)
    --volume <host:container>  Volume mount (can be specified multiple times)
    --cwd <path>              Working directory inside container
    --host-net                Use host networking (disables network namespace isolation)
    --restart <policy>        Restart policy (no, always, on-failure)
    --health-cmd <command>    Health check command
    --health-interval <sec>   Health check interval (default: 30)
    --health-timeout <sec>    Health check timeout (default: 5)
    --health-retries <num>    Health check retries (default: 3)
    -m, --memory <limit>      Memory limit (e.g. 512m, 1g)
    --cpus <limit>            CPU limit (fractional cores, e.g. 1.5, 0.5)
  run <id>                  Run container synchronously with log capture
  start <id>                Start a stopped container (detached)
  stop <id>                 Stop a running container
  rm <id>                   Remove a container
  ps                        List all containers
  images                    List cached images
  rmi <image>               Remove a cached image
  inspect <id>              Show detailed container info
  exec <id> <command>       Execute a command in a running container
  logs <id> [--tail <n>]    Show container logs
  daemon                    Run lifecycle orchestrator and health check daemon
  compose up <path>         Create and start all services from a docker-compose.yml
  compose down <path>       Stop and remove all services from a docker-compose.yml
  compose ps <path>         List containers managed by a docker-compose.yml

Global Flags:
  --data-dir <path>         Data directory (default: /var/lib/zeno-container)

Examples:
  zeno-container pull nginx:alpine
  zeno-container create my-web --image nginx:alpine --port 8080:80
  zeno-container create my-app --image node:18-alpine --cmd "node /app/index.js" --env PORT=3000 --port 3000:3000
  zeno-container start my-web
  zeno-container ps
  zeno-container stop my-web
  zeno-container rm my-web
`)
}

func cmdPull(cm *internal.ContainerManager, args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: zeno-container pull <image>")
		os.Exit(1)
	}
	image := args[0]
	fmt.Printf("Pulling image: %s\n", image)
	cmd, err := internal.PullImage(image, cm.DataDir)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
	if len(cmd) > 0 {
		fmt.Printf("Default command: %s\n", strings.Join(cmd, " "))
	}
	fmt.Println("Done.")
}

func cmdLogin(cm *internal.ContainerManager, args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: zeno-container login <registry> [--username <username>] [--password <password>]")
		os.Exit(1)
	}
	registry := args[0]
	var username, password string
	rest := args[1:]

	for i := 0; i < len(rest); i++ {
		switch rest[i] {
		case "--username", "-u":
			if i+1 < len(rest) {
				username = rest[i+1]
				i++
			}
		case "--password", "-p":
			if i+1 < len(rest) {
				password = rest[i+1]
				i++
			}
		}
	}

	if username == "" {
		fmt.Print("Enter Username: ")
		fmt.Scanln(&username)
		username = strings.TrimSpace(username)
	}
	if password == "" {
		fmt.Print("Enter Password: ")
		fmt.Scanln(&password)
		password = strings.TrimSpace(password)
	}

	if username == "" || password == "" {
		fmt.Fprintln(os.Stderr, "Error: Username and Password cannot be empty.")
		os.Exit(1)
	}

	if err := internal.SaveRegistryCredentials(registry, username, password); err != nil {
		fmt.Fprintf(os.Stderr, "Error saving credentials: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("Login Succeeded for registry: %s\n", registry)
}

func cmdCreate(cm *internal.ContainerManager, args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: zeno-container create <name> --image <image> [options]")
		os.Exit(1)
	}
	name := args[0]
	rest := args[1:]
	var image, cmdStr, cwd string
	var hostNet bool
	var restartPolicy string
	var healthCmd string
	healthInterval := 30
	healthTimeout := 5
	healthRetries := 3
	var memoryLimitStr string
	var cpuLimit float64

	for i := 0; i < len(rest); i++ {
		switch rest[i] {
		case "--image":
			if i+1 < len(rest) {
				image = rest[i+1]
				i++
			}
		case "--cmd":
			if i+1 < len(rest) {
				cmdStr = rest[i+1]
				i++
			}
		case "--cwd":
			if i+1 < len(rest) {
				cwd = rest[i+1]
				i++
			}
		case "--host-net":
			hostNet = true
		case "--restart":
			if i+1 < len(rest) {
				restartPolicy = rest[i+1]
				i++
			}
		case "--health-cmd":
			if i+1 < len(rest) {
				healthCmd = rest[i+1]
				i++
			}
		case "--health-interval":
			if i+1 < len(rest) {
				healthInterval, _ = strconv.Atoi(rest[i+1])
				i++
			}
		case "--health-timeout":
			if i+1 < len(rest) {
				healthTimeout, _ = strconv.Atoi(rest[i+1])
				i++
			}
		case "--health-retries":
			if i+1 < len(rest) {
				healthRetries, _ = strconv.Atoi(rest[i+1])
				i++
			}
		case "--memory", "-m":
			if i+1 < len(rest) {
				memoryLimitStr = rest[i+1]
				i++
			}
		case "--cpus":
			if i+1 < len(rest) {
				cpuLimit, _ = strconv.ParseFloat(rest[i+1], 64)
				i++
			}
		}
	}
	if image == "" {
		fmt.Fprintln(os.Stderr, "Error: --image is required")
		os.Exit(1)
	}

	envMap := parseKeyValuePairs(rest, "--env")
	ports := parseListValues(rest, "--port")
	volumes := parseListValues(rest, "--volume")

	cmd, err := cm.ResolveImageCmd(image)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error resolving image: %v\n", err)
		os.Exit(1)
	}
	var finalCmd []string
	if cmdStr != "" {
		finalCmd = strings.Fields(cmdStr)
	} else if len(cmd) > 0 {
		finalCmd = cmd
	} else {
		finalCmd = []string{"/bin/sh"}
	}

	var healthConfig *internal.HealthCheckConfig
	if healthCmd != "" {
		healthConfig = &internal.HealthCheckConfig{
			Test:     []string{"CMD-SHELL", healthCmd},
			Interval: healthInterval,
			Timeout:  healthTimeout,
			Retries:  healthRetries,
		}
	}

	if restartPolicy == "" {
		restartPolicy = "no"
	}

	memoryLimit := parseMemoryBytes(memoryLimitStr)

	fmt.Printf("Creating container '%s' from image '%s'...\n", name, image)
	if err := cm.ContainerCreate(name, image, finalCmd, envMap, cwd, volumes, ports, hostNet, restartPolicy, healthConfig, memoryLimit, cpuLimit); err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("Container '%s' created.\n", name)
}

func cmdRun(cm *internal.ContainerManager, args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: zeno-container run <id>")
		os.Exit(1)
	}
	id := args[0]
	fmt.Printf("Running container '%s' (sync, capturing logs)...\n", id)
	if err := cm.ContainerRun(id); err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
	lines, _ := cm.ContainerLogs(id, 0)
	for _, l := range lines {
		fmt.Println(l)
	}
	fmt.Printf("Container '%s' finished.\n", id)
}

func cmdStart(cm *internal.ContainerManager, args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: zeno-container start <id>")
		os.Exit(1)
	}
	if err := cm.ContainerStart(args[0]); err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("Container '%s' started.\n", args[0])
}

func cmdStop(cm *internal.ContainerManager, args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: zeno-container stop <id>")
		os.Exit(1)
	}
	if err := cm.ContainerStop(args[0]); err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("Container '%s' stopped.\n", args[0])
}

func cmdDelete(cm *internal.ContainerManager, args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: zeno-container rm <id>")
		os.Exit(1)
	}
	if err := cm.ContainerDelete(args[0]); err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("Container '%s' removed.\n", args[0])
}

func cmdList(cm *internal.ContainerManager, args []string) {
	containers, err := cm.ContainerList()
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}

	// JSON output — always works even when empty
	if len(args) > 0 && args[0] == "--json" {
		if containers == nil {
			fmt.Println("[]")
		} else {
			data, _ := json.MarshalIndent(containers, "", "  ")
			fmt.Println(string(data))
		}
		return
	}

	// Table output
	if len(containers) == 0 {
		fmt.Println("No containers.")
		return
	}
	fmt.Printf("%-24s %-16s %-10s %-8s %-12s %s\n", "ID", "IMAGE", "STATUS", "PID", "PORTS", "LOGS")
	fmt.Println(strings.Repeat("-", 100))
	for _, c := range containers {
		ports := strings.Join(c.Ports, ",")
		if ports == "" {
			ports = "-"
		}
		pid := "-"
		if c.PID > 0 {
			pid = fmt.Sprintf("%d", c.PID)
		}
		logs := "no"
		if c.LogPath != "" {
			logs = "yes"
		}
		fmt.Printf("%-24s %-16s %-10s %-8s %-12s %s\n", c.ID, c.Image, c.Status, pid, ports, logs)
	}
}

func cmdImages(cm *internal.ContainerManager, args []string) {
	images, err := cm.ListLocalImages()
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
	if len(images) == 0 {
		fmt.Println("No cached images.")
		return
	}
	fmt.Println("Cached images:")
	for _, img := range images {
		fmt.Printf("  \u2022 %s\n", img)
	}
}

func cmdRmi(cm *internal.ContainerManager, args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: zeno-container rmi <image>")
		os.Exit(1)
	}
	image := args[0]
	if err := cm.RemoveLocalImage(image); err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("Image '%s' removed.\n", image)
}

func cmdInspect(cm *internal.ContainerManager, args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: zeno-container inspect <id>")
		os.Exit(1)
	}
	state, err := cm.ContainerInspect(args[0])
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
	data, _ := json.MarshalIndent(state, "", "  ")
	fmt.Println(string(data))
}

func cmdExec(cm *internal.ContainerManager, args []string) {
	if len(args) < 2 {
		fmt.Fprintln(os.Stderr, "Usage: zeno-container exec <id> <command>")
		os.Exit(1)
	}
	cmdToExec := strings.Join(args[1:], " ")
	if err := cm.ContainerExec(args[0], cmdToExec); err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
	}
}

func cmdCompose(cm *internal.ContainerManager, args []string) {
	if len(args) < 2 {
		fmt.Fprintln(os.Stderr, "Usage: zeno-container compose <up|down|ps> <path>")
		os.Exit(1)
	}
	action := args[0]
	path := args[1]

	switch action {
	case "up":
		results, err := cm.ComposeUp(path)
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}
		fmt.Println()
		fmt.Println("Results:")
		for _, r := range results {
			if r.Error != nil {
				fmt.Printf("  ✗ %s: %v\n", r.Service, r.Error)
			} else {
				fmt.Printf("  ✓ %s: up\n", r.Service)
			}
		}

	case "down":
		if err := cm.ComposeDown(path); err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}
		fmt.Println("All services stopped and removed.")

	case "ps":
		if err := cm.ComposePs(path); err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}

	default:
		fmt.Fprintf(os.Stderr, "Unknown compose action: %s\n\n", action)
		fmt.Fprintln(os.Stderr, "Usage: zeno-container compose <up|down|ps> <path>")
		os.Exit(1)
	}
}

func cmdLogs(cm *internal.ContainerManager, args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: zeno-container logs <id> [--tail <n>]")
		os.Exit(1)
	}
	id := args[0]
	tail := 0
	for i := 1; i < len(args); i++ {
		if args[i] == "--tail" && i+1 < len(args) {
			tail, _ = strconv.Atoi(args[i+1])
			i++
		}
	}
	lines, err := cm.ContainerLogs(id, tail)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
	for _, line := range lines {
		fmt.Println(line)
	}
}

func cmdDaemon(cm *internal.ContainerManager, args []string) {
	internal.StartDaemon(cm)
}

func parseKeyValuePairs(args []string, flag string) map[string]string {
	result := make(map[string]string)
	for i := 0; i < len(args); i++ {
		if args[i] == flag || strings.HasPrefix(args[i], flag+"=") {
			var val string
			if strings.HasPrefix(args[i], flag+"=") {
				val = strings.TrimPrefix(args[i], flag+"=")
			} else if i+1 < len(args) {
				val = args[i+1]
				i++
			}
			if parts := strings.SplitN(val, "=", 2); len(parts) == 2 {
				result[parts[0]] = parts[1]
			}
		}
	}
	return result
}

func parseListValues(args []string, flag string) []string {
	var result []string
	for i := 0; i < len(args); i++ {
		if args[i] == flag || strings.HasPrefix(args[i], flag+"=") {
			var val string
			if strings.HasPrefix(args[i], flag+"=") {
				val = strings.TrimPrefix(args[i], flag+"=")
			} else if i+1 < len(args) {
				val = args[i+1]
				i++
			}
			result = append(result, val)
		}
	}
	return result
}

func parseMemoryBytes(mStr string) int64 {
	if mStr == "" {
		return 0
	}
	mStr = strings.ToLower(strings.TrimSpace(mStr))
	var unit int64 = 1
	if strings.HasSuffix(mStr, "b") {
		mStr = strings.TrimSuffix(mStr, "b")
	}
	if strings.HasSuffix(mStr, "k") {
		unit = 1024
		mStr = strings.TrimSuffix(mStr, "k")
	} else if strings.HasSuffix(mStr, "m") {
		unit = 1024 * 1024
		mStr = strings.TrimSuffix(mStr, "m")
	} else if strings.HasSuffix(mStr, "g") {
		unit = 1024 * 1024 * 1024
		mStr = strings.TrimSuffix(mStr, "g")
	}
	val, err := strconv.ParseInt(mStr, 10, 64)
	if err != nil {
		return 0
	}
	return val * unit
}
