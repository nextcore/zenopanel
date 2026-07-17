# ==============================================================================
# 🚀 ZenoPanel Control Center - PowerShell GUI Edition (v1.5.22)
# ==============================================================================
# Skrip ini menduplikasi fungsionalitas launcher native Zig, menyediakan GUI 
# interaktif yang ringan menggunakan Windows Forms untuk menghindari pemblokiran
# reputasi biner oleh antivirus seperti Symantec.
# ==============================================================================

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$DISTRO_NAME = "zenopanel"
$PORT = 3001
$PANEL_URL = "http://localhost:3001/login"
$VERSION = "v1.5.22"

# Ambil path skrip saat ini
$scriptPath = $MyInvocation.MyCommand.Path
if ([string]::IsNullOrEmpty($scriptPath)) {
    # Fallback jika dijalankan secara langsung lewat pipeline/terpotong
    $scriptPath = Join-Path $PSScriptRoot "zenopanel.ps1"
}

# Global states
$global:isZenoActive = $false
$global:isProcessing = $false

# ------------------------------------------------------------------------------
# Helper: Deteksi Status Port 3001 (Non-blocking)
# ------------------------------------------------------------------------------
function Test-PortActive {
    $tcp = New-Object System.Net.Sockets.TcpClient
    try {
        $connect = $tcp.BeginConnect("127.0.0.1", $PORT, $null, $null)
        $wait = $connect.AsyncWaitHandle.WaitOne(100, $false)
        if ($wait -and $tcp.Connected) {
            return $true
        }
    } catch {
        # Abaikan error
    } finally {
        $tcp.Close()
    }
    return $false
}

# ------------------------------------------------------------------------------
# Helper: Cek & Konfigurasi WSL Eksperimental di Win 11 (22H2+)
# ------------------------------------------------------------------------------
function Get-IsWindows11Experimental {
    try {
        $os = Get-CimInstance Win32_OperatingSystem -ErrorAction SilentlyContinue
        if ($os.Version -match '^10\.0\.(\d+)') {
            $build = [int]$Matches[1]
            return $build -ge 22621
        }
    } catch {}
    return $false
}

function Configure-WslConfig {
    if (-not (Get-IsWindows11Experimental)) { return }
    
    $wslconfig = Join-Path $env:USERPROFILE ".wslconfig"
    $experimentalBlock = @"
[experimental]
virtFSMounting=true
autoMemoryReclaim=gradual
networkingMode=mirrored
dnsTunneling=true
"@

    if (-not (Test-Path $wslconfig)) {
        Set-Content -Path $wslconfig -Value $experimentalBlock -Encoding UTF8
        return
    }

    $content = Get-Content -Path $wslconfig -Raw
    if ($content -notmatch '\[experimental\]') {
        $content = $content.Trim() + "`r`n`r`n" + $experimentalBlock
        Set-Content -Path $wslconfig -Value $content -Encoding UTF8
    } else {
        $lines = Get-Content -Path $wslconfig
        $newLines = @()
        $inExperimental = $false
        $virt = $false; $memory = $false; $net = $false; $dns = $false

        foreach ($line in $lines) {
            $trimmed = $line.Trim()
            if ($trimmed -eq "[experimental]") { $inExperimental = $true }
            elseif ($trimmed -like "[*]") { $inExperimental = $false }
            
            if ($inExperimental) {
                if ($trimmed -like "virtFSMounting=*") { $virt = $true }
                if ($trimmed -like "autoMemoryReclaim=*") { $memory = $true }
                if ($trimmed -like "networkingMode=*") { $net = $true }
                if ($trimmed -like "dnsTunneling=*") { $dns = $true }
            }
            $newLines += $line
        }

        if (-not $virt) { $newLines += "virtFSMounting=true" }
        if (-not $memory) { $newLines += "autoMemoryReclaim=gradual" }
        if (-not $net) { $newLines += "networkingMode=mirrored" }
        if (-not $dns) { $newLines += "dnsTunneling=true" }

        Set-Content -Path $wslconfig -Value ($newLines -join "`r`n") -Encoding UTF8
    }
}

