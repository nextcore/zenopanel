package internal

import (
	"encoding/json"
	"fmt"
	"net"
	"net/http"
	"os"
	"strings"
)

type APIServer struct {
	cm *ContainerManager
}

func NewAPIServer(cm *ContainerManager) *APIServer {
	return &APIServer{cm: cm}
}

func (s *APIServer) Start(socketPath string) error {
	// Clean up socket file if it exists
	if err := os.Remove(socketPath); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("failed to remove existing socket: %w", err)
	}

	listener, err := net.Listen("unix", socketPath)
	if err != nil {
		return fmt.Errorf("failed to listen on unix socket %s: %w", socketPath, err)
	}

	// Make socket writeable by anyone so external tools can connect without sudo
	if err := os.Chmod(socketPath, 0666); err != nil {
		listener.Close()
		return fmt.Errorf("failed to set socket permissions: %w", err)
	}

	mux := http.NewServeMux()
	mux.HandleFunc("/", s.handleRequest)

	server := &http.Server{
		Handler: mux,
	}

	fmt.Printf("[API Server] Listening on UNIX Socket: %s\n", socketPath)
	return server.Serve(listener)
}

func (s *APIServer) handleRequest(w http.ResponseWriter, r *http.Request) {
	// Set JSON response header by default
	w.Header().Set("Content-Type", "application/json")

	path := r.URL.Path
	method := r.Method

	// 1. GET /_ping or /info
	if (path == "/_ping" || path == "/info") && method == "GET" {
		w.WriteHeader(http.StatusOK)
		json.NewEncoder(w).Encode(map[string]interface{}{
			"status":  "OK",
			"version": "0.5.0",
			"engine":  "zeno-container",
		})
		return
	}

	// 2. GET /images/json
	if path == "/images/json" && method == "GET" {
		images, err := s.cm.ListLocalImages()
		if err != nil {
			s.writeError(w, http.StatusInternalServerError, err.Error())
			return
		}
		// Return list of images matching Docker schema format
		dockerImages := []map[string]interface{}{}
		for _, img := range images {
			dockerImages = append(dockerImages, map[string]interface{}{
				"RepoTags": []string{img},
				"Id":       img,
			})
		}
		w.WriteHeader(http.StatusOK)
		json.NewEncoder(w).Encode(dockerImages)
		return
	}

	// 3. GET /containers/json
	if path == "/containers/json" && method == "GET" {
		containers, err := s.cm.ContainerList()
		if err != nil {
			s.writeError(w, http.StatusInternalServerError, err.Error())
			return
		}
		// Format to resemble Docker API JSON
		dockerContainers := []map[string]interface{}{}
		for _, c := range containers {
			var stateStr = "exited"
			if c.Status == StatusRunning {
				stateStr = "running"
			}
			dockerContainers = append(dockerContainers, map[string]interface{}{
				"Id":      c.ID,
				"Names":   []string{"/" + c.ID},
				"Image":   c.Image,
				"State":   stateStr,
				"Status":  c.Status,
				"Ports":   c.Ports,
				"Command": strings.Join(c.Cmd, " "),
			})
		}
		w.WriteHeader(http.StatusOK)
		json.NewEncoder(w).Encode(dockerContainers)
		return
	}

	// 4. POST /containers/create
	if path == "/containers/create" && method == "POST" {
		name := r.URL.Query().Get("name")
		if name == "" {
			s.writeError(w, http.StatusBadRequest, "Missing name query parameter")
			return
		}

		var req struct {
			Image      string   `json:"Image"`
			Cmd        []string `json:"Cmd"`
			Env        []string `json:"Env"`
			WorkingDir string   `json:"WorkingDir"`
			HostConfig struct {
				PortBindings map[string][]struct {
					HostPort string `json:"HostPort"`
				} `json:"PortBindings"`
				Binds       []string `json:"Binds"`
				Memory      int64    `json:"Memory"`
				NanoCPUs    int64    `json:"NanoCPUs"`
				NetworkMode string   `json:"NetworkMode"`
			} `json:"HostConfig"`
		}

		if r.ContentLength > 0 {
			if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
				s.writeError(w, http.StatusBadRequest, "Invalid request JSON: "+err.Error())
				return
			}
		}

		if req.Image == "" {
			s.writeError(w, http.StatusBadRequest, "Missing Image field")
			return
		}

		// Convert env slice ["K=V"] to map
		envMap := make(map[string]string)
		for _, envStr := range req.Env {
			parts := strings.SplitN(envStr, "=", 2)
			if len(parts) == 2 {
				envMap[parts[0]] = parts[1]
			}
		}

		// Convert Docker Binds to volumes slice ["host:container"]
		var volumes []string
		for _, bind := range req.HostConfig.Binds {
			volumes = append(volumes, bind)
		}

		// Convert PortBindings to ports slice ["host:container"]
		var ports []string
		for containerPortProto, bindings := range req.HostConfig.PortBindings {
			containerPort := strings.Split(containerPortProto, "/")[0]
			for _, binding := range bindings {
				ports = append(ports, fmt.Sprintf("%s:%s", binding.HostPort, containerPort))
			}
		}

		// CPUs
		cpuLimit := float64(req.HostConfig.NanoCPUs) / 1e9

		// Try resolving default cmd from image if req.Cmd is empty
		finalCmd := req.Cmd
		if len(finalCmd) == 0 {
			resolvedCmd, err := s.cm.ResolveImageCmd(req.Image)
			if err == nil && len(resolvedCmd) > 0 {
				finalCmd = resolvedCmd
			} else {
				finalCmd = []string{"/bin/sh"}
			}
		}

		// Create container
		err := s.cm.ContainerCreate(
			name,
			req.Image,
			finalCmd,
			envMap,
			req.WorkingDir,
			volumes,
			ports,
			false,    // hostNet
			"always", // restartPolicy
			nil,      // healthConfig
			req.HostConfig.Memory,
			cpuLimit,
			nil,   // oomScoreAdj
			false, // readOnly
			req.HostConfig.NetworkMode, // network
		)

		if err != nil {
			s.writeError(w, http.StatusInternalServerError, err.Error())
			return
		}

		w.WriteHeader(http.StatusCreated)
		json.NewEncoder(w).Encode(map[string]interface{}{
			"Id":       name,
			"Warnings": []string{},
		})
		return
	}

	// 5. GET /networks
	if path == "/networks" && method == "GET" {
		list := []map[string]interface{}{
			{
				"Name":       "bridge",
				"Id":         "zenobr0",
				"Scope":      "local",
				"Driver":     "bridge",
				"EnableIPv6": false,
				"IPAM": map[string]interface{}{
					"Driver": "default",
					"Config": []map[string]interface{}{
						{
							"Subnet":  "172.20.0.0/16",
							"Gateway": "172.20.0.1",
						},
					},
				},
			},
		}

		customNets, err := LoadNetworks(s.cm.DataDir)
		if err == nil {
			for _, n := range customNets {
				list = append(list, map[string]interface{}{
					"Name":       n.Name,
					"Id":         n.ID,
					"Scope":      "local",
					"Driver":     n.Driver,
					"EnableIPv6": false,
					"IPAM": map[string]interface{}{
						"Driver": "default",
						"Config": []map[string]interface{}{
							{
								"Subnet":  n.Subnet,
								"Gateway": n.Gateway,
							},
						},
					},
				})
			}
		}

		w.WriteHeader(http.StatusOK)
		json.NewEncoder(w).Encode(list)
		return
	}

	// 6. POST /networks/create
	if path == "/networks/create" && method == "POST" {
		var req struct {
			Name string `json:"Name"`
		}
		_ = json.NewDecoder(r.Body).Decode(&req)
		if req.Name == "" {
			s.writeError(w, http.StatusBadRequest, "Missing Name field")
			return
		}

		if err := CreateBridgeNetwork(s.cm.DataDir, req.Name); err != nil {
			s.writeError(w, http.StatusInternalServerError, err.Error())
			return
		}

		customNets, err := LoadNetworks(s.cm.DataDir)
		id := ""
		if err == nil {
			for _, n := range customNets {
				if n.Name == req.Name {
					id = n.ID
					break
				}
			}
		}

		w.WriteHeader(http.StatusCreated)
		json.NewEncoder(w).Encode(map[string]interface{}{
			"Id":      id,
			"Warning": "",
		})
		return
	}

	// 7. GET /volumes
	if path == "/volumes" && method == "GET" {
		volumesDir := s.cm.DataDir + "/volumes"
		_ = os.MkdirAll(volumesDir, 0755)

		files, err := os.ReadDir(volumesDir)
		list := []map[string]interface{}{}
		if err == nil {
			for _, file := range files {
				if file.IsDir() {
					name := file.Name()
					list = append(list, map[string]interface{}{
						"Name":       name,
						"Driver":     "local",
						"Mountpoint": volumesDir + "/" + name,
					})
				}
			}
		}

		w.WriteHeader(http.StatusOK)
		json.NewEncoder(w).Encode(map[string]interface{}{
			"Volumes":  list,
			"Warnings": []string{},
		})
		return
	}

	// 8. POST /volumes/create
	if path == "/volumes/create" && method == "POST" {
		var req struct {
			Name string `json:"Name"`
		}
		_ = json.NewDecoder(r.Body).Decode(&req)
		if req.Name == "" {
			s.writeError(w, http.StatusBadRequest, "Missing Name field")
			return
		}

		volumesDir := s.cm.DataDir + "/volumes"
		volPath := volumesDir + "/" + req.Name
		if err := os.MkdirAll(volPath, 0755); err != nil {
			s.writeError(w, http.StatusInternalServerError, "Failed to create volume folder: "+err.Error())
			return
		}

		w.WriteHeader(http.StatusCreated)
		json.NewEncoder(w).Encode(map[string]interface{}{
			"Name":       req.Name,
			"Driver":     "local",
			"Mountpoint": volPath,
		})
		return
	}

	// 9. DELETE /volumes/{name}
	if strings.HasPrefix(path, "/volumes/") && method == "DELETE" {
		name := strings.TrimPrefix(path, "/volumes/")
		if name != "" {
			volPath := s.cm.DataDir + "/volumes/" + name
			_ = os.RemoveAll(volPath)
			w.WriteHeader(http.StatusNoContent)
			return
		}
	}

	// 10. GET or DELETE /networks/{id}
	if strings.HasPrefix(path, "/networks/") {
		id := strings.TrimPrefix(path, "/networks/")
		if id != "" {
			if method == "DELETE" {
				if id == "zenobr0" || id == "bridge" || id == "default" {
					s.writeError(w, http.StatusForbidden, "Cannot delete default bridge network")
					return
				}
				if err := DeleteBridgeNetwork(s.cm.DataDir, id); err != nil {
					s.writeError(w, http.StatusInternalServerError, err.Error())
					return
				}
				w.WriteHeader(http.StatusNoContent)
				return
			}
			if method == "GET" {
				if id == "zenobr0" || id == "bridge" || id == "default" {
					w.WriteHeader(http.StatusOK)
					json.NewEncoder(w).Encode(map[string]interface{}{
						"Name":   "bridge",
						"Id":     "zenobr0",
						"Driver": "bridge",
					})
					return
				}

				customNets, err := LoadNetworks(s.cm.DataDir)
				if err == nil {
					for _, n := range customNets {
						if n.ID == id || n.Name == id {
							w.WriteHeader(http.StatusOK)
							json.NewEncoder(w).Encode(map[string]interface{}{
								"Name":   n.Name,
								"Id":     n.ID,
								"Driver": n.Driver,
							})
							return
						}
					}
				}
				s.writeError(w, http.StatusNotFound, "Network not found")
				return
			}
		}
	}

	// Endpoints with dynamic container ID:
	// - /containers/{id}/json (GET)
	// - /containers/{id}/start (POST)
	// - /containers/{id}/stop (POST)
	// - /containers/{id}/logs (GET)
	// - /containers/{id} (DELETE)
	if strings.HasPrefix(path, "/containers/") {
		parts := strings.Split(strings.TrimPrefix(path, "/containers/"), "/")
		if len(parts) > 0 && parts[0] != "" {
			id := parts[0]

			// 5. GET /containers/{id}/json (Inspect)
			if len(parts) == 1 && method == "GET" {
				state, err := s.cm.ContainerInspect(id)
				if err != nil {
					s.writeError(w, http.StatusNotFound, err.Error())
					return
				}
				w.WriteHeader(http.StatusOK)
				json.NewEncoder(w).Encode(state)
				return
			}

			// 6. POST /containers/{id}/start
			if len(parts) == 2 && parts[1] == "start" && method == "POST" {
				if err := s.cm.ContainerStart(id); err != nil {
					s.writeError(w, http.StatusInternalServerError, err.Error())
					return
				}
				w.WriteHeader(http.StatusNoContent)
				return
			}

			// 7. POST /containers/{id}/stop
			if len(parts) == 2 && parts[1] == "stop" && method == "POST" {
				if err := s.cm.ContainerStop(id); err != nil {
					s.writeError(w, http.StatusInternalServerError, err.Error())
					return
				}
				w.WriteHeader(http.StatusNoContent)
				return
			}

			// 8. GET /containers/{id}/logs
			if len(parts) == 2 && parts[1] == "logs" && method == "GET" {
				lines, err := s.cm.ContainerLogs(id, 0)
				if err != nil {
					s.writeError(w, http.StatusInternalServerError, err.Error())
					return
				}
				w.Header().Set("Content-Type", "text/plain")
				w.WriteHeader(http.StatusOK)
				for _, line := range lines {
					w.Write([]byte(line + "\n"))
				}
				return
			}

			// 9. DELETE /containers/{id}
			if len(parts) == 1 && method == "DELETE" {
				if err := s.cm.ContainerDelete(id); err != nil {
					s.writeError(w, http.StatusInternalServerError, err.Error())
					return
				}
				w.WriteHeader(http.StatusNoContent)
				return
			}
		}
	}

	w.WriteHeader(http.StatusNotFound)
	json.NewEncoder(w).Encode(map[string]string{
		"message": "Endpoint not found",
	})
}

func (s *APIServer) writeError(w http.ResponseWriter, status int, msg string) {
	w.WriteHeader(status)
	json.NewEncoder(w).Encode(map[string]string{
		"message": msg,
	})
}
