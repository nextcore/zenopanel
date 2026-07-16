package main

import (
	"bytes"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"syscall"
	"time"
	"unsafe"
)

// Versi default (dapat di-override saat compile time menggunakan -ldflags "-X main.version=v1.5.19")
var version = "v1.5.19"

const (
	distroName = "zenopanel"
	port       = "3001"
	panelURL   = "http://localhost:3001/login"
)

// Windows API MessageBox Constants
const (
	MB_OK                = 0x00000000
	MB_OKCANCEL          = 0x00000001
	MB_ICONHAND          = 0x00000010
	MB_ICONQUESTION      = 0x00000020
	MB_ICONEXCLAMATION   = 0x00000030
	MB_ICONASTERISK      = 0x00000040
	IDOK                 = 1
)

func messageBox(hwnd uintptr, text, caption string, utype uint32) int {
	user32 := syscall.NewLazyDLL("user32.dll")
	messageBoxW := user32.NewProc("MessageBoxW")
	
	ret, _, _ := messageBoxW.Call(
		hwnd,
		uintptr(unsafe.Pointer(syscall.StringToUTF16Ptr(text))),
		uintptr(unsafe.Pointer(syscall.StringToUTF16Ptr(caption))),
		uintptr(utype),
	)
	return int(ret)
}

// Clean UTF-16 null bytes to convert to normal ASCII string
func cleanWSLOutput(b []byte) string {
	var buf bytes.Buffer
	for _, char := range b {
		if char != 0 {
			buf.WriteByte(char)
		}
	}
	return buf.String()
}

func downloadFile(filepath string, url string) error {
	out, err := os.Create(filepath)
	if err != nil {
		return err
	}
	defer out.Close()

	resp, err := http.Get(url)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("HTTP status %s", resp.Status)
	}

	_, err = io.Copy(out, resp.Body)
	if err != nil {
		return err
	}

	return nil
}

func runUninstall() {
	ans := messageBox(0, 
		"Apakah Anda yakin ingin menghapus ZenoPanel secara total dari sistem Anda?\nSemua kontainer, data, dan konfigurasi di dalam ZenoOS akan dihapus.", 
		"Hapus ZenoPanel", 
		MB_OKCANCEL|MB_ICONEXCLAMATION)
	if ans != IDOK {
		return
	}

	// 1. Unregister distro
	exec.Command("wsl", "--unregister", distroName).Run()

	// 2. Hapus folder LocalAppData
	localAppData := os.Getenv("LOCALAPPDATA")
	if localAppData != "" {
		installDir := filepath.Join(localAppData, "zenopanel-wsl")
		os.RemoveAll(installDir)
	}

	// 3. Hapus Desktop Shortcut jika ada
	desktopPath := filepath.Join(os.Getenv("USERPROFILE"), "Desktop", "ZenoPanel.lnk")
	os.Remove(desktopPath)

	// 4. Hapus Startup Shortcut jika ada
	appData := os.Getenv("APPDATA")
	if appData != "" {
		startupLnk := filepath.Join(appData, "Microsoft", "Windows", "Start Menu", "Programs", "Startup", "ZenoPanel.lnk")
		os.Remove(startupLnk)
	}

	messageBox(0, "ZenoPanel telah berhasil dihapus secara total dari sistem Anda.", "Copot Pemasangan Sukses", MB_OK|MB_ICONASTERISK)
}

func runAutostart(enable bool) {
	appData := os.Getenv("APPDATA")
	if appData == "" {
		messageBox(0, "Gagal melacak direktori APPDATA Windows.", "Error", MB_OK|MB_ICONHAND)
		return
	}
	startupLnk := filepath.Join(appData, "Microsoft", "Windows", "Start Menu", "Programs", "Startup", "ZenoPanel.lnk")

	if !enable {
		os.Remove(startupLnk)
		messageBox(0, "ZenoPanel dinonaktifkan dari startup Windows.", "Sukses", MB_OK|MB_ICONASTERISK)
		return
	}

	// Buat startup shortcut
	exePath, err := os.Executable()
	if err != nil {
		messageBox(0, "Gagal mendapatkan lokasi executable.", "Error", MB_OK|MB_ICONHAND)
		return
	}

	cmdStr := fmt.Sprintf(`$WshShell = New-Object -ComObject WScript.Shell; $Shortcut = $WshShell.CreateShortcut('%s'); $Shortcut.TargetPath = '%s'; $Shortcut.Arguments = '--silent'; $Shortcut.IconLocation = 'imageres.dll,-1005'; $Shortcut.Description = 'ZenoPanel Background Service'; $Shortcut.Save()`, startupLnk, exePath)
	exec.Command("powershell", "-Command", cmdStr).Run()

	messageBox(0, "ZenoPanel berhasil dikonfigurasi untuk berjalan otomatis saat Windows menyala (secara background).", "Sukses", MB_OK|MB_ICONASTERISK)
}