# ------------------------------------------------------------------------------
# Logika Bisnis: Start, Stop, Uninstall, Autostart
# ------------------------------------------------------------------------------
function Run-Normal {
    param([bool]$silent)

    Configure-WslConfig

    # Cek Port Aktif
    if (Test-PortActive) {
        if (-not $silent) {
            $ans = [System.Windows.Forms.MessageBox]::Show(
                "Layanan ZenoPanel (distro ZenoOS) sudah berjalan.`n`nApakah Anda ingin membuka ZenoPanel di browser?`n`nPilih 'Yes' untuk membuka browser,`nPilih 'No' untuk menghentikan layanan (Stop WSL),`nPilih 'Cancel' untuk batal.",
                "ZenoPanel",
                [System.Windows.Forms.MessageBoxButtons]::YesNoCancel,
                [System.Windows.Forms.MessageBoxIcon]::Question
            )
            if ($ans -eq [System.Windows.Forms.DialogResult]::Yes) {
                Start-Process $PANEL_URL
            } elseif ($ans -eq [System.Windows.Forms.DialogResult]::No) {
                Run-Stop
            }
        }
        return
    }

    # Cek instalasi WSL
    $wslCheck = wsl.exe -l 2>&1
    if ($LASTEXITCODE -ne 0) {
        [System.Windows.Forms.MessageBox]::Show(
            "WSL 2 tidak ditemukan atau gagal dijalankan.`nPastikan fitur WSL 2 dan Virtual Machine Platform telah diaktifkan di komputer Anda.",
            "WSL 2 Tidak Ditemukan",
            [System.Windows.Forms.MessageBoxButtons]::OK,
            [System.Windows.Forms.MessageBoxIcon]::Error
        )
        return
    }

    # Cek apakah distro zenopanel sudah terpasang
    $installed = $false
    foreach ($line in $wslCheck) {
        if ($line.ToLower() -match $DISTRO_NAME) {
            $installed = $true
            break
        }
    }

    if (-not $installed) {
        $confirm = [System.Windows.Forms.MessageBox]::Show(
            "ZenoPanel $VERSION belum terpasang di WSL 2.`nApakah Anda ingin mengunduh dan menginstalnya sekarang secara otomatis dari GitHub? (~18MB)",
            "ZenoPanel Installer",
            [System.Windows.Forms.MessageBoxButtons]::YesNo,
            [System.Windows.Forms.MessageBoxIcon]::Question
        )
        if ($confirm -ne [System.Windows.Forms.DialogResult]::Yes) { return }

        $localAppData = $env:LOCALAPPDATA
        if ([string]::IsNullOrEmpty($localAppData)) {
            $localAppData = Join-Path $env:USERPROFILE "AppData\Local"
        }
        $installDir = Join-Path $localAppData "zenopanel-wsl"
        $null = New-Item -ItemType Directory -Force -Path $installDir

        $tarFileName = "zenoos-$VERSION.tar.gz"
        $tempDir = [System.IO.Path]::GetTempPath()
        $tempTarPath = Join-Path $tempDir $tarFileName
        $downloadUrl = "https://github.com/nextcore/zenopanel/releases/download/$VERSION/$tarFileName"

        [System.Windows.Forms.MessageBox]::Show(
            "Proses pengunduhan distro ZenoPanel dimulai.`n`nKlik OK untuk memulai download di latar belakang. Kami akan memberi tahu Anda jika instalasi telah selesai.",
            "Mengunduh ZenoPanel...",
            [System.Windows.Forms.MessageBoxButtons]::OK,
            [System.Windows.Forms.MessageBoxIcon]::Information
        )

        try {
            $webClient = New-Object System.Net.WebClient
            $webClient.DownloadFile($downloadUrl, $tempTarPath)
        } catch {
            [System.Windows.Forms.MessageBox]::Show(
                "Gagal mengunduh ZenoPanel dari GitHub.`nPastikan Anda terhubung ke Internet.`n`nDetail Error: $_",
                "Error Download",
                [System.Windows.Forms.MessageBoxButtons]::OK,
                [System.Windows.Forms.MessageBoxIcon]::Error
            )
            return
        }

        # Import Distro WSL
        Write-Host "Mengimpor distro ZenoPanel ke WSL..."
        $importProcess = Start-Process wsl.exe -ArgumentList "--import", $DISTRO_NAME, "`"$installDir`"", "`"$tempTarPath`"", "--version", "2" -NoNewWindow -PassThru -Wait
        
        # Bersihkan berkas unduhan
        Remove-Item -Path $tempTarPath -ErrorAction SilentlyContinue

        if ($importProcess.ExitCode -ne 0) {
            [System.Windows.Forms.MessageBox]::Show(
                "Gagal mengimpor distro ZenoPanel ke WSL 2.",
                "Error Import WSL",
                [System.Windows.Forms.MessageBoxButtons]::OK,
                [System.Windows.Forms.MessageBoxIcon]::Error
            )
            return
        }

        [System.Windows.Forms.MessageBox]::Show(
            "Instalasi sukses! ZenoPanel berhasil terpasang di WSL 2.",
            "Sukses",
            [System.Windows.Forms.MessageBoxButtons]::OK,
            [System.Windows.Forms.MessageBoxIcon]::Information
        )
    }

    # Daftarkan path launcher saat ini di dalam WSL
    $writeCmd = "echo '$scriptPath' > /opt/zenopanel/.launcher_path"
    $null = Start-Process wsl.exe -ArgumentList "-d", $DISTRO_NAME, "-u", "root", "--", "sh", "-c", "`"$writeCmd`"" -NoNewWindow -Wait

    # Jalankan layanan ZenoPanel
    $null = Start-Process wsl.exe -ArgumentList "-d", $DISTRO_NAME, "-u", "root", "--cd", "/opt/zenopanel", "/usr/local/bin/zenopanel" -NoNewWindow

    # Tunggu port aktif
    $success = $false
    for ($i = 0; $i -lt 50; $i++) {
        if (Test-PortActive) {
            $success = $true
            break
        }
        Start-Sleep -Milliseconds 200
    }

    if ($success) {
        if ($silent) {
            # Jalankan Windows Toast Notification jika mode silent
            [void] [System.Reflection.Assembly]::LoadWithPartialName('System.Windows.Forms')
            $objNotifyIcon = New-Object System.Windows.Forms.NotifyIcon
            $objNotifyIcon.Icon = [System.Drawing.SystemIcons]::Information
            $objNotifyIcon.BalloonTipText = 'Layanan ZenoPanel background service telah berhasil dijalankan.'
            $objNotifyIcon.BalloonTipTitle = 'ZenoPanel'
            $objNotifyIcon.Visible = $true
            $objNotifyIcon.ShowBalloonTip(5000)
        } else {
            Start-Process $PANEL_URL
        }
    } else {
        [System.Windows.Forms.MessageBox]::Show(
            "ZenoPanel telah dijalankan tetapi port 3001 tidak merespons dalam 10 detik.`nDistro WSL mungkin butuh waktu booting lebih lama.",
            "Peringatan ZenoPanel",
            [System.Windows.Forms.MessageBoxButtons]::OK,
            [System.Windows.Forms.MessageBoxIcon]::Warning
        )
    }
}

