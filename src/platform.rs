use anyhow::{Context, Result, bail};
use std::{
    ffi::OsStr,
    fs,
    os::windows::{ffi::OsStrExt, fs::MetadataExt, process::CommandExt},
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};
use windows::Win32::Storage::Packaging::Appx::GetApplicationUserModelId;
use windows::{
    Win32::{
        Foundation::*,
        Security::{Authorization::*, *},
        Storage::FileSystem::*,
        System::{Diagnostics::ToolHelp::*, Registry::*, Threading::*},
        UI::{Shell::*, WindowsAndMessaging::*},
    },
    core::{PCWSTR, PWSTR, w},
};

pub fn wide(s: impl AsRef<OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(Some(0)).collect()
}
pub fn hidden_command(exe: impl AsRef<OsStr>) -> Command {
    let mut c = Command::new(exe);
    c.creation_flags(0x08000000);
    c
}
pub fn check_path(path: &Path) -> Result<()> {
    for p in path.ancestors() {
        match fs::symlink_metadata(p) {
            Ok(m) if m.file_attributes() & 0x400 != 0 => {
                bail!("Symbolic links and junctions are not allowed in credential paths")
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e).context("Cannot inspect credential path"),
        }
    }
    Ok(())
}
pub fn restrict(path: &Path) -> Result<()> {
    check_path(path)?;
    let sid = current_sid()?;
    let sddl = wide(format!("D:P(A;OICI;FA;;;{sid})"));
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(sddl.as_ptr()),
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )?;
        let result = SetFileSecurityW(
            PCWSTR(wide(path).as_ptr()),
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            descriptor,
        )
        .ok();
        LocalFree(HLOCAL(descriptor.0));
        result?;
    }
    Ok(())
}
pub fn current_sid() -> Result<String> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)?;
        let result = (|| -> Result<String> {
            let mut size = 0;
            let _ = GetTokenInformation(token, TokenUser, None, 0, &mut size);
            anyhow::ensure!(size > 0, "Cannot query current user SID");
            let mut buffer = vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
            GetTokenInformation(
                token,
                TokenUser,
                Some(buffer.as_mut_ptr().cast()),
                size,
                &mut size,
            )?;
            let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
            let mut text = PWSTR::null();
            ConvertSidToStringSidW(user.User.Sid, &mut text)?;
            let value = text.to_string();
            LocalFree(HLOCAL(text.0.cast()));
            Ok(value?)
        })();
        let _ = CloseHandle(token);
        result
    }
}
pub fn secure_dir(path: &Path) -> Result<()> {
    check_path(path)?;
    if !path.exists() {
        if let Some(parent) = path.parent()
            && !parent.exists()
        {
            secure_dir(parent)?;
        }
        // Restrict before storing any credentials; parent private directory protects the creation interval.
        fs::create_dir(path)?;
    }
    restrict(path)
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    check_path(path)?;
    let parent = path.parent().context("Missing parent directory")?;
    secure_dir(parent)?;
    replace_file(path, parent, bytes)
}
pub fn atomic_write_external(path: &Path, bytes: &[u8]) -> Result<()> {
    check_path(path)?;
    let parent = path.parent().context("Missing parent directory")?;
    if !parent.exists() {
        fs::create_dir_all(parent).context("Cannot create credential directory")?;
    }
    // The external directory (for example CODEX_HOME) keeps its existing ACL.
    replace_file(path, parent, bytes)
}
fn replace_file(path: &Path, parent: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let temp = parent.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut f = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)?;
        restrict(&temp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        check_path(path)?;
        unsafe {
            MoveFileExW(
                PCWSTR(wide(&temp).as_ptr()),
                PCWSTR(wide(path).as_ptr()),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
pub fn read(path: &Path) -> Result<Vec<u8>> {
    check_path(path)?;
    fs::read(path).context("Cannot read local account data")
}
pub fn remove_tree(path: &Path, root: &Path) -> Result<()> {
    check_path(path)?;
    check_path(root)?;
    let absolute = fs::canonicalize(path)?;
    let root = fs::canonicalize(root)?;
    if !absolute.starts_with(&root) || absolute == root {
        bail!("Unsafe profile deletion path");
    }
    fn inspect(p: &Path) -> Result<()> {
        check_path(p)?;
        if p.is_dir() {
            for entry in fs::read_dir(p)? {
                inspect(&entry?.path())?;
            }
        }
        Ok(())
    }
    inspect(&absolute)?;
    fs::remove_dir_all(absolute)?;
    Ok(())
}
pub fn open(target: impl AsRef<OsStr>) -> Result<()> {
    let value = wide(target);
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(value.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize <= 32 {
        bail!("Windows could not open the requested resource");
    }
    Ok(())
}
pub fn browser(url: &str) -> Result<()> {
    let parsed = reqwest::Url::parse(url)?;
    if parsed.scheme() != "https"
        && !(parsed.scheme() == "http"
            && matches!(parsed.host_str(), Some("localhost" | "127.0.0.1")))
    {
        bail!("Unsupported login URL");
    }
    open(url)
}
pub fn local_data() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("USERPROFILE").unwrap_or_default()).join("AppData/Local")
        })
}
pub fn active_home() -> PathBuf {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("USERPROFILE").unwrap_or_default()).join(".codex")
        })
}
pub fn startup(enabled: bool) -> Result<()> {
    unsafe {
        let mut key = HKEY::default();
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            0,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            None,
        )
        .ok()?;
        let result = if enabled {
            let command = wide(format!(
                "\"{}\" --background",
                std::env::current_exe()?.display()
            ));
            let bytes =
                std::slice::from_raw_parts(command.as_ptr().cast::<u8>(), command.len() * 2);
            RegSetValueExW(key, w!("CatCodexSwitcher"), 0, REG_SZ, Some(bytes)).ok()
        } else {
            let r = RegDeleteValueW(key, w!("CatCodexSwitcher"));
            if r == ERROR_FILE_NOT_FOUND {
                Ok(())
            } else {
                r.ok()
            }
        };
        let _ = RegCloseKey(key);
        result?;
    }
    Ok(())
}
pub fn taskbar_dark() -> bool {
    unsafe {
        let mut value: u32 = 1;
        let mut size = 4;
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut size),
        ) == ERROR_SUCCESS
            && value == 0
    }
}
pub struct Instance {
    mutex: HANDLE,
    activate: HANDLE,
}
pub struct OriginalGuard(HANDLE);
impl OriginalGuard {
    pub fn acquire(data: &Path) -> Result<Self> {
        use sha2::{Digest, Sha256};
        let absolute = if data.is_absolute() {
            data.to_owned()
        } else {
            std::env::current_dir()?.join(data)
        };
        let hash = Sha256::digest(
            absolute
                .to_string_lossy()
                .replace('/', "\\")
                .to_uppercase()
                .as_bytes(),
        );
        let name = wide(format!("Local\\CodexAccountSwitcher-{hash:X}"));
        unsafe {
            let handle = CreateMutexW(None, true, PCWSTR(name.as_ptr()))?;
            if GetLastError() == ERROR_ALREADY_EXISTS {
                let wait = WaitForSingleObject(handle, 0);
                if wait != WAIT_OBJECT_0 && wait != WAIT_ABANDONED {
                    let _ = CloseHandle(handle);
                    bail!(
                        "Quit the original Codex Account Switcher before using account operations or importing its data."
                    );
                }
            }
            Ok(Self(handle))
        }
    }
    pub fn default_location() -> Result<Self> {
        Self::acquire(
            &std::env::var_os("CODEX_SWITCHER_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| local_data().join("Codex Account Switcher")),
        )
    }
}
impl Drop for OriginalGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = ReleaseMutex(self.0);
            let _ = CloseHandle(self.0);
        }
    }
}
impl Instance {
    pub fn activation_handle(&self) -> isize {
        self.activate.0 as isize
    }
    pub fn acquire() -> Result<Option<Self>> {
        unsafe {
            let mutex = CreateMutexW(None, true, w!("Local\\CatCodexSwitcher"))?;
            let exists = GetLastError() == ERROR_ALREADY_EXISTS;
            let activate =
                CreateEventW(None, false, false, w!("Local\\CatCodexSwitcher-Activate"))?;
            if exists {
                SetEvent(activate)?;
                CloseHandle(activate)?;
                CloseHandle(mutex)?;
                return Ok(None);
            }
            Ok(Some(Self { mutex, activate }))
        }
    }
}
impl Drop for Instance {
    fn drop(&mut self) {
        unsafe {
            let _ = ReleaseMutex(self.mutex);
            let _ = CloseHandle(self.mutex);
            let _ = CloseHandle(self.activate);
        }
    }
}

