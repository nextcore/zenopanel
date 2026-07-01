import { getCSRFToken } from "./utils.js";
import { showToast } from "./toast.js";

// ─── Docker Compose State & Functions ─────────────────────────────────

let activeProject = "default";
let activeFile = "docker-compose.yml";

function composeExec(action) {
  const yaml = document.getElementById("compose-yaml-input").value.trim();
  const projectName = activeProject || "default";

  const resultDiv = document.getElementById("compose-result");
  if (resultDiv) {
    resultDiv.style.display = "block";
    resultDiv.textContent = `Running compose ${action} for project ${projectName}...\n`;
  }

  const csrf = getCSRFToken();

  fetch("/api/containers/compose", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-CSRF-Token": csrf,
    },
    body: JSON.stringify({ action, yaml, project_name: projectName, file: activeFile }),
  })
    .then((res) => res.json())
    .then((res) => {
      if (res.success) {
        if (resultDiv) {
          resultDiv.textContent = res.output || "Done.\n";
          triggerAutoScroll();
        }
        showToast("success", `Compose ${action} completed`);
        import("./containers.js").then((m) => m.loadContainers());
      } else {
        if (resultDiv) {
          resultDiv.textContent = res.error || "Failed.\n";
          triggerAutoScroll();
        }
        showToast("error", res.message || "Compose failed");
      }
    })
    .catch((err) => {
      if (resultDiv) {
        resultDiv.textContent = "Network error.\n";
      }
      showToast("error", "Network error");
    });
}

function triggerAutoScroll() {
  const resultDiv = document.getElementById("compose-result");
  const autoScroll = document.getElementById("compose-console-autoscroll");
  if (resultDiv && autoScroll && autoScroll.checked) {
    resultDiv.scrollTop = resultDiv.scrollHeight;
  }
}

export function composeUp() {
  composeExec("up");
}

export function composeDown() {
  composeExec("down");
}

export function composePs() {
  composeExec("ps");
}

export function loadComposeYaml(projectName, fileName) {
  const name = projectName || activeProject || "default";
  const file = fileName || activeFile || "docker-compose.yml";
  
  fetch(`/api/containers/compose/yaml?project_name=${encodeURIComponent(name)}&file=${encodeURIComponent(file)}`)
    .then((res) => res.json())
    .then((res) => {
      const textarea = document.getElementById("compose-yaml-input");
      if (textarea) {
        textarea.value = res.yaml || "";
      }
    })
    .catch((err) => console.error("Error loading compose YAML:", err));
}

// ─── Project Explorer Functions ───────────────────────────────────────

export function loadComposeProjects() {
  const projectListDiv = document.getElementById("compose-project-list");
  if (!projectListDiv) return;

  fetch("/api/containers/compose/projects")
    .then((res) => res.json())
    .then((res) => {
      if (!res.data || !Array.isArray(res.data)) {
        projectListDiv.innerHTML = `<div style="padding:10px; text-align:center; color:var(--text-muted); font-size:0.8rem;">No projects found</div>`;
        return;
      }

      // Filter folders only
      const projects = res.data.filter(item => item.is_dir);

      if (projects.length === 0) {
        projectListDiv.innerHTML = `<div style="padding:10px; text-align:center; color:var(--text-muted); font-size:0.8rem;">No projects found</div>`;
        return;
      }

      projectListDiv.innerHTML = "";
      projects.forEach(project => {
        const name = project.name;
        const item = document.createElement("div");
        item.className = `project-item ${name === activeProject ? "active" : ""}`;
        item.style.padding = "8px 12px";
        item.style.borderRadius = "6px";
        item.style.cursor = "pointer";
        item.style.fontSize = "0.85rem";
        item.style.display = "flex";
        item.style.alignItems = "center";
        item.style.gap = "8px";
        item.style.color = name === activeProject ? "#fff" : "var(--text-muted)";
        item.style.background = name === activeProject ? "rgba(59, 130, 246, 0.15)" : "transparent";
        item.style.border = name === activeProject ? "1px solid rgba(59, 130, 246, 0.25)" : "1px solid transparent";
        item.style.transition = "all 0.2s ease";

        // Hover styling
        item.addEventListener("mouseenter", () => {
          if (name !== activeProject) {
            item.style.background = "rgba(255, 255, 255, 0.03)";
            item.style.color = "var(--text-main)";
          }
        });
        item.addEventListener("mouseleave", () => {
          if (name !== activeProject) {
            item.style.background = "transparent";
            item.style.color = "var(--text-muted)";
          }
        });

        item.onclick = () => selectComposeProject(name);

        item.innerHTML = `
          <i class="fa-solid fa-folder" style="color: ${name === activeProject ? "var(--accent-primary)" : "var(--warning)"}; font-size: 0.9rem;"></i>
          <span style="font-weight: ${name === activeProject ? "500" : "normal"}; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">${name}</span>
        `;
        projectListDiv.appendChild(item);
      });

      // Synchronize input elements
      const projectInput = document.getElementById("compose-project-input");
      if (projectInput) {
        projectInput.value = activeProject;
      }
      const displaySpan = document.getElementById("active-project-name-display");
      if (displaySpan) {
        displaySpan.textContent = activeProject;
      }
    })
    .catch((err) => {
      console.error("Error loading compose projects:", err);
      projectListDiv.innerHTML = `<div style="padding:10px; text-align:center; color:var(--danger); font-size:0.8rem;">Error loading projects</div>`;
    });
}

