package internal

import (
	"fmt"
	"net"
	"os"
	"os/exec"
	"strings"
	"time"
)

// DaemonState tracks in-memory state for running containers in the daemon
type DaemonState struct {
	LastHealthCheck time.Time
	Failures        int
}

func StartDaemon(cm *ContainerManager, socketPath string) {
	fmt.Printf("Starting Zeno Container Daemon...\n")
	fmt.Printf("Monitoring containers in: %s\n", cm.DataDir)

	if socketPath != "" {
		go func() {
			server := NewAPIServer(cm)
			if err := server.Start(socketPath); err != nil {
				fmt.Fprintf(os.Stderr, "Daemon API Server failed to start: %v\n", err)
			}
		}()
	}

	daemonStates := make(map[string]*DaemonState)

	for {
		time.Sleep(5 * time.Second)

		containers, err := cm.ContainerList()
		if err != nil {
			fmt.Fprintf(os.Stderr, "Daemon Error listing containers: %v\n", err)
			continue
		}

		for _, c := range containers {
			cState := daemonStates[c.ID]
			if cState == nil {
				cState = &DaemonState{}
				daemonStates[c.ID] = cState
			}

			// 1. Handle Restart Policy
			if c.DesiredStatus == StatusRunning && c.Status != StatusRunning {
				// Container is supposed to be running but is stopped/failed
				shouldRestart := false
				policy := strings.ToLower(c.RestartPolicy)

				switch policy {
				case "always":
					shouldRestart = true
				case "unless-stopped":
					shouldRestart = true
				case "on-failure":
					// Restart if exit code is non-zero
					if c.ExitCode != 0 {
						shouldRestart = true
					}
				}

				if shouldRestart {
					fmt.Printf("[Daemon] Restarting container '%s' (policy: %s, exit code: %d)...\n", c.ID, policy, c.ExitCode)
					if err := cm.ContainerStart(c.ID); err != nil {
						fmt.Fprintf(os.Stderr, "[Daemon] Error restarting '%s': %v\n", c.ID, err)
					} else {
						fmt.Printf("[Daemon] Container '%s' successfully restarted.\n", c.ID)
					}
					// Skip health check in this iteration as it's restarting
					continue
				}
			}

			// 2. Handle Health Checks
			if c.Status == StatusRunning && c.HealthCheck != nil && len(c.HealthCheck.Test) > 0 {
				interval := time.Duration(c.HealthCheck.Interval) * time.Second
				if interval == 0 {
					interval = 30 * time.Second
				}

				if time.Since(cState.LastHealthCheck) >= interval {
					cState.LastHealthCheck = time.Now()
					healthy := runHealthCheck(cm, &c)

					// Update health status
					oldStatus := c.HealthStatus
					if healthy {
						cState.Failures = 0
						c.HealthStatus = "healthy"
					} else {
						cState.Failures++
						retries := c.HealthCheck.Retries
						if retries == 0 {
							retries = 3
						}
						if cState.Failures >= retries {
							c.HealthStatus = "unhealthy"
						} else {
							c.HealthStatus = "starting"
						}
					}

					if oldStatus != c.HealthStatus {
						fmt.Printf("[Daemon] Container '%s' health status changed from '%s' to '%s'\n", c.ID, oldStatus, c.HealthStatus)
						_ = cm.saveState(&c)
					}

					if c.HealthStatus == "unhealthy" {
						fmt.Printf("[Daemon] Container '%s' is unhealthy. Restarting for auto-healing...\n", c.ID)
						cState.Failures = 0
						c.HealthStatus = "starting"
						_ = cm.saveState(&c)

						if err := cm.ContainerStop(c.ID); err != nil {
							fmt.Fprintf(os.Stderr, "[Daemon] Error stopping unhealthy container '%s': %v\n", c.ID, err)
						}
						if err := cm.ContainerStart(c.ID); err != nil {
							fmt.Fprintf(os.Stderr, "[Daemon] Error restarting unhealthy container '%s': %v\n", c.ID, err)
						} else {
							fmt.Printf("[Daemon] Unhealthy container '%s' successfully restarted for auto-healing.\n", c.ID)
						}
						continue
					}
				}
			}
		}

		// Clean up daemonStates for deleted containers
		for id := range daemonStates {
			found := false
			for _, c := range containers {
				if c.ID == id {
					found = true
					break
				}
			}
			if !found {
				delete(daemonStates, id)
			}
		}
	}
}

func runHealthCheck(cm *ContainerManager, c *ContainerState) bool {
	test := c.HealthCheck.Test
	if len(test) < 2 {
		return false
	}

	timeout := time.Duration(c.HealthCheck.Timeout) * time.Second
	if timeout == 0 {
		timeout = 5 * time.Second
	}

	// 1. TCP Check
	if strings.ToUpper(test[0]) == "TCP" {
		port := test[1]
		ip := ""
		if c.Env != nil {
			ip = c.Env["ZENO_IP"]
		}
		if ip == "" {
			// Fallback to localhost if using host networking
			ip = "127.0.0.1"
		}
		conn, err := net.DialTimeout("tcp", net.JoinHostPort(ip, port), timeout)
		if err == nil {
			conn.Close()
			return true
		}
		return false
	}

	// 2. CMD / CMD-SHELL Check via runc exec
	var execArgs []string
	if strings.ToUpper(test[0]) == "CMD-SHELL" {
		execArgs = []string{"/bin/sh", "-c", test[1]}
	} else if strings.ToUpper(test[0]) == "CMD" {
		execArgs = test[1:]
	} else {
		// Legacy CMD list support if type is not recognized but it's just raw command array
		execArgs = test
	}

	runcBin := cm.getRuncBin()
	runcRoot := cm.runcRoot()
	args := append([]string{"--root", runcRoot, "exec", c.ID}, execArgs...)

	// Run with a timeout context
	cmd := exec.Command(runcBin, args...)
	
	// Ensure we run it without stdin/stdout to avoid blockages
	cmd.Stdin = nil
	cmd.Stdout = nil
	cmd.Stderr = nil

	done := make(chan error, 1)
	if err := cmd.Start(); err != nil {
		return false
	}

	go func() {
		done <- cmd.Wait()
	}()

	select {
	case <-time.After(timeout):
		if cmd.Process != nil {
			_ = cmd.Process.Kill()
		}
		return false
	case err := <-done:
		return err == nil
	}
}