#[derive(serde::Deserialize, Clone)]
pub struct Desktop {
    pub executable: PathBuf,
    #[serde(rename = "appId")]
    pub app_id: Option<String>,
}
impl Desktop {
    pub fn discover() -> Result<Self> {
        if let Some(p) = std::env::var_os("CODEX_DESKTOP_PATH") {
            let executable = PathBuf::from(p);
            if !executable.is_file() {
                bail!("Configured Desktop executable is missing");
            }
            return Ok(Self {
                executable,
                app_id: None,
            });
        }
        let mut store_error = None;
        match Self::store_installation() {
            Ok(Some(app)) => return Ok(app),
            Ok(None) => {}
            Err(error) => store_error = Some(error),
        }
        for p in ["Programs/Codex/Codex.exe", "OpenAI/Codex/Codex.exe"] {
            let executable = local_data().join(p);
            if executable.is_file() {
                return Ok(Self {
                    executable,
                    app_id: None,
                });
            }
        }
        if let Some(error) = store_error {
            return Err(error.context("Codex Desktop installation was not found"));
        }
        bail!("Codex Desktop installation was not found")
    }
    fn store_installation() -> Result<Option<Self>> {
        use windows::{
            Data::Xml::Dom::XmlDocument,
            Management::Deployment::PackageManager,
            Storage::{IStorageFile, StorageFile},
            Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize},
            core::{HSTRING, Interface},
        };
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        }
        struct Apartment;
        impl Drop for Apartment {
            fn drop(&mut self) {
                unsafe {
                    CoUninitialize();
                }
            }
        }
        let _apartment = Apartment;
        let manager = PackageManager::new()?;
        for package in manager.FindPackagesByUserSecurityId(&HSTRING::new())? {
            let id = package.Id()?;
            if id.Name()? != "OpenAI.Codex" {
                continue;
            }
            let location = PathBuf::from(package.InstalledLocation()?.Path()?.to_string());
            // The file loader handles the BOM and XML encoding declaration together.
            let manifest = StorageFile::GetFileFromPathAsync(&HSTRING::from(
                location.join("AppxManifest.xml").as_os_str(),
            ))?
            .get()?;
            let xml = XmlDocument::LoadFromFileAsync(&manifest.cast::<IStorageFile>()?)?.get()?;
            let app=xml.SelectSingleNode(&HSTRING::from("/*[local-name()='Package']/*[local-name()='Applications']/*[local-name()='Application'][1]"))?;
            let attr = app.Attributes()?;
            let executable = location.join(
                attr.GetNamedItem(&HSTRING::from("Executable"))?
                    .InnerText()?
                    .to_string(),
            );
            let app_id = format!(
                "{}!{}",
                id.FamilyName()?,
                attr.GetNamedItem(&HSTRING::from("Id"))?.InnerText()?
            );
            anyhow::ensure!(executable.is_file(), "Store Desktop executable is missing");
            return Ok(Some(Self {
                executable,
                app_id: Some(app_id),
            }));
        }
        Ok(None)
    }
    pub fn system_language() -> String {
        let mut name = [0u16; 85];
        let count = unsafe { windows::Win32::Globalization::GetUserDefaultLocaleName(&mut name) };
        if count > 0 {
            String::from_utf16_lossy(&name[..count as usize - 1]).to_lowercase()
        } else {
            "en".into()
        }
    }
    pub fn processes(&self) -> Result<Vec<u32>> {
        let executable =
            fs::canonicalize(&self.executable).context("Cannot resolve Desktop executable path")?;
        let mut ids = Vec::new();
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)?;
            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            let mut found = Process32FirstW(snap, &mut entry).is_ok();
            let result = (|| -> Result<()> {
                while found {
                    let name = String::from_utf16_lossy(&entry.szExeFile)
                        .trim_end_matches('\0')
                        .to_string();
                    if executable
                        .file_name()
                        .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(&name))
                    {
                        let handle = match OpenProcess(
                            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                            false,
                            entry.th32ProcessID,
                        ) {
                            Ok(handle) => handle,
                            Err(e)
                                if e.code()
                                    == windows::core::HRESULT::from_win32(
                                        ERROR_INVALID_PARAMETER.0,
                                    ) =>
                            {
                                found = Process32NextW(snap, &mut entry).is_ok();
                                continue;
                            }
                            Err(e) => {
                                return Err(e)
                                    .context("Cannot inspect Desktop process; close it manually");
                            }
                        };
                        if WaitForSingleObject(handle, 0) == WAIT_OBJECT_0 {
                            let _ = CloseHandle(handle);
                            found = Process32NextW(snap, &mut entry).is_ok();
                            continue;
                        }
                        let mut buffer = vec![0u16; 32768];
                        let mut length = buffer.len() as u32;
                        let path_result = QueryFullProcessImageNameW(
                            handle,
                            PROCESS_NAME_WIN32,
                            PWSTR(buffer.as_mut_ptr()),
                            &mut length,
                        );
                        let mut package_match = None;
                        if let Some(expected) = &self.app_id {
                            let mut size = 0;
                            if GetApplicationUserModelId(handle, &mut size, PWSTR::null())
                                == ERROR_INSUFFICIENT_BUFFER
                            {
                                let mut id = vec![0u16; size as usize];
                                if GetApplicationUserModelId(
                                    handle,
                                    &mut size,
                                    PWSTR(id.as_mut_ptr()),
                                ) == ERROR_SUCCESS
                                {
                                    package_match = Some(expected.eq_ignore_ascii_case(
                                        String::from_utf16_lossy(&id).trim_end_matches('\0'),
                                    ));
                                }
                            }
                        }
                        // Windows can revoke image-path access just before signaling process exit.
                        // A denied live process still fails closed after this bounded wait.
                        let exited = WaitForSingleObject(
                            handle,
                            if path_result.is_err() { 1000 } else { 0 },
                        ) == WAIT_OBJECT_0;
                        let _ = CloseHandle(handle);
                        if exited {
                            found = Process32NextW(snap, &mut entry).is_ok();
                            continue;
                        }
                        path_result.context("Cannot read the live Desktop executable path")?;
                        let matches = match package_match {
                            Some(matches) => matches,
                            None => {
                                let path = PathBuf::from(String::from_utf16_lossy(
                                    &buffer[..length as usize],
                                ));
                                let actual = fs::canonicalize(path)
                                    .context("Cannot resolve the live Desktop executable path")?;
                                executable
                                    .to_string_lossy()
                                    .eq_ignore_ascii_case(&actual.to_string_lossy())
                            }
                        };
                        if matches {
                            ids.push(entry.th32ProcessID);
                        }
                    }
                    found = Process32NextW(snap, &mut entry).is_ok();
                }
                Ok(())
            })();
            let _ = CloseHandle(snap);
            result?;
        }
        Ok(ids)
    }
    pub fn close(&self) -> Result<()> {
        let ids = self.processes()?;
        if ids.is_empty() {
            return Ok(());
        }
        struct CloseData {
            ids: Vec<u32>,
            sent: bool,
            failed: bool,
        }
        unsafe extern "system" fn close_window(hwnd: HWND, data: LPARAM) -> BOOL {
            let data = unsafe { &mut *(data.0 as *mut CloseData) };
            let mut pid = 0;
            unsafe {
                GetWindowThreadProcessId(hwnd, Some(&mut pid));
            }
            if data.ids.contains(&pid)
                && unsafe {
                    IsWindowVisible(hwnd).as_bool()
                        && GetWindow(hwnd, GW_OWNER).unwrap_or_default().0.is_null()
                }
            {
                data.sent = true;
                if unsafe { PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0)) }.is_err() {
                    data.failed = true;
                }
            }
            true.into()
        }
        let mut data = CloseData {
            ids,
            sent: false,
            failed: false,
        };
        unsafe {
            EnumWindows(
                Some(close_window),
                LPARAM((&mut data as *mut CloseData) as isize),
            )?;
        }
        if !data.sent || data.failed {
            bail!("Desktop has no closable window; quit it manually. Account unchanged.");
        }
        let start = Instant::now();
        while !self.processes()?.is_empty() {
            if start.elapsed() > Duration::from_secs(30) {
                bail!("Desktop did not exit within 30 seconds. Account unchanged.");
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        Ok(())
    }
    pub fn reopen(&self) -> Result<()> {
        if let Some(id) = &self.app_id {
            open(format!("shell:AppsFolder\\{id}"))?;
        } else {
            open(&self.executable)?;
        }
        let start = Instant::now();
        while self.processes()?.is_empty() {
            if start.elapsed() > Duration::from_secs(15) {
                bail!("Account switched, but Desktop did not start. Open it manually.");
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_write_keeps_the_external_directory_acl() {
        let temp = std::env::temp_dir();
        let root = temp.join(format!("switcher-external-{}", uuid::Uuid::new_v4()));
        let external = root.join("codex-home");
        fs::create_dir_all(&external).unwrap();
        let file = external.join("auth.json");
        let acl = || {
            let output = hidden_command("icacls").arg(&external).output().unwrap();
            assert!(output.status.success());
            output.stdout
        };
        let before = acl();
        atomic_write_external(&file, b"first").unwrap();
        atomic_write_external(&file, b"second").unwrap();
        assert_eq!(read(&file).unwrap(), b"second");
        assert_eq!(before, acl());
        let nested = external.join("nested").join("auth.json");
        atomic_write_external(&nested, b"nested").unwrap();
        assert_eq!(read(&nested).unwrap(), b"nested");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn desktop_process_matching_accepts_equivalent_paths() {
        let executable = std::env::current_exe().unwrap();
        // Give relative paths a stable base without changing other test threads' directory.
        if std::env::var_os("SWITCHER_PATH_TEST_CHILD").is_none() {
            let status = hidden_command(&executable)
                .args([
                    "--exact",
                    "platform::tests::desktop_process_matching_accepts_equivalent_paths",
                ])
                .current_dir(executable.parent().unwrap())
                .env("SWITCHER_PATH_TEST_CHILD", "1")
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        for path in [
            executable.clone(),
            PathBuf::from(executable.to_string_lossy().replace('\\', "/")),
            fs::canonicalize(&executable).unwrap(),
            PathBuf::from(executable.file_name().unwrap()),
            PathBuf::from(".").join(executable.file_name().unwrap()),
            PathBuf::from("..")
                .join(executable.parent().unwrap().file_name().unwrap())
                .join(executable.file_name().unwrap()),
        ] {
            let desktop = Desktop {
                executable: path,
                app_id: None,
            };
            assert!(
                desktop.processes().unwrap().contains(&std::process::id()),
                "Could not find this process using {:?}",
                desktop.executable
            );
        }
        let other = std::env::temp_dir().join(format!("switcher-path-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&other).unwrap();
        let other_executable = other.join(executable.file_name().unwrap());
        fs::write(&other_executable, b"Unrelated file with the same name").unwrap();
        let different = Desktop {
            executable: other_executable.clone(),
            app_id: None,
        };
        assert!(different.processes().unwrap().is_empty());
        fs::remove_file(other_executable).unwrap();
        fs::remove_dir(other).unwrap();
        let missing = Desktop {
            executable: executable.with_file_name("missing-desktop.exe"),
            app_id: None,
        };
        assert!(missing.processes().is_err());
    }
}