function Run-Stop {
    $null = Start-Process wsl.exe -ArgumentList "--terminate", $DISTRO_NAME -NoNewWindow -Wait
    [System.Windows.Forms.MessageBox]::Show(
        "Layanan ZenoPanel (distro ZenoOS) telah dimatikan.",
        "ZenoPanel Dihentikan",
        [System.Windows.Forms.MessageBoxButtons]::OK,
        [System.Windows.Forms.MessageBoxIcon]::Information
    )
}

function Run-Uninstall {
    $ans = [System.Windows.Forms.MessageBox]::Show(
        "Apakah Anda yakin ingin menghapus ZenoPanel secara total dari sistem Anda?`nSemua kontainer, data, dan konfigurasi di dalam ZenoOS akan dihapus.",
        "Hapus ZenoPanel",
        [System.Windows.Forms.MessageBoxButtons]::YesNo,
        [System.Windows.Forms.MessageBoxIcon]::Warning
    )
    if ($ans -ne [System.Windows.Forms.DialogResult]::Yes) { return }

    # Unregister WSL
    $null = Start-Process wsl.exe -ArgumentList "--unregister", $DISTRO_NAME -NoNewWindow -Wait

    # Hapus folder
    $localAppData = $env:LOCALAPPDATA
    if ([string]::IsNullOrEmpty($localAppData)) {
        $localAppData = Join-Path $env:USERPROFILE "AppData\Local"
    }
    $installDir = Join-Path $localAppData "zenopanel-wsl"
    Remove-Item -Recurse -Force -Path $installDir -ErrorAction SilentlyContinue

    # Hapus Shortcuts
    $desktopLnk = Join-Path $env:USERPROFILE "Desktop\ZenoPanel.lnk"
    Remove-Item -Path $desktopLnk -ErrorAction SilentlyContinue

    $startupLnk = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\Startup\ZenoPanel.lnk"
    Remove-Item -Path $startupLnk -ErrorAction SilentlyContinue

    [System.Windows.Forms.MessageBox]::Show(
        "ZenoPanel telah berhasil dihapus secara total dari sistem Anda.",
        "Copot Pemasangan Sukses",
        [System.Windows.Forms.MessageBoxButtons]::OK,
        [System.Windows.Forms.MessageBoxIcon]::Information
    )
}

