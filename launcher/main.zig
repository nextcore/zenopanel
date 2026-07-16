const std = @import("std");
const builtin = @import("builtin");
const Allocator = std.mem.Allocator;

const DISTRO_NAME = "zenopanel";
const PORT = "3001";
const PANEL_URL = "http://localhost:3001/login";

// Versi default yang diselaraskan dengan tag rilis
const VERSION = "v1.5.19";

// Win32 API declarations untuk target Windows
const win = if (builtin.os.tag == .windows) struct {
    pub const HWND = *anyopaque;
    pub const UINT = u32;
    pub const INT = i32;

    pub const MB_OK: u32 = 0x00000000;
    pub const MB_OKCANCEL: u32 = 0x00000001;
    pub const MB_ICONHAND: u32 = 0x00000010;
    pub const MB_ICONQUESTION: u32 = 0x00000020;
    pub const MB_ICONEXCLAMATION: u32 = 0x00000030;
    pub const MB_ICONASTERISK: u32 = 0x00000040;
    pub const IDOK: i32 = 1;

    pub extern "user32" fn MessageBoxW(
        hWnd: ?HWND,
        lpText: [*:0]const u16,
        lpCaption: [*:0]const u16,
        uType: UINT,
    ) callconv(.winapi) INT;
} else struct {};

fn showMessageBox(text: []const u8, caption: []const u8, utype: u32) !i32 {
    if (builtin.os.tag == .windows) {
        var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
        defer arena.deinit();
        const allocator = arena.allocator();

        const w_text = try std.unicode.utf8ToUtf16LeAllocZ(allocator, text);
        const w_caption = try std.unicode.utf8ToUtf16LeAllocZ(allocator, caption);

        return win.MessageBoxW(null, w_text.ptr, w_caption.ptr, utype);
    } else {
        std.debug.print("[MessageBox] {s}: {s}\n", .{ caption, text });
        return 1; // IDOK
    }
}

fn isPortActive(port_str: []const u8) bool {
    const port = std.fmt.parseInt(u16, port_str, 10) catch return false;
    const address = std.net.Address.parseIp4("127.0.0.1", port) catch return false;
    var stream = std.net.tcpConnectToAddress(address) catch return false;
    stream.close();
    return true;
}

fn openBrowser(allocator: Allocator, url: []const u8) !void {
    if (builtin.os.tag == .windows) {
        var child = std.process.Child.init(&[_][]const u8{ "cmd.exe", "/C", "start", "", url }, allocator);
        _ = try child.spawn();
    } else {
        std.debug.print("Opening browser: {s}\n", .{url});
    }
}

fn downloadFile(allocator: Allocator, url_str: []const u8, dest_path: []const u8) !void {
    var client = std.http.Client{ .allocator = allocator };
    defer client.deinit();

    const uri = try std.Uri.parse(url_str);
    var req = try client.request(.GET, uri, .{});
    defer req.deinit();

    try req.sendBodiless();

    var redirect_buffer: [8192]u8 = undefined;
    var response = try req.receiveHead(&redirect_buffer);

    if (response.head.status != .ok) return error.HttpDownloadFailed;

    const file = try std.fs.cwd().createFile(dest_path, .{});
    defer file.close();

    var transfer_buffer: [4096]u8 = undefined;
    var reader = response.reader(&transfer_buffer);

    var buf: [4096]u8 = undefined;
    while (true) {
        const bytes_read = try reader.readSliceShort(&buf);
        if (bytes_read == 0) break;
        try file.writeAll(buf[0..bytes_read]);
    }
}

