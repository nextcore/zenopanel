import { getCSRFToken } from "./utils.js";
import { showToast } from "./toast.js";

// ─── Docker Compose ──────────────────────────────────────────────────

function composeExec(action) {
  const yaml = document.getElementById("compose-yaml-input").value.trim();
  const projectName = document.getElementById("compose-project-input").value.trim() || "default";

  const resultDiv = document.getElementById("compose-result");
  resultDiv.style.display = "block";
  resultDiv.textContent = `Running compose ${action} for project ${projectName}...\n`;

  const csrf = getCSRFToken();

  fetch("/api/containers/compose", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-CSRF-Token": csrf,
    },
    body: JSON.stringify({ action, yaml, project_name: projectName }),
  })
    .then((res) => res.json())
    .then((res) => {
      if (res.success) {
        resultDiv.textContent = res.output || "Done.\n";
        showToast("success", `Compose ${action} completed`);
        import("./containers.js").then((m) => m.loadContainers());
      } else {
        resultDiv.textContent = res.error || "Failed.\n";
        showToast("error", res.message || "Compose failed");
      }
    })
    .catch((err) => {
      resultDiv.textContent = "Network error.\n";
      showToast("error", "Network error");
    });
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

export function loadComposeYaml() {
  const projectInput = document.getElementById("compose-project-input");
  const projectName = projectInput ? projectInput.value.trim() || "default" : "default";

  fetch(`/api/containers/compose/yaml?project_name=${encodeURIComponent(projectName)}`)
    .then((res) => res.json())
    .then((res) => {
      const textarea = document.getElementById("compose-yaml-input");
      if (textarea) {
        textarea.value = res.yaml || "";
      }
    })
    .catch((err) => console.error("Error loading compose YAML:", err));
}