func main() {
	// Evaluasi argumen CLI rilis
	if len(os.Args) > 1 {
		arg := os.Args[1]
		switch arg {
		case "--uninstall":
			runUninstall()
			return
		case "--autostart-enable":
			runAutostart(true)
			return
		case "--autostart-disable":
			runAutostart(false)
			return
		case "--silent":
			runNormal(true)
			return
		}
	}

	runNormal(false)
}

func runNormal(silent bool) {
	// 1. Cek apakah port ZenoPanel (3001) sudah aktif
	conn, err := net.DialTimeout("tcp", "127.0.0.1:"+port, 300*time.Millisecond)
	if err == nil {
		conn.Close()
		// Server sudah jalan, langsung buka browser jika tidak silent
		if !silent {
			openBrowser(panelURL)
		}
		return
	}

	// 2. Cek apakah distro 'zenopanel' sudah terdaftar di WSL
	listCmd := exec.Command("wsl", "-l")
	listOutput, _ := listCmd.Output()
	listStr := strings.ToLower(cleanWSLOutput(listOutput))

	isInstalled := strings.Contains(listStr, distroName)

	if !isInstalled {
		// Tampilkan konfirmasi instalasi
		ans := messageBox(0, 
			fmt.Sprintf("ZenoPanel %s belum terpasang di WSL 2.\nApakah Anda ingin mengunduh dan menginstalnya sekarang secara otomatis dari GitHub? (~18MB)", version), 
			"ZenoPanel Installer", 
			MB_OKCANCEL|MB_ICONQUESTION)
		
		if ans != IDOK {
			return
		}

		// Siapkan direktori lokal AppData dan unduhan temporer
		localAppData := os.Getenv("LOCALAPPDATA")
		if localAppData == "" {
			localAppData = filepath.Join(os.Getenv("USERPROFILE"), "AppData", "Local")
		}
		installDir := filepath.Join(localAppData, "zenopanel-wsl")
		_ = os.MkdirAll(installDir, 0755)

		tarFileName := fmt.Sprintf("zenoos-%s.tar.gz", version)
		tempTarPath := filepath.Join(os.Getenv("TEMP"), tarFileName)
		downloadURL := fmt.Sprintf("https://github.com/nextcore/zenopanel/releases/download/%s/%s", version, tarFileName)

		// Tampilkan notifikasi proses unduhan dimulai
		messageBox(0, 
			"Proses pengunduhan distro ZenoPanel dimulai.\n\nKlik OK untuk memulai download di background. Kami akan memberi tahu Anda jika instalasi telah selesai.", 
			"Mengunduh ZenoPanel...", 
			MB_OK|MB_ICONASTERISK)

		// Jalankan proses download
		err = downloadFile(tempTarPath, downloadURL)
		if err != nil {
			messageBox(0, 
				fmt.Sprintf("Gagal mengunduh ZenoPanel dari GitHub.\nPastikan Anda terhubung ke Internet.\n\nDetail Error:\n%v", err), 
				"Error Download", 
				MB_OK|MB_ICONHAND)
			return
		}

		// Jalankan wsl --import menggunakan berkas yang berhasil diunduh
		importCmd := exec.Command("wsl", "--import", distroName, installDir, tempTarPath, "--version", "2")
		var stderr bytes.Buffer
		importCmd.Stderr = &stderr
		
		err = importCmd.Run()
		_ = os.Remove(tempTarPath)

		if err != nil {
			messageBox(0, 
				fmt.Sprintf("Gagal mengimpor distro ke WSL 2.\nDetail Error:\n%s", stderr.String()), 
				"Error Import WSL", 
				MB_OK|MB_ICONHAND)
			return
		}

		messageBox(0, "Instalasi sukses! ZenoPanel berhasil terpasang di WSL 2.", "Sukses", MB_OK|MB_ICONASTERISK)
	}

	// Tulis path launcher executable ke guest WSL agar Guest Web UI bisa memanggilnya
	exePath, err := os.Executable()
	if err == nil {
		writeCmd := exec.Command("wsl", "-d", distroName, "-u", "root", "--", "sh", "-c", fmt.Sprintf("echo '%s' > /opt/zenopanel/.launcher_path", exePath))
		writeCmd.SysProcAttr = &syscall.SysProcAttr{HideWindow: true}
		_ = writeCmd.Run()
	}

	// 3. Jalankan server ZenoPanel di background
	runCmd := exec.Command("wsl", "-d", distroName, "-u", "root", "--cd", "/opt/zenopanel", "/usr/local/bin/zenopanel")
	runCmd.SysProcAttr = &syscall.SysProcAttr{HideWindow: true}
	
	err = runCmd.Start()
	if err != nil {
		messageBox(0, fmt.Sprintf("Gagal menjalankan server ZenoPanel: %v", err), "Error Running Server", MB_OK|MB_ICONHAND)
		return
	}

	// Tunggu sebentar hingga server binding ke port 3001
	time.Sleep(1200 * time.Millisecond)

	// 4. Buka Browser (jika tidak silent)
	if !silent {
		openBrowser(panelURL)
	}
}

func openBrowser(url string) {
	cmd := exec.Command("rundll32", "url.dll,FileProtocolHandler", url)
	_ = cmd.Run()
}