export function selectComposeProject(name) {
  activeProject = name;
  loadComposeProjects(); // Re-render lists to update active highlights
  loadComposeYaml(name, activeFile);
}

export function openCreateComposeModal() {
  const modal = document.getElementById("create-compose-modal");
  if (modal) {
    document.getElementById("new-compose-project-name").value = "";
    document.getElementById("new-compose-project-template").value = "custom";
    modal.classList.add("active");
  }
}

export function closeCreateComposeModal() {
  const modal = document.getElementById("create-compose-modal");
  if (modal) {
    modal.classList.remove("active");
  }
}

export function createComposeProject() {
  openCreateComposeModal();
}

export function submitCreateComposeProject() {
  const nameInput = document.getElementById("new-compose-project-name");
  const templateSelect = document.getElementById("new-compose-project-template");
  
  if (!nameInput || !templateSelect) return;
  
  const name = nameInput.value.trim();
  const template = templateSelect.value;
  
  if (!name) {
    showToast("error", "Project name is required");
    return;
  }
  
  const cleanName = name.toLowerCase().replace(/[^a-z0-9-_]/g, "");
  if (!cleanName) {
    showToast("error", "Invalid project name");
    return;
  }

  let defaultYaml = "";
  if (template === "node") {
    defaultYaml = `version: '3.8'\n\nservices:\n  node-app:\n    image: node:20-alpine\n    container_name: ${cleanName}_node_app\n    command: sh -c "cd /app && npm install --production && node server.js"\n    ports:\n      - "3000:3000"\n    volumes:\n      - .:/app\n    environment:\n      NODE_ENV: production\n      PORT: 3000\n    memory: 512m\n    cpus: 0.5\n`;
  } else if (template === "python") {
    defaultYaml = `version: '3.8'\n\nservices:\n  python-app:\n    image: python:3.11-alpine\n    container_name: ${cleanName}_python_app\n    command: sh -c "cd /app && pip install -r requirements.txt && uvicorn main:app --host 0.0.0.0 --port 8000"\n    ports:\n      - "8000:80"\n    volumes:\n      - .:/app\n    environment:\n      PYTHONUNBUFFERED: 1\n      ENV: production\n    memory: 512m\n    cpus: 0.5\n`;
  } else if (template === "laravel") {
    defaultYaml = `version: '3.8'\n\nservices:\n  php-app:\n    image: dunglas/frankenphp:latest-alpine\n    container_name: ${cleanName}_php_app\n    ports:\n      - "8000:80"\n    volumes:\n      - .:/app\n    environment:\n      SERVER_NAME: :80\n      APP_ENV: production\n      APP_DEBUG: 'false'\n    memory: 512m\n    cpus: 0.5\n`;
  } else if (template === "go") {
    defaultYaml = `version: '3.8'\n\nservices:\n  go-app:\n    image: alpine:latest\n    container_name: ${cleanName}_go_app\n    command: sh -c "chmod +x /app/main-binary && /app/main-binary"\n    ports:\n      - "8080:8080"\n    volumes:\n      - .:/app\n    environment:\n      APP_ENV: production\n    memory: 256m\n    cpus: 0.5\n`;
  } else if (template === "ruby") {
    defaultYaml = `version: '3.8'\n\nservices:\n  rails-app:\n    image: ruby:3.2-alpine\n    container_name: ${cleanName}_rails_app\n    command: sh -c "bundle install && rails server -b 0.0.0.0"\n    ports:\n      - "3000:3000"\n    volumes:\n      - .:/app\n    working_dir: /app\n    environment:\n      RAILS_ENV: development\n    memory: 512m\n    cpus: 0.5\n`;
  } else if (template === "java") {
    defaultYaml = `version: '3.8'\n\nservices:\n  spring-app:\n    image: eclipse-temurin:17-jre-alpine\n    container_name: ${cleanName}_spring_app\n    command: java -jar /app/app.jar\n    ports:\n      - "8080:8080"\n    volumes:\n      - .:/app\n    working_dir: /app\n    memory: 1g\n    cpus: 1.0\n`;
  } else if (template === "rust") {
    defaultYaml = `version: '3.8'\n\nservices:\n  rust-app:\n    image: rust:alpine\n    container_name: ${cleanName}_rust_app\n    command: cargo run --release\n    ports:\n      - "8080:8080"\n    volumes:\n      - .:/app\n    working_dir: /app\n    memory: 512m\n    cpus: 0.5\n`;
  } else if (template === "dotnet") {
    defaultYaml = `version: '3.8'\n\nservices:\n  dotnet-app:\n    image: mcr.microsoft.com/dotnet/sdk:8.0-alpine\n    container_name: ${cleanName}_dotnet_app\n    command: dotnet watch run --urls http://0.0.0.0:5000\n    ports:\n      - "5000:5000"\n    volumes:\n      - .:/app\n    working_dir: /app\n    environment:\n      ASPNETCORE_ENVIRONMENT: Development\n    memory: 512m\n    cpus: 0.5\n`;
  } else {
    // Blank/Custom
    defaultYaml = `version: '3.8'\n\nservices:\n  app:\n    image: alpine:latest\n    container_name: ${cleanName}_custom_app\n    command: tail -f /dev/null\n    restart: always\n`;
  }

  closeCreateComposeModal();
  const csrf = getCSRFToken();

  fetch("/api/containers/compose", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-CSRF-Token": csrf,
    },
    body: JSON.stringify({
      action: "save",
      yaml: defaultYaml,
      project_name: cleanName,
      file: "docker-compose.yml"
    }),
  })
    .then((res) => res.json())
    .then((res) => {
      if (res.success) {
        showToast("success", `Project '${cleanName}' created successfully`);
        activeProject = cleanName;
        // Also create empty .env file by default
        return fetch("/api/containers/compose", {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            "X-CSRF-Token": csrf,
          },
          body: JSON.stringify({
            action: "save",
            yaml: "# Environment variables go here\n",
            project_name: cleanName,
            file: ".env"
          })
        });
      } else {
        throw new Error(res.error || "Failed to create project");
      }
    })
    .then((res) => res.json())
    .then(() => {
      loadComposeProjects();
      loadComposeYaml(cleanName, activeFile);
    })
    .catch((err) => {
      console.error("Error creating project:", err);
      showToast("error", err.message || "Failed to create project");
    });
}