fn configureWslConfigs(allocator: Allocator) !void {
    const user_profile = std.process.getEnvVarOwned(allocator, "USERPROFILE") catch return;
    defer allocator.free(user_profile);

    const wslconfig_path = try std.fs.path.join(allocator, &[_][]const u8{ user_profile, ".wslconfig" });
    defer allocator.free(wslconfig_path);

    var content: []u8 = &.{};
    defer allocator.free(content);

    const file_exists = blk: {
        const f = std.fs.openFileAbsolute(wslconfig_path, .{ .mode = .read_only }) catch {
            break :blk false;
        };
        defer f.close();
        content = f.readToEndAlloc(allocator, 10 * 1024 * 1024) catch {
            break :blk false;
        };
        break :blk true;
    };

    var modified = false;
    var wslconfig_out = std.ArrayList(u8).empty;
    defer wslconfig_out.deinit(allocator);

    if (!file_exists) {
        try wslconfig_out.appendSlice(allocator,
            \\[experimental]
            \\virtFSMounting=true
            \\autoMemoryReclaim=gradual
            \\networkingMode=mirrored
            \\dnsTunneling=true
            \\
        );
        modified = true;
    } else {
        var line_it = std.mem.splitAny(u8, content, "\r\n");
        var has_experimental = false;
        
        var virt_fs = false;
        var memory_reclaim = false;
        var net_mode = false;
        var dns_tunnel = false;

        var in_experimental = false;
        while (line_it.next()) |line| {
            const trimmed = std.mem.trim(u8, line, " \t");
            if (std.mem.eql(u8, trimmed, "[experimental]")) {
                has_experimental = true;
                in_experimental = true;
            } else if (std.mem.startsWith(u8, trimmed, "[")) {
                in_experimental = false;
            }

            if (in_experimental) {
                if (std.mem.startsWith(u8, trimmed, "virtFSMounting=")) virt_fs = true;
                if (std.mem.startsWith(u8, trimmed, "autoMemoryReclaim=")) memory_reclaim = true;
                if (std.mem.startsWith(u8, trimmed, "networkingMode=")) net_mode = true;
                if (std.mem.startsWith(u8, trimmed, "dnsTunneling=")) dns_tunnel = true;
            }
            try wslconfig_out.appendSlice(allocator, line);
            try wslconfig_out.appendSlice(allocator, "\n");
        }

        if (!has_experimental) {
            try wslconfig_out.appendSlice(allocator, "\n[experimental]\n");
            try wslconfig_out.appendSlice(allocator, "virtFSMounting=true\n");
            try wslconfig_out.appendSlice(allocator, "autoMemoryReclaim=gradual\n");
            try wslconfig_out.appendSlice(allocator, "networkingMode=mirrored\n");
            try wslconfig_out.appendSlice(allocator, "dnsTunneling=true\n");
            modified = true;
        } else {
            var modified_content = std.ArrayList(u8).empty;
            defer modified_content.deinit(allocator);

            var line_it2 = std.mem.splitScalar(u8, wslconfig_out.items, '\n');
            while (line_it2.next()) |line| {
                try modified_content.appendSlice(allocator, line);
                try modified_content.appendSlice(allocator, "\n");
                if (std.mem.eql(u8, std.mem.trim(u8, line, " \t\r"), "[experimental]")) {
                    if (!virt_fs) {
                        try modified_content.appendSlice(allocator, "virtFSMounting=true\n");
                        modified = true;
                    }
                    if (!memory_reclaim) {
                        try modified_content.appendSlice(allocator, "autoMemoryReclaim=gradual\n");
                        modified = true;
                    }
                    if (!net_mode) {
                        try modified_content.appendSlice(allocator, "networkingMode=mirrored\n");
                        modified = true;
                    }
                    if (!dns_tunnel) {
                        try modified_content.appendSlice(allocator, "dnsTunneling=true\n");
                        modified = true;
                    }
                }
            }
            try wslconfig_out.resize(allocator, 0);
            try wslconfig_out.appendSlice(allocator, modified_content.items);
        }
    }

    if (modified) {
        const out_file = try std.fs.createFileAbsolute(wslconfig_path, .{});
        defer out_file.close();
        try out_file.writeAll(wslconfig_out.items);
    }
}