function Test-AutostartEnabled {
    $startupLnk = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\Startup\ZenoPanel.lnk"
    return Test-Path $startupLnk
}

function Run-Autostart {
    param([bool]$enable)

    $startupLnk = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\Startup\ZenoPanel.lnk"

    if (-not $enable) {
        Remove-Item -Path $startupLnk -ErrorAction SilentlyContinue
        [System.Windows.Forms.MessageBox]::Show(
            "ZenoPanel dinonaktifkan dari startup Windows.",
            "Sukses",
            [System.Windows.Forms.MessageBoxButtons]::OK,
            [System.Windows.Forms.MessageBoxIcon]::Information
        )
        return
    }

    try {
        $WshShell = New-Object -ComObject WScript.Shell
        $Shortcut = $WshShell.CreateShortcut($startupLnk)
        $Shortcut.TargetPath = "powershell.exe"
        $Shortcut.Arguments = "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$scriptPath`" --silent"
        $Shortcut.IconLocation = "imageres.dll,-1005"
        $Shortcut.Description = "ZenoPanel Background Service"
        $Shortcut.Save()

        [System.Windows.Forms.MessageBox]::Show(
            "ZenoPanel berhasil dikonfigurasi untuk berjalan otomatis saat Windows menyala (secara background).",
            "Sukses",
            [System.Windows.Forms.MessageBoxButtons]::OK,
            [System.Windows.Forms.MessageBoxIcon]::Information
        )
    } catch {
        [System.Windows.Forms.MessageBox]::Show(
            "Gagal mengonfigurasi autostart.",
            "Error",
            [System.Windows.Forms.MessageBoxButtons]::OK,
            [System.Windows.Forms.MessageBoxIcon]::Error
        )
    }
}