export function saveComposeYaml() {
  const yamlTextarea = document.getElementById("compose-yaml-input");
  if (!yamlTextarea) return;

  const content = yamlTextarea.value;
  const csrf = getCSRFToken();

  // Validate .env format if saving .env file
  if (activeFile === ".env") {
    const lines = content.split('\n');
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i].trim();
      if (!line || line.startsWith('#')) continue;

      if (!/^[a-zA-Z_][a-zA-Z0-9_]*=.*$/.test(line)) {
        if (!confirm(`Warning: Line ${i + 1} ("${line}") does not follow the standard KEY=VALUE format. Save anyway?`)) {
          return;
        }
      }
    }
  }

  fetch("/api/containers/compose", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-CSRF-Token": csrf,
    },
    body: JSON.stringify({
      action: "save",
      yaml: content,
      project_name: activeProject,
      file: activeFile
    }),
  })
    .then((res) => res.json())
    .then((res) => {
      if (res.success) {
        showToast("success", `'${activeFile}' saved successfully`);
      } else {
        showToast("error", res.error || `Failed to save ${activeFile}`);
      }
    })
    .catch((err) => {
      console.error("Error saving file:", err);
      showToast("error", "Failed to save file");
    });
}

// ─── Env Editor Tab Switcher ─────────────────────────────────────────