fn runNormal(allocator: Allocator, silent: bool) !void {
    try configureWslConfigs(allocator);

    if (isPortActive(PORT)) {
        if (!silent) {
            try openBrowser(allocator, PANEL_URL);
        }
        return;
    }

    var list_child = std.process.Child.init(&[_][]const u8{ "wsl.exe", "-l" }, allocator);
    list_child.stdout_behavior = .Pipe;
    list_child.stderr_behavior = .Ignore;

    try list_child.spawn();
    const stdout_list = try list_child.stdout.?.readToEndAlloc(allocator, 1024 * 1024);
    defer allocator.free(stdout_list);
    _ = try list_child.wait();

    var cleaned_list = std.ArrayList(u8).empty;
    defer cleaned_list.deinit(allocator);
    for (stdout_list) |c| {
        if (c != 0) {
            try cleaned_list.append(allocator, std.ascii.toLower(c));
        }
    }

    const is_installed = std.mem.indexOf(u8, cleaned_list.items, DISTRO_NAME) != null;

    if (!is_installed) {
        const confirm_text = try std.fmt.allocPrint(allocator,
            "ZenoPanel {s} belum terpasang di WSL 2.\nApakah Anda ingin mengunduh dan menginstalnya sekarang secara otomatis dari GitHub? (~18MB)",
            .{VERSION}
        );
        defer allocator.free(confirm_text);

        const ans = try showMessageBox(confirm_text, "ZenoPanel Installer", 0x00000001 | 0x00000020);
        if (ans != 1) return;

        const local_app_data = std.process.getEnvVarOwned(allocator, "LOCALAPPDATA") catch blk: {
            const user_prof = try std.process.getEnvVarOwned(allocator, "USERPROFILE");
            defer allocator.free(user_prof);
            break :blk try std.fs.path.join(allocator, &[_][]const u8{ user_prof, "AppData", "Local" });
        };
        defer allocator.free(local_app_data);

        const install_dir = try std.fs.path.join(allocator, &[_][]const u8{ local_app_data, "zenopanel-wsl" });
        defer allocator.free(install_dir);

        try std.fs.cwd().makePath(install_dir);

        const tar_file_name = try std.fmt.allocPrint(allocator, "zenoos-{s}.tar.gz", .{VERSION});
        defer allocator.free(tar_file_name);

        const temp_dir = std.process.getEnvVarOwned(allocator, "TEMP") catch "/tmp";
        defer if (!std.mem.eql(u8, temp_dir, "/tmp")) allocator.free(temp_dir);

        const temp_tar_path = try std.fs.path.join(allocator, &[_][]const u8{ temp_dir, tar_file_name });
        defer allocator.free(temp_tar_path);

        const download_url = try std.fmt.allocPrint(allocator, "https://github.com/nextcore/zenopanel/releases/download/{s}/{s}", .{ VERSION, tar_file_name });
        defer allocator.free(download_url);

        _ = try showMessageBox(
            "Proses pengunduhan distro ZenoPanel dimulai.\n\nKlik OK untuk memulai download di background. Kami akan memberi tahu Anda jika instalasi telah selesai.",
            "Mengunduh ZenoPanel...",
            0x00000000 | 0x00000040
        );

        downloadFile(allocator, download_url, temp_tar_path) catch |err| {
            const err_msg = try std.fmt.allocPrint(allocator,
                "Gagal mengunduh ZenoPanel dari GitHub.\nPastikan Anda terhubung ke Internet.\n\nDetail Error: {}",
                .{err}
            );
            defer allocator.free(err_msg);
            _ = try showMessageBox(err_msg, "Error Download", 0x00000000 | 0x00000010);
            return;
        };

        var import_child = std.process.Child.init(&[_][]const u8{
            "wsl.exe", "--import", DISTRO_NAME, install_dir, temp_tar_path, "--version", "2"
        }, allocator);
        import_child.stderr_behavior = .Pipe;

        try import_child.spawn();
        const stderr_buf = try import_child.stderr.?.readToEndAlloc(allocator, 1024 * 1024);
        defer allocator.free(stderr_buf);
        const import_exit = try import_child.wait();

        std.fs.deleteFileAbsolute(temp_tar_path) catch {};

        if (import_exit != .Exited or import_exit.Exited != 0) {
            const err_msg = try std.fmt.allocPrint(allocator,
                "Gagal mengimpor distro ke WSL 2.\nDetail Error:\n{s}",
                .{stderr_buf}
            );
            defer allocator.free(err_msg);
            _ = try showMessageBox(err_msg, "Error Import WSL", 0x00000000 | 0x00000010);
            return;
        }

        _ = try showMessageBox("Instalasi sukses! ZenoPanel berhasil terpasang di WSL 2.", "Sukses", 0x00000000 | 0x00000040);
    }

    if (std.fs.selfExePathAlloc(allocator)) |exe_path| {
        defer allocator.free(exe_path);
        const write_sh_cmd = try std.fmt.allocPrint(allocator, "echo '{s}' > /opt/zenopanel/.launcher_path", .{exe_path});
        defer allocator.free(write_sh_cmd);

        var write_child = std.process.Child.init(&[_][]const u8{
            "wsl.exe", "-d", DISTRO_NAME, "-u", "root", "--", "sh", "-c", write_sh_cmd
        }, allocator);
        _ = try write_child.spawn();
        _ = try write_child.wait();
    } else |_| {}

    var run_child = std.process.Child.init(&[_][]const u8{
        "wsl.exe", "-d", DISTRO_NAME, "-u", "root", "--cd", "/opt/zenopanel", "/usr/local/bin/zenopanel"
    }, allocator);
    
    _ = try run_child.spawn();

    std.Thread.sleep(1200 * std.time.ns_per_ms);

    if (!silent) {
        try openBrowser(allocator, PANEL_URL);
    }
}

