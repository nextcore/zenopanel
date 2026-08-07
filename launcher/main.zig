const std = @import("std");
const builtin = @import("builtin");
const Allocator = std.mem.Allocator;

const DISTRO_NAME = "zenopanel";
const PORT = "3001";
const PANEL_URL = "http://localhost:3001/login";

// Versi default yang diselaraskan dengan tag rilis
const VERSION = "v1.9.1";

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

    pub const OSVERSIONINFOEXW = extern struct {
        dwOSVersionInfoSize: u32,
        dwMajorVersion: u32,
        dwMinorVersion: u32,
        dwBuildNumber: u32,
        dwPlatformId: u32,
        szCSDVersion: [128]u16,
        wServicePackMajor: u16,
        wServicePackMinor: u16,
        wSuiteMask: u16,
        wProductType: u8,
        wReserved: u8,
    };

    pub extern "ntdll" fn RtlGetVersion(
        lpVersionInformation: *OSVERSIONINFOEXW,
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


fn supportsWslExperimental() bool {
    if (builtin.os.tag != .windows) return false;
    var info: win.OSVERSIONINFOEXW = undefined;
    info.dwOSVersionInfoSize = @sizeOf(win.OSVERSIONINFOEXW);
    const status = win.RtlGetVersion(&info);
    if (status == 0) {
        // Build 22621 adalah Windows 11 22H2 yang mendukung fitur eksperimental WSL
        return info.dwBuildNumber >= 22621;
    }
    return false;
}

fn configureWslConfigs(allocator: Allocator) !void {
    const user_profile = std.process.getEnvVarOwned(allocator, "USERPROFILE") catch return;
    defer allocator.free(user_profile);

    const wslconfig_path = try std.fs.path.join(allocator, &[_][]const u8{ user_profile, ".wslconfig" });
    defer allocator.free(wslconfig_path);

    if (!supportsWslExperimental()) return;

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
            const ans = try showMessageBox(
                "Layanan ZenoPanel (distro ZenoOS) sudah berjalan.\n\n" ++
                "Apakah Anda ingin membuka ZenoPanel di browser?\n\n" ++
                "Pilih 'Yes' untuk membuka browser,\n" ++
                "Pilih 'No' untuk menghentikan layanan (Stop WSL),\n" ++
                "Pilih 'Cancel' untuk batal.",
                "ZenoPanel",
                0x00000003 | 0x00000020 // MB_YESNOCANCEL | MB_ICONQUESTION
            );
            if (ans == 6) { // IDYES
                try openBrowser(allocator, PANEL_URL);
            } else if (ans == 7) { // IDNO
                try runStop(allocator);
            }
        }
        return;
    }

    var list_child = std.process.Child.init(&[_][]const u8{ "wsl.exe", "-l" }, allocator);
    list_child.stdout_behavior = .Pipe;
    list_child.stderr_behavior = .Ignore;

    list_child.spawn() catch |err| {
        const err_msg = try std.fmt.allocPrint(allocator,
            "WSL 2 (Windows Subsystem for Linux) tidak ditemukan atau gagal dijalankan.\n" ++
            "Pastikan fitur WSL 2 dan Virtual Machine Platform telah diaktifkan.\n\nDetail Error: {}",
            .{err}
        );
        defer allocator.free(err_msg);
        _ = try showMessageBox(err_msg, "WSL 2 Tidak Ditemukan/Gagal", 0x00000000 | 0x00000010);
        return;
    };
    const stdout_list = try list_child.stdout.?.readToEndAlloc(allocator, 1024 * 1024);
    defer allocator.free(stdout_list);
    const list_exit = try list_child.wait();
    if (list_exit != .Exited or list_exit.Exited != 0) {
        _ = try showMessageBox(
            "WSL 2 gagal mengembalikan daftar distro. Silakan jalankan 'wsl --status' di PowerShell untuk memeriksa status WSL Anda.",
            "Error WSL 2",
            0x00000000 | 0x00000010
        );
        return;
    }

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
            "ZenoPanel {s} belum terpasang di WSL 2.\nApakah Anda ingin memasangnya sekarang?",
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

        var final_tar_path: ?[]const u8 = null;

        if (std.fs.selfExePathAlloc(allocator)) |exe_path| {
            defer allocator.free(exe_path);
            if (std.fs.path.dirname(exe_path)) |exe_dir| {
                const candidate1 = try std.fs.path.join(allocator, &[_][]const u8{ exe_dir, tar_file_name });
                if (std.fs.accessAbsolute(candidate1, .{})) |_| {
                    final_tar_path = candidate1;
                } else |_| {
                    allocator.free(candidate1);
                    const candidate2 = try std.fs.path.join(allocator, &[_][]const u8{ exe_dir, "zenoos.tar.gz" });
                    if (std.fs.accessAbsolute(candidate2, .{})) |_| {
                        final_tar_path = candidate2;
                    } else |_| {
                        allocator.free(candidate2);
                    }
                }
            }
        } else |_| {}

        const local_tar_path = final_tar_path orelse {
            const err_msg = try std.fmt.allocPrint(allocator,
                "Berkas distro ZenoOS ({s}) tidak ditemukan di direktori launcher.\n\n" ++
                "Pastikan Anda telah mengekstrak seluruh isi berkas ZIP ZenoPanel sebelum menjalankan launcher ini.",
                .{tar_file_name}
            );
            defer allocator.free(err_msg);
            _ = try showMessageBox(err_msg, "Berkas Distro Tidak Ditemukan", 0x00000000 | 0x00000010);
            return;
        };
        defer allocator.free(local_tar_path);

        var import_child = std.process.Child.init(&[_][]const u8{
            "wsl.exe", "--import", DISTRO_NAME, install_dir, local_tar_path, "--version", "2"
        }, allocator);
        import_child.stderr_behavior = .Pipe;

        try import_child.spawn();
        const stderr_buf = try import_child.stderr.?.readToEndAlloc(allocator, 1024 * 1024);
        defer allocator.free(stderr_buf);
        const import_exit = try import_child.wait();

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

    var success = false;
    var attempts: usize = 0;
    while (attempts < 50) : (attempts += 1) {
        if (isPortActive(PORT)) {
            success = true;
            break;
        }
        std.Thread.sleep(200 * std.time.ns_per_ms);
    }

    if (success) {
        if (silent) {
            const powershell_cmd =
                \\[void] [System.Reflection.Assembly]::LoadWithPartialName('System.Windows.Forms'); $objNotifyIcon = New-Object System.Windows.Forms.NotifyIcon; $objNotifyIcon.Icon = [System.Drawing.SystemIcons]::Information; $objNotifyIcon.BalloonTipText = 'Layanan ZenoPanel background service telah berhasil dijalankan.'; $objNotifyIcon.BalloonTipTitle = 'ZenoPanel'; $objNotifyIcon.Visible = $true; $objNotifyIcon.ShowBalloonTip(5000);
            ;
            var toast_child = std.process.Child.init(&[_][]const u8{ "powershell.exe", "-Command", powershell_cmd }, allocator);
            _ = toast_child.spawn() catch {};
        } else {
            try openBrowser(allocator, PANEL_URL);
        }
    } else {
        _ = try showMessageBox(
            "ZenoPanel telah dijalankan tetapi port 3001 tidak merespons dalam 10 detik.\nDistro WSL mungkin butuh waktu lebih lama untuk booting, atau ada kendala pada layanan.",
            "Peringatan ZenoPanel",
            0x00000000 | 0x00000030
        );
    }
}