# ------------------------------------------------------------------------------
# Antarmuka Windows Forms GUI (Replikasi Desain Slate Dark Mode)
# ------------------------------------------------------------------------------
function Show-Gui {
    $form = New-Object Windows.Forms.Form
    $form.Text = "ZenoPanel Control Center (Script Edition)"
    $form.Size = New-Object Drawing.Size(376, 365)
    $form.StartPosition = "CenterScreen"
    $form.FormBorderStyle = "FixedSingle"
    $form.MaximizeBox = $false
    $form.BackColor = [Drawing.Color]::FromArgb(15, 23, 42) # Slate Dark Mode Background

    # Title Label
    $lblTitle = New-Object Windows.Forms.Label
    $lblTitle.Text = "ZenoPanel Control Center"
    $lblTitle.Location = New-Object Drawing.Point(10, 15)
    $lblTitle.Size = New-Object Drawing.Size(340, 25)
    $lblTitle.TextAlign = [Drawing.ContentAlignment]::MiddleCenter
    $lblTitle.ForeColor = [Drawing.Color]::White
    $lblTitle.Font = New-Object Drawing.Font("Segoe UI", 13, [Drawing.FontStyle]::Bold)

    # Status Label
    $lblStatus = New-Object Windows.Forms.Label
    $lblStatus.Text = "Status:"
    $lblStatus.Location = New-Object Drawing.Point(20, 55)
    $lblStatus.Size = New-Object Drawing.Size(60, 20)
    $lblStatus.ForeColor = [Drawing.Color]::White
    $lblStatus.Font = New-Object Drawing.Font("Segoe UI", 10)

    # Dynamic Status Value
    $statusVal = New-Object Windows.Forms.Label
    $statusVal.Text = "Mencari..."
    $statusVal.Location = New-Object Drawing.Point(80, 55)
    $statusVal.Size = New-Object Drawing.Size(200, 20)
    $statusVal.ForeColor = [Drawing.Color]::FromArgb(156, 163, 175) # Gray
    $statusVal.Font = New-Object Drawing.Font("Segoe UI", 10)

    # Common Button Styles
    $btnFont = New-Object Drawing.Font("Segoe UI", 10)
    $btnBackColor = [Drawing.Color]::FromArgb(30, 41, 59) # Slate Dark Button
    $btnForeColor = [Drawing.Color]::White

    # Button 1: Buka Dashboard
    $btnStart = New-Object Windows.Forms.Button
    $btnStart.Text = "Buka Dashboard"
    $btnStart.Location = New-Object Drawing.Point(20, 90)
    $btnStart.Size = New-Object Drawing.Size(320, 35)
    $btnStart.FlatStyle = "Flat"
    $btnStart.Font = $btnFont
    $btnStart.BackColor = $btnBackColor
    $btnStart.ForeColor = $btnForeColor
    $btnStart.Add_Click({
        if ($global:isProcessing) { return }
        $global:isProcessing = $true
        $statusVal.Text = "Memulai..."
        $statusVal.ForeColor = [Drawing.Color]::FromArgb(56, 189, 248) # Sky Blue
        $form.Update()

        # Jalankan di Thread lain agar UI tidak freeze
        $job = Start-Job -ScriptBlock {
            param($path)
            & $path --silent
        } -ArgumentList $scriptPath
        
        while (-not $job.State -eq "Completed" -and -not (Test-PortActive)) {
            [System.Windows.Forms.Application]::DoEvents()
            Start-Sleep -Milliseconds 100
        }
        $null = Remove-Job -Job $job -Force

        $global:isProcessing = $false
        $form.Update()
    })

    # Button 2: Matikan Layanan (Stop WSL)
    $btnStop = New-Object Windows.Forms.Button
    $btnStop.Text = "Matikan Layanan (Stop WSL)"
    $btnStop.Location = New-Object Drawing.Point(20, 135)
    $btnStop.Size = New-Object Drawing.Size(320, 35)
    $btnStop.FlatStyle = "Flat"
    $btnStop.Font = $btnFont
    $btnStop.BackColor = $btnBackColor
    $btnStop.ForeColor = $btnForeColor
    $btnStop.Add_Click({
        if ($global:isProcessing) { return }
        $global:isProcessing = $true
        $statusVal.Text = "Menghentikan..."
        $statusVal.ForeColor = [Drawing.Color]::FromArgb(56, 189, 248) # Sky Blue
        $form.Update()

        Run-Stop

        $global:isProcessing = $false
        $form.Update()
    })

    # Helper: Update text tombol Autostart
    function Update-AutostartText {
        if (Test-AutostartEnabled) {
            $btnAutostart.Text = "Autostart: AKTIF (Klik untuk Nonaktif)"
        } else {
            $btnAutostart.Text = "Autostart: NONAKTIF (Klik untuk Aktif)"
        }
    }

    # Button 3: Autostart
    $btnAutostart = New-Object Windows.Forms.Button
    $btnAutostart.Location = New-Object Drawing.Point(20, 180)
    $btnAutostart.Size = New-Object Drawing.Size(320, 35)
    $btnAutostart.FlatStyle = "Flat"
    $btnAutostart.Font = $btnFont
    $btnAutostart.BackColor = $btnBackColor
    $btnAutostart.ForeColor = $btnForeColor
    $btnAutostart.Add_Click({
        $enabled = Test-AutostartEnabled
        Run-Autostart -enable (-not $enabled)
        Update-AutostartText
    })

    # Button 4: Copot ZenoPanel
    $btnUninstall = New-Object Windows.Forms.Button
    $btnUninstall.Text = "Copot ZenoPanel"
    $btnUninstall.Location = New-Object Drawing.Point(20, 225)
    $btnUninstall.Size = New-Object Drawing.Size(320, 35)
    $btnUninstall.FlatStyle = "Flat"
    $btnUninstall.Font = $btnFont
    $btnUninstall.BackColor = $btnBackColor
    $btnUninstall.ForeColor = $btnForeColor
    $btnUninstall.Add_Click({
        Run-Uninstall
        $form.Close()
    })

    # Button 5: Keluar
    $btnExit = New-Object Windows.Forms.Button
    $btnExit.Text = "Keluar"
    $btnExit.Location = New-Object Drawing.Point(20, 270)
    $btnExit.Size = New-Object Drawing.Size(320, 35)
    $btnExit.FlatStyle = "Flat"
    $btnExit.Font = $btnFont
    $btnExit.BackColor = $btnBackColor
    $btnExit.ForeColor = $btnForeColor
    $btnExit.Add_Click({
        $form.Close()
    })

    # Tambahkan elemen ke form
    $form.Controls.Add($lblTitle)
    $form.Controls.Add($lblStatus)
    $form.Controls.Add($statusVal)
    $form.Controls.Add($btnStart)
    $form.Controls.Add($btnStop)
    $form.Controls.Add($btnAutostart)
    $form.Controls.Add($btnUninstall)
    $form.Controls.Add($btnExit)

    # Inisialisasi text tombol autostart
    Update-AutostartText

    # Timer monitoring status port 3001
    $timer = New-Object Windows.Forms.Timer
    $timer.Interval = 1000
    $timer.Add_Tick({
        if ($global:isProcessing) { return }
        $active = Test-PortActive
        if ($active -ne $global:isZenoActive) {
            $global:isZenoActive = $active
            if ($active) {
                $statusVal.Text = "Aktif"
                $statusVal.ForeColor = [Drawing.Color]::FromArgb(74, 222, 128) # Green
            } else {
                $statusVal.Text = "Tidak Aktif"
                $statusVal.ForeColor = [Drawing.Color]::FromArgb(156, 163, 175) # Gray
            }
        }
    })
    $timer.Start()

    # Tampilkan Form secara modal
    [void]$form.ShowDialog()
    $timer.Stop()
}

# ------------------------------------------------------------------------------
# Entry Point Skrip (Parsing Argument)
# ------------------------------------------------------------------------------
if ($args.Count -gt 0) {
    $arg = $args[0]
    if ($arg -eq "--uninstall") {
        Run-Uninstall
    } elseif ($arg -eq "--autostart-enable") {
        Run-Autostart -enable $true
    } elseif ($arg -eq "--autostart-disable") {
        Run-Autostart -enable $false
    } elseif ($arg -eq "--silent") {
        Run-Normal -silent $true
    } elseif ($arg -eq "--stop") {
        Run-Stop
    } else {
        # default: jalankan GUI
        Show-Gui
    }
} else {
    Show-Gui
}