export function switchComposeFileTab(file) {
  activeFile = file;

  const tabYaml = document.getElementById("compose-file-tab-yaml");
  const tabEnv = document.getElementById("compose-file-tab-env");

  if (tabYaml && tabEnv) {
    if (file === "docker-compose.yml") {
      tabYaml.style.background = "rgba(59,130,246,0.1)";
      tabYaml.style.color = "var(--accent-primary)";
      tabYaml.style.borderColor = "rgba(59,130,246,0.25)";

      tabEnv.style.background = "transparent";
      tabEnv.style.color = "var(--text-muted)";
      tabEnv.style.borderColor = "transparent";
    } else {
      tabEnv.style.background = "rgba(59,130,246,0.1)";
      tabEnv.style.color = "var(--accent-primary)";
      tabEnv.style.borderColor = "rgba(59,130,246,0.25)";

      tabYaml.style.background = "transparent";
      tabYaml.style.color = "var(--text-muted)";
      tabYaml.style.borderColor = "transparent";
    }
  }

  loadComposeYaml(activeProject, file);
}

// ─── Delete Project ──────────────────────────────────────────────────

export function deleteComposeProject() {
  if (activeProject === "default") {
    showToast("error", "Cannot delete default project");
    return;
  }

  if (!confirm(`Are you sure you want to delete the project '${activeProject}'? All containers will be stopped and files removed.`)) {
    return;
  }

  const csrf = getCSRFToken();

  // First stop containers
  const resultDiv = document.getElementById("compose-result");
  if (resultDiv) {
    resultDiv.textContent = `Stopping compose project ${activeProject} and deleting files...\n`;
  }

  fetch("/api/containers/compose", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-CSRF-Token": csrf,
    },
    body: JSON.stringify({ action: "down", yaml: "", project_name: activeProject }),
  })
    .then((res) => res.json())
    .then(() => {
      // Delete project folder
      return fetch("/api/containers/compose/delete", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "X-CSRF-Token": csrf,
        },
        body: JSON.stringify({ project_name: activeProject }),
      });
    })
    .then((res) => res.json())
    .then((res) => {
      if (res.success) {
        showToast("success", `Project '${activeProject}' deleted successfully`);
        activeProject = "default";
        activeFile = "docker-compose.yml";
        switchComposeFileTab("docker-compose.yml");
        loadComposeProjects();
        loadComposeYaml("default", "docker-compose.yml");
      } else {
        showToast("error", res.message || "Failed to delete project");
      }
    })
    .catch((err) => {
      console.error("Error deleting project:", err);
      showToast("error", "Failed to delete project");
    });
}

// ─── Proxy & Log Console Utilities ───────────────────────────────────

export function exposeComposeViaProxy() {
  const yamlText = document.getElementById("compose-yaml-input").value;
  
  // Regex to match port pattern - "8080:80" or - 3000:3000
  const portRegex = /-\s*["']?(\d+):(\d+)["']?/g;
  let ports = [];
  let match;
  while ((match = portRegex.exec(yamlText)) !== null) {
    ports.push(parseInt(match[1], 10));
  }

  if (ports.length === 0) {
    showToast("error", "No exposed ports found in YAML configuration");
    return;
  }

  const targetPort = ports[0];
  const nameRegex = /container_name:\s*([a-zA-Z0-9_-]+)/;
  const nameMatch = nameRegex.exec(yamlText);
  const suggestedName = nameMatch ? nameMatch[1] : activeProject;

  // Open proxy modal (which resets inputs)
  if (typeof window.openAddProxyModal === "function") {
    window.openAddProxyModal();
  }

  // Pre-fill values
  const nameInput = document.getElementById('proxy-name');
  const domInput = document.getElementById('proxy-domain');
  const targetInput = document.getElementById('proxy-target');

  if (nameInput) nameInput.value = suggestedName;
  if (domInput) domInput.value = suggestedName + '.local';
  if (targetInput) targetInput.value = `http://127.0.0.1:${targetPort}`;
  
  // Show navigation helper toast
  showToast("info", "Reverse proxy configured with exposed port " + targetPort + ". Switch to the Proxy tab to finalize.");
}

export function clearComposeConsole() {
  const resultDiv = document.getElementById("compose-result");
  if (resultDiv) {
    resultDiv.textContent = "";
  }
}