fn runStop(allocator: Allocator) !void {
    var stop_child = std.process.Child.init(&[_][]const u8{ "wsl.exe", "--terminate", DISTRO_NAME }, allocator);
    _ = stop_child.spawn() catch |err| {
        const err_msg = try std.fmt.allocPrint(allocator, "Gagal menjalankan perintah WSL untuk menonaktifkan distro.\nError: {}", .{err});
        defer allocator.free(err_msg);
        _ = try showMessageBox(err_msg, "Error Stop ZenoOS", 0x00000000 | 0x00000010);
        return;
    };
    _ = try stop_child.wait();
    _ = try showMessageBox("Layanan ZenoPanel (distro ZenoOS) telah dimatikan.", "ZenoPanel Dihentikan", 0x00000000 | 0x00000040);
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

const win32 = if (builtin.os.tag == .windows) struct {
    pub const HWND = *anyopaque;
    pub const HINSTANCE = *anyopaque;
    pub const HMENU = *anyopaque;
    pub const HBRUSH = *anyopaque;
    pub const HFONT = *anyopaque;
    pub const HDC = *anyopaque;
    pub const HICON = *anyopaque;
    pub const HCURSOR = *anyopaque;
    pub const COLORREF = u32;

    pub const WNDPROC = *const fn (HWND, u32, usize, isize) callconv(.winapi) isize;

    pub const WNDCLASSEXW = extern struct {
        cbSize: u32 = @sizeOf(WNDCLASSEXW),
        style: u32,
        lpfnWndProc: WNDPROC,
        cbClsExtra: i32 = 0,
        cbWndExtra: i32 = 0,
        hInstance: HINSTANCE,
        hIcon: ?HICON = null,
        hCursor: ?HCURSOR = null,
        hbrBackground: ?HBRUSH = null,
        lpszMenuName: ?[*:0]const u16 = null,
        lpszClassName: [*:0]const u16,
        hIconSm: ?HICON = null,
    };

    pub const POINT = extern struct {
        x: i32,
        y: i32,
    };

    pub const MSG = extern struct {
        hwnd: ?HWND,
        message: u32,
        wParam: usize,
        lParam: isize,
        time: u32,
        pt: POINT,
        lPrivate: u32 = 0,
    };

    pub const RECT = extern struct {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    };

    pub const PAINTSTRUCT = extern struct {
        hdc: HDC,
        fErase: i32,
        rcPaint: RECT,
        fRestore: i32,
        fIncUpdate: i32,
        rgbReserved: [32]u8,
    };

    pub const CS_HREDRAW: u32 = 0x0002;
    pub const CS_VREDRAW: u32 = 0x0001;
    pub const WS_CHILD: u32 = 0x40000000;
    pub const WS_VISIBLE: u32 = 0x10000000;
    pub const BS_PUSHBUTTON: u32 = 0x00000000;
    pub const SS_CENTER: u32 = 0x00000001;
    pub const SS_LEFT: u32 = 0x00000000;
    pub const CW_USEDEFAULT: i32 = @as(i32, @bitCast(@as(u32, 0x80000000)));

    pub const WM_CREATE: u32 = 0x0001;
    pub const WM_DESTROY: u32 = 0x0002;
    pub const WM_COMMAND: u32 = 0x0111;
    pub const WM_CTLCOLORSTATIC: u32 = 0x0138;
    pub const WM_SETFONT: u32 = 0x0030;

    pub extern "user32" fn RegisterClassExW(lpwcx: *const WNDCLASSEXW) callconv(.winapi) u16;
    pub extern "user32" fn CreateWindowExW(
        dwExStyle: u32,
        lpClassName: [*:0]const u16,
        lpWindowName: ?[*:0]const u16,
        dwStyle: u32,
        X: i32,
        Y: i32,
        nWidth: i32,
        nHeight: i32,
        hWndParent: ?HWND,
        hMenu: ?HMENU,
        hInstance: HINSTANCE,
        lpParam: ?*anyopaque,
    ) callconv(.winapi) ?HWND;
    pub extern "user32" fn ShowWindow(hWnd: HWND, nCmdShow: i32) callconv(.winapi) i32;
    pub extern "user32" fn UpdateWindow(hWnd: HWND) callconv(.winapi) i32;
    pub extern "user32" fn GetMessageW(lpMsg: *MSG, hWnd: ?HWND, wMsgFilterMin: u32, wMsgFilterMax: u32) callconv(.winapi) i32;
    pub extern "user32" fn TranslateMessage(lpMsg: *const MSG) callconv(.winapi) i32;
    pub extern "user32" fn DispatchMessageW(lpMsg: *const MSG) callconv(.winapi) isize;
    pub extern "user32" fn PostQuitMessage(nExitCode: i32) callconv(.winapi) void;
    pub extern "user32" fn DefWindowProcW(hWnd: HWND, Msg: u32, wParam: usize, lParam: isize) callconv(.winapi) isize;
    pub extern "user32" fn SendMessageW(hWnd: HWND, Msg: u32, wParam: usize, lParam: isize) callconv(.winapi) isize;
    pub extern "user32" fn LoadCursorW(hInstance: ?HINSTANCE, lpCursorName: [*:0]const u16) callconv(.winapi) ?HCURSOR;
    pub extern "user32" fn SetWindowTextW(hWnd: HWND, lpString: [*:0]const u16) callconv(.winapi) i32;
    pub extern "user32" fn InvalidateRect(hWnd: HWND, lpRect: ?*const RECT, bErase: i32) callconv(.winapi) i32;
    pub extern "user32" fn GetModuleHandleW(lpModuleName: ?[*:0]const u16) callconv(.winapi) ?HINSTANCE;
    
    pub extern "gdi32" fn CreateSolidBrush(color: COLORREF) callconv(.winapi) ?HBRUSH;
    pub extern "gdi32" fn DeleteObject(ho: ?*anyopaque) callconv(.winapi) i32;
    pub extern "gdi32" fn SetTextColor(hdc: HDC, color: COLORREF) callconv(.winapi) COLORREF;
    pub extern "gdi32" fn SetBkColor(hdc: HDC, color: COLORREF) callconv(.winapi) COLORREF;
    pub extern "gdi32" fn SetBkMode(hdc: HDC, mode: i32) callconv(.winapi) i32;
    pub extern "gdi32" fn CreateFontW(
        cHeight: i32,
        cWidth: i32,
        cEscapement: i32,
        cOrientation: i32,
        cWeight: i32,
        bItalic: u32,
        bUnderline: u32,
        bStrikeOut: u32,
        iCharSet: u32,
        iOutPrecision: u32,
        iClipPrecision: u32,
        iQuality: u32,
        iPitchAndFamily: u32,
        pszFaceName: [*:0]const u16,
    ) callconv(.winapi) ?HFONT;
} else struct {};

const win32_gui = if (builtin.os.tag == .windows) struct {
    var hwnd_main: win32.HWND = undefined;
    var hwnd_status_val: win32.HWND = undefined;
    var hwnd_btn_start: win32.HWND = undefined;
    var hwnd_btn_stop: win32.HWND = undefined;
    var hwnd_btn_autostart: win32.HWND = undefined;
    var bg_brush: win32.HBRUSH = undefined;
    var global_allocator: Allocator = undefined;

    var is_zeno_active = false;
    var is_processing = false;

    fn toWString(comptime str: []const u8) *const [str.len:0]u16 {
        const S = struct {
            const out = blk: {
                var res: [str.len:0]u16 = undefined;
                for (str, 0..) |c, i| {
                    res[i] = c;
                }
                break :blk res;
            };
        };
        return &S.out;
    }

    fn statusMonitorLoop(hwnd: win32.HWND) void {
        _ = hwnd;
        while (true) {
            const active = isPortActive(PORT);
            if (active != is_zeno_active) {
                is_zeno_active = active;
                if (!is_processing) {
                    if (active) {
                        _ = win32.SetWindowTextW(hwnd_status_val, toWString("Aktif").ptr);
                    } else {
                        _ = win32.SetWindowTextW(hwnd_status_val, toWString("Tidak Aktif").ptr);
                    }
                }
                _ = win32.InvalidateRect(hwnd_status_val, null, 1);
            }
            std.Thread.sleep(1 * std.time.ns_per_s);
        }
    }

    fn startZenoTask(hwnd: win32.HWND) void {
        _ = hwnd;
        is_processing = true;
        _ = win32.SetWindowTextW(hwnd_status_val, toWString("Memulai...").ptr);
        _ = win32.InvalidateRect(hwnd_status_val, null, 1);
        
        runNormal(global_allocator, false) catch |err| {
            is_processing = false;
            const msg = std.fmt.allocPrint(global_allocator, "Gagal menjalankan ZenoPanel: {}", .{err}) catch return;
            defer global_allocator.free(msg);
            _ = showMessageBox(msg, "Error", 0x10) catch {};
            return;
        };
        
        is_processing = false;
        _ = win32.InvalidateRect(hwnd_status_val, null, 1);
    }

    fn stopZenoTask(hwnd: win32.HWND) void {
        _ = hwnd;
        is_processing = true;
        _ = win32.SetWindowTextW(hwnd_status_val, toWString("Menghentikan...").ptr);
        _ = win32.InvalidateRect(hwnd_status_val, null, 1);
        
        runStop(global_allocator) catch |err| {
            is_processing = false;
            const msg = std.fmt.allocPrint(global_allocator, "Gagal menghentikan ZenoPanel: {}", .{err}) catch return;
            defer global_allocator.free(msg);
            _ = showMessageBox(msg, "Error", 0x10) catch {};
            return;
        };
        
        is_processing = false;
        _ = win32.InvalidateRect(hwnd_status_val, null, 1);
    }

    fn isAutostartEnabled(allocator: Allocator) bool {
        const app_data = std.process.getEnvVarOwned(allocator, "APPDATA") catch return false;
        defer allocator.free(app_data);
        
        const startup_lnk = std.fs.path.join(allocator, &[_][]const u8{ app_data, "Microsoft", "Windows", "Start Menu", "Programs", "Startup", "ZenoPanel.lnk" }) catch return false;
        defer allocator.free(startup_lnk);
        
        const f = std.fs.openFileAbsolute(startup_lnk, .{}) catch return false;
        f.close();
        return true;
    }

    fn updateAutostartButtonText() void {
        const enabled = isAutostartEnabled(global_allocator);
        if (enabled) {
            _ = win32.SetWindowTextW(hwnd_btn_autostart, toWString("Autostart: AKTIF (Klik untuk Nonaktif)").ptr);
        } else {
            _ = win32.SetWindowTextW(hwnd_btn_autostart, toWString("Autostart: NONAKTIF (Klik untuk Aktif)").ptr);
        }
    }

    fn toggleAutostartTask(hwnd: win32.HWND) void {
        _ = hwnd;
        const enabled = isAutostartEnabled(global_allocator);
        runAutostart(global_allocator, !enabled) catch |err| {
            const msg = std.fmt.allocPrint(global_allocator, "Gagal mengubah autostart: {}", .{err}) catch return;
            defer global_allocator.free(msg);
            _ = showMessageBox(msg, "Error", 0x10) catch {};
            return;
        };
        updateAutostartButtonText();
    }

    fn uninstallTask(hwnd: win32.HWND) void {
        _ = hwnd;
        runUninstall(global_allocator) catch |err| {
            const msg = std.fmt.allocPrint(global_allocator, "Gagal mencopot ZenoPanel: {}", .{err}) catch return;
            defer global_allocator.free(msg);
            _ = showMessageBox(msg, "Error", 0x10) catch {};
            return;
        };
        win32.PostQuitMessage(0);
    }

    fn wndProc(hwnd: win32.HWND, message: u32, wParam: usize, lParam: isize) callconv(.winapi) isize {
        switch (message) {
            win32.WM_CREATE => {
                const hInstance = win32.GetModuleHandleW(null).?;
                
                const hTitleFont = win32.CreateFontW(
                    22, 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 2, 0,
                    toWString("Segoe UI")
                );
                const hFont = win32.CreateFontW(
                    16, 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 2, 0,
                    toWString("Segoe UI")
                );
                
                const hwnd_title = win32.CreateWindowExW(
                    0, toWString("STATIC"), toWString("ZenoPanel Control Center"),
                    win32.WS_CHILD | win32.WS_VISIBLE | win32.SS_CENTER,
                    10, 15, 340, 25, hwnd, null, hInstance, null
                ).?;
                _ = win32.SendMessageW(hwnd_title, win32.WM_SETFONT, @intFromPtr(hTitleFont), 1);
                
                const hwnd_status_lbl = win32.CreateWindowExW(
                    0, toWString("STATIC"), toWString("Status:"),
                    win32.WS_CHILD | win32.WS_VISIBLE | win32.SS_LEFT,
                    20, 55, 60, 20, hwnd, null, hInstance, null
                ).?;
                _ = win32.SendMessageW(hwnd_status_lbl, win32.WM_SETFONT, @intFromPtr(hFont), 1);
                
                hwnd_status_val = win32.CreateWindowExW(
                    0, toWString("STATIC"), toWString("Mencari..."),
                    win32.WS_CHILD | win32.WS_VISIBLE | win32.SS_LEFT,
                    80, 55, 200, 20, hwnd, null, hInstance, null
                ).?;
                _ = win32.SendMessageW(hwnd_status_val, win32.WM_SETFONT, @intFromPtr(hFont), 1);
                
                hwnd_btn_start = win32.CreateWindowExW(
                    0, toWString("BUTTON"), toWString("Buka Dashboard"),
                    win32.WS_CHILD | win32.WS_VISIBLE | win32.BS_PUSHBUTTON,
                    20, 90, 320, 35, hwnd, @ptrFromInt(1001), hInstance, null
                ).?;
                _ = win32.SendMessageW(hwnd_btn_start, win32.WM_SETFONT, @intFromPtr(hFont), 1);
                
                hwnd_btn_stop = win32.CreateWindowExW(
                    0, toWString("BUTTON"), toWString("Matikan Layanan (Stop WSL)"),
                    win32.WS_CHILD | win32.WS_VISIBLE | win32.BS_PUSHBUTTON,
                    20, 135, 320, 35, hwnd, @ptrFromInt(1002), hInstance, null
                ).?;
                _ = win32.SendMessageW(hwnd_btn_stop, win32.WM_SETFONT, @intFromPtr(hFont), 1);
                
                hwnd_btn_autostart = win32.CreateWindowExW(
                    0, toWString("BUTTON"), toWString("Autostart"),
                    win32.WS_CHILD | win32.WS_VISIBLE | win32.BS_PUSHBUTTON,
                    20, 180, 320, 35, hwnd, @ptrFromInt(1003), hInstance, null
                ).?;
                _ = win32.SendMessageW(hwnd_btn_autostart, win32.WM_SETFONT, @intFromPtr(hFont), 1);
                updateAutostartButtonText();
                
                const hwnd_btn_uninstall = win32.CreateWindowExW(
                    0, toWString("BUTTON"), toWString("Copot ZenoPanel"),
                    win32.WS_CHILD | win32.WS_VISIBLE | win32.BS_PUSHBUTTON,
                    20, 225, 320, 35, hwnd, @ptrFromInt(1004), hInstance, null
                ).?;
                _ = win32.SendMessageW(hwnd_btn_uninstall, win32.WM_SETFONT, @intFromPtr(hFont), 1);
                
                const hwnd_btn_exit = win32.CreateWindowExW(
                    0, toWString("BUTTON"), toWString("Keluar"),
                    win32.WS_CHILD | win32.WS_VISIBLE | win32.BS_PUSHBUTTON,
                    20, 270, 320, 35, hwnd, @ptrFromInt(1005), hInstance, null
                ).?;
                _ = win32.SendMessageW(hwnd_btn_exit, win32.WM_SETFONT, @intFromPtr(hFont), 1);
                
                _ = std.Thread.spawn(.{}, statusMonitorLoop, .{hwnd}) catch {};
            },
            win32.WM_CTLCOLORSTATIC => {
                const control_hwnd = @as(win32.HWND, @ptrFromInt(@as(usize, @bitCast(lParam))));
                const hdc = @as(win32.HDC, @ptrFromInt(wParam));
                _ = win32.SetBkColor(hdc, 0x002A170F);
                _ = win32.SetBkMode(hdc, 1);
                if (control_hwnd == hwnd_status_val) {
                    if (is_processing) {
                        _ = win32.SetTextColor(hdc, 0x0038bdf8);
                    } else if (is_zeno_active) {
                        _ = win32.SetTextColor(hdc, 0x004ade80);
                    } else {
                        _ = win32.SetTextColor(hdc, 0x009ca3af);
                    }
                } else {
                    _ = win32.SetTextColor(hdc, 0x00FFFFFF);
                }
                return @as(isize, @bitCast(@intFromPtr(bg_brush)));
            },
            win32.WM_COMMAND => {
                const control_id = wParam & 0xFFFF;
                switch (control_id) {
                    1001 => {
                        if (!is_processing) {
                            _ = std.Thread.spawn(.{}, startZenoTask, .{hwnd}) catch {};
                        }
                    },
                    1002 => {
                        if (!is_processing) {
                            _ = std.Thread.spawn(.{}, stopZenoTask, .{hwnd}) catch {};
                        }
                    },
                    1003 => {
                        _ = std.Thread.spawn(.{}, toggleAutostartTask, .{hwnd}) catch {};
                    },
                    1004 => {
                        _ = std.Thread.spawn(.{}, uninstallTask, .{hwnd}) catch {};
                    },
                    1005 => {
                        win32.PostQuitMessage(0);
                    },
                    else => {}
                }
            },
            win32.WM_DESTROY => {
                win32.PostQuitMessage(0);
            },
            else => {
                return win32.DefWindowProcW(hwnd, message, wParam, lParam);
            }
        }
        return 0;
    }

    pub fn runGui(allocator: Allocator) !void {
        global_allocator = allocator;
        bg_brush = win32.CreateSolidBrush(0x002A170F).?;
        defer _ = win32.DeleteObject(bg_brush);
        
        const hInstance = win32.GetModuleHandleW(null).?;
        const CLASS_NAME = toWString("ZenoPanelControlCenter");
        
        const wcex = win32.WNDCLASSEXW{
            .style = win32.CS_HREDRAW | win32.CS_VREDRAW,
            .lpfnWndProc = wndProc,
            .hInstance = hInstance,
            .hCursor = win32.LoadCursorW(null, @ptrFromInt(32512)),
            .hbrBackground = bg_brush,
            .lpszClassName = CLASS_NAME,
        };
        
        _ = win32.RegisterClassExW(&wcex);
        
        const hwnd = win32.CreateWindowExW(
            0, CLASS_NAME, toWString("ZenoPanel Control Center"),
            0x00CA0000 | win32.WS_VISIBLE,
            win32.CW_USEDEFAULT, win32.CW_USEDEFAULT, 376, 365,
            null, null, hInstance, null
        ) orelse return error.WindowCreationFailed;
        
        hwnd_main = hwnd;
        
        const dwmapi = std.os.windows.kernel32.GetModuleHandleW(
            &[_:0]u16{ 'd', 'w', 'm', 'a', 'p', 'i', '.', 'd', 'l', 'l' }
        ) orelse std.os.windows.kernel32.LoadLibraryW(
            &[_:0]u16{ 'd', 'w', 'm', 'a', 'p', 'i', '.', 'd', 'l', 'l' }
        );
        if (dwmapi) |dll| {
            const DwmSetWindowAttribute = @as(
                *const fn (win32.HWND, u32, *const anyopaque, u32) callconv(.winapi) i32,
                @ptrCast(std.os.windows.kernel32.GetProcAddress(dll, "DwmSetWindowAttribute") orelse return)
            );
            const use_dark: u32 = 1;
            _ = DwmSetWindowAttribute(hwnd, 20, &use_dark, @sizeOf(u32));
        }
        
        _ = win32.ShowWindow(hwnd, 5);
        _ = win32.UpdateWindow(hwnd);
        
        var msg: win32.MSG = undefined;
        while (win32.GetMessageW(&msg, null, 0, 0) > 0) {
            _ = win32.TranslateMessage(&msg);
            _ = win32.DispatchMessageW(&msg);
        }
    }
} else struct {};

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
        } else if (std.mem.eql(u8, arg, "--stop")) {
            try runStop(allocator);
            return;
        }
    }

    if (builtin.os.tag == .windows) {
        try win32_gui.runGui(allocator);
    } else {
        try runNormal(allocator, false);
    }
}
