use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn choose_file(save: bool, initial: &Path) -> Result<Option<PathBuf>, String> {
    #[cfg(target_os = "linux")]
    {
        let mut command = Command::new("zenity");
        command.args(["--file-selection", "--title=openpaint"]);
        command.arg(format!("--filename={}", initial.display()));
        if save {
            command.args(["--save", "--confirm-overwrite"]);
        }
        command.arg("--file-filter=images | *.opaint *.png *.bmp *.tga *.ico *.pnm *.ppm *.pgm *.webp *.dds");
        command.arg("--file-filter=all files | *");
        match run(&mut command) {
            Err(_) => {
                let mut fallback = Command::new("kdialog");
                fallback
                    .arg(if save {
                        "--getsavefilename"
                    } else {
                        "--getopenfilename"
                    })
                    .arg(initial);
                run(&mut fallback)
            }
            result => result,
        }
    }
    #[cfg(target_os = "macos")]
    {
        let script = if save {
            "on run argv\nset f to choose file name with prompt \"save image\" default name (item 1 of argv)\nreturn POSIX path of f\nend run"
        } else {
            "on run argv\nset f to choose file with prompt \"open image\"\nreturn POSIX path of f\nend run"
        };
        run(Command::new("osascript")
            .args(["-e", script, "--"])
            .arg(initial.file_name().unwrap_or_default()))
    }
    #[cfg(target_os = "windows")]
    {
        let script = if save {
            "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.SaveFileDialog; $d.FileName = $env:OPENPAINT_FILE; $d.Filter = 'openpaint project|*.opaint|PNG image|*.png|BMP image|*.bmp|TGA image|*.tga'; if ($d.ShowDialog() -eq 'OK') { [Console]::Write($d.FileName) }"
        } else {
            "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.OpenFileDialog; $d.Filter = 'supported images|*.opaint;*.png;*.bmp;*.tga;*.ico;*.pnm;*.webp;*.dds|all files|*.*'; if ($d.ShowDialog() -eq 'OK') { [Console]::Write($d.FileName) }"
        };
        run(Command::new("powershell.exe")
            .args(["-NoProfile", "-STA", "-Command", script])
            .env("OPENPAINT_FILE", initial))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = (save, initial);
        Err("native file dialogs are unavailable".into())
    }
}

fn run(command: &mut Command) -> Result<Option<PathBuf>, String> {
    let output = command
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| e.to_string())?;
    let value = String::from_utf8_lossy(&output.stdout)
        .trim_end_matches(['\r', '\n'])
        .to_string();
    if output.status.success() {
        return Ok((!value.is_empty()).then(|| PathBuf::from(value)));
    }
    if output.status.code() == Some(1) && output.stderr.is_empty() {
        return Ok(None);
    }
    let error = String::from_utf8_lossy(&output.stderr).to_string();
    if error.contains("User canceled") || error.contains("-128") {
        return Ok(None);
    }
    Err(if error.is_empty() {
        "file dialog failed".into()
    } else {
        error
    })
}