fn runUninstall(allocator: Allocator) !void {
    const ans = try showMessageBox(
        "Apakah Anda yakin ingin menghapus ZenoPanel secara total dari sistem Anda?\nSemua kontainer, data, dan konfigurasi di dalam ZenoOS akan dihapus.",
        "Hapus ZenoPanel",
        0x00000001 | 0x00000030
    );
    if (ans != 1) return;

    var unreg_child = std.process.Child.init(&[_][]const u8{ "wsl.exe", "--unregister", DISTRO_NAME }, allocator);
    _ = try unreg_child.spawn();
    _ = try unreg_child.wait();

    const local_app_data = std.process.getEnvVarOwned(allocator, "LOCALAPPDATA") catch blk: {
        const user_prof = try std.process.getEnvVarOwned(allocator, "USERPROFILE");
        defer allocator.free(user_prof);
        break :blk try std.fs.path.join(allocator, &[_][]const u8{ user_prof, "AppData", "Local" });
    };
    defer allocator.free(local_app_data);

    const install_dir = try std.fs.path.join(allocator, &[_][]const u8{ local_app_data, "zenopanel-wsl" });
    defer allocator.free(install_dir);

    std.fs.deleteTreeAbsolute(install_dir) catch {};

    const user_profile = try std.process.getEnvVarOwned(allocator, "USERPROFILE");
    defer allocator.free(user_profile);
    const desktop_lnk = try std.fs.path.join(allocator, &[_][]const u8{ user_profile, "Desktop", "ZenoPanel.lnk" });
    defer allocator.free(desktop_lnk);
    std.fs.deleteFileAbsolute(desktop_lnk) catch {};

    const app_data = std.process.getEnvVarOwned(allocator, "APPDATA") catch "";
    defer if (app_data.len > 0) allocator.free(app_data);

    if (app_data.len > 0) {
        const startup_lnk = try std.fs.path.join(allocator, &[_][]const u8{ app_data, "Microsoft", "Windows", "Start Menu", "Programs", "Startup", "ZenoPanel.lnk" });
        defer allocator.free(startup_lnk);
        std.fs.deleteFileAbsolute(startup_lnk) catch {};
    }

    _ = try showMessageBox("ZenoPanel telah berhasil dihapus secara total dari sistem Anda.", "Copot Pemasangan Sukses", 0x00000000 | 0x00000040);
}

fn runAutostart(allocator: Allocator, enable: bool) !void {
    const app_data = std.process.getEnvVarOwned(allocator, "APPDATA") catch {
        _ = try showMessageBox("Gagal melacak direktori APPDATA Windows.", "Error", 0x00000000 | 0x00000010);
        return;
    };
    defer allocator.free(app_data);

    const startup_lnk = try std.fs.path.join(allocator, &[_][]const u8{ app_data, "Microsoft", "Windows", "Start Menu", "Programs", "Startup", "ZenoPanel.lnk" });
    defer allocator.free(startup_lnk);

    if (!enable) {
        std.fs.deleteFileAbsolute(startup_lnk) catch {};
        _ = try showMessageBox("ZenoPanel dinonaktifkan dari startup Windows.", "Sukses", 0x00000000 | 0x00000040);
        return;
    }

    const exe_path = std.fs.selfExePathAlloc(allocator) catch {
        _ = try showMessageBox("Gagal mendapatkan lokasi executable.", "Error", 0x00000000 | 0x00000010);
        return;
    };
    defer allocator.free(exe_path);

    const powershell_cmd = try std.fmt.allocPrint(allocator,
        \\$WshShell = New-Object -ComObject WScript.Shell; $Shortcut = $WshShell.CreateShortcut('{s}'); $Shortcut.TargetPath = '{s}'; $Shortcut.Arguments = '--silent'; $Shortcut.IconLocation = 'imageres.dll,-1005'; $Shortcut.Description = 'ZenoPanel Background Service'; $Shortcut.Save()
    , .{ startup_lnk, exe_path });
    defer allocator.free(powershell_cmd);

    var shortcut_child = std.process.Child.init(&[_][]const u8{ "powershell.exe", "-Command", powershell_cmd }, allocator);
    _ = try shortcut_child.spawn();
    _ = try shortcut_child.wait();

    _ = try showMessageBox("ZenoPanel berhasil dikonfigurasi untuk berjalan otomatis saat Windows menyala (secara background).", "Sukses", 0x00000000 | 0x00000040);
}

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    const args = try std.process.argsAlloc(allocator);
    defer std.process.argsFree(allocator, args);

    if (args.len > 1) {
        const arg = args[1];
        if (std.mem.eql(u8, arg, "--uninstall")) {
            try runUninstall(allocator);
            return;
        } else if (std.mem.eql(u8, arg, "--autostart-enable")) {
            try runAutostart(allocator, true);
            return;
        } else if (std.mem.eql(u8, arg, "--autostart-disable")) {
            try runAutostart(allocator, false);
            return;
        } else if (std.mem.eql(u8, arg, "--silent")) {
            try runNormal(allocator, true);
            return;
        }
    }

    try runNormal(allocator, false);
}
