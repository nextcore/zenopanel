# Implementation Plan - File Upload Feature in File Manager

Enable users to upload files directly into their active directory inside the ZenoPanel File Manager interface.

## User Review Required

> [!IMPORTANT]
> The backend route `/api/files/upload` will be implemented natively in Axum to handle multipart stream uploads, allowing for efficient streaming and bypasses ZenoLang limits.
> We will enable the `multipart` feature for `axum` in `Cargo.toml`.

## Proposed Changes

### Cargo Configuration

#### [MODIFY] [Cargo.toml](file:///home/max/Documents/PROJ/github/zenopanel/Cargo.toml)
* Enable `multipart` feature for `axum`.

### Backend Routing & Logic

#### [MODIFY] [src/main.rs](file:///home/max/Documents/PROJ/github/zenopanel/src/main.rs)
* Add a new native Axum route handler `upload_file_handler` for POST `/api/files/upload`.
* Limit check: disable standard body limits (`DefaultBodyLimit::disable()`) for the upload route to support large files.
* Multipart stream handling: extract the destination `path` and file data from the multipart body, construct the full file destination path, and stream write the bytes to disk.

### Frontend UI

#### [MODIFY] [views/partials/tab_files.blade.zl](file:///home/max/Documents/PROJ/github/zenopanel/views/partials/tab_files.blade.zl)
* Add an **Upload File** button to the toolbar.
* Add a hidden `<input type="file">` selector for selecting multiple files.

#### [MODIFY] [views/partials/js.blade.zl](file:///home/max/Documents/PROJ/github/zenopanel/views/partials/js.blade.zl)
* Implement `triggerFileUpload()` to open the file picker.
* Implement `handleFileUpload(event)` to submit selected files to `/api/files/upload` via `FormData`, track progress, show UI toasts, and reload the current directory list upon success.

---

## Verification Plan

### Automated Tests
* We will update `scratch/verify.py` to:
  * Create a mock text file.
  * Send a POST request to `/api/files/upload` with the mock file using multipart/form-data.
  * Check that the file is successfully created in the workspace.
  * Clean up the uploaded file.

### Manual Verification
* Run the server and check the file upload button and upload functionality in the File Manager interface.


Progress:

- `[x]` 1. Enable `multipart` feature for `axum` in `Cargo.toml`
- `[x]` 2. Add native Axum endpoint `/api/files/upload` in `src/main.rs`
- `[x]` 3. Add file upload UI button and input in `views/partials/tab_files.blade.zl`
- `[/]` 4. Implement `triggerFileUpload` and `handleFileUpload` Javascript in `views/partials/js.blade.zl`
- `[ ]` 5. Verify the multipart upload endpoint compiles and works correctly via curl/test script

