//! A stand-in broker for testing the text service without the app: the same pipe and protocol
//! (platforms/desktop/IPC.md), a tiny zhuyin "engine" (standard layout keys to symbols; Space or a tone key commits,
//! 「ㄓㄨㄥ」 becomes 中), and a log of every request.  Test machines only: it logs what is typed.
//!
//!   mock_broker.exe [--pipe \\.\pipe\NAME] [--log FILE] [--delay-ms N]
//!
//! The pipe allows the user's AppContainer apps too, so a test can see whether immersive apps reach it.  `--delay-ms`
//! answers every key late (the text service gives up after 200 ms).

#[cfg(not(windows))]
fn main() {
    eprintln!("mock_broker runs on Windows");
}

#[cfg(windows)]
fn main() {
    windows_main::run();
}

#[cfg(windows)]
mod windows_main {
    use std::collections::HashMap;
    use std::fs::OpenOptions;
    use std::io::Write as _;
    use std::sync::{Arc, Mutex};

    use serde_json::{Value, json};
    use windows::Win32::Foundation::{CloseHandle, HANDLE, HLOCAL, LocalFree};
    use windows::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
    use windows::Win32::Storage::FileSystem::{FlushFileBuffers, PIPE_ACCESS_DUPLEX, ReadFile, WriteFile};
    use windows::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
        PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
    };
    use windows::core::{HSTRING, w};

    struct Field {
        preedit: String,
        english: bool,
    }

    struct State {
        fields: HashMap<String, Field>,
        log: Option<std::fs::File>,
        delay_ms: u64,
    }

    fn symbol(key: i64, shift: bool) -> Option<&'static str> {
        if shift {
            return None;
        }
        let c = u8::try_from(key).ok()? as char;
        Some(match c.to_ascii_uppercase() {
            '1' => "ㄅ",
            'Q' => "ㄆ",
            'A' => "ㄇ",
            'Z' => "ㄈ",
            '2' => "ㄉ",
            'W' => "ㄊ",
            'S' => "ㄋ",
            'X' => "ㄌ",
            'E' => "ㄍ",
            'D' => "ㄎ",
            'C' => "ㄏ",
            'R' => "ㄐ",
            'F' => "ㄑ",
            'V' => "ㄒ",
            '5' => "ㄓ",
            'T' => "ㄔ",
            'G' => "ㄕ",
            'B' => "ㄖ",
            'Y' => "ㄗ",
            'H' => "ㄘ",
            'N' => "ㄙ",
            'U' => "ㄧ",
            'J' => "ㄨ",
            'M' => "ㄩ",
            '8' => "ㄚ",
            'I' => "ㄛ",
            'K' => "ㄜ",
            ',' => "ㄝ",
            '9' => "ㄞ",
            'O' => "ㄟ",
            'L' => "ㄠ",
            '.' => "ㄡ",
            '0' => "ㄢ",
            'P' => "ㄣ",
            ';' => "ㄤ",
            '/' => "ㄥ",
            '-' => "ㄦ",
            '6' => "ˊ",
            '3' => "ˇ",
            '4' => "ˋ",
            '7' => "˙",
            _ => return None,
        })
    }

    fn convert(reading: &str) -> String {
        match reading {
            "ㄓㄨㄥ" => "中".into(),
            "ㄨㄣˊ" => "文".into(),
            "ㄓㄨˋ" => "注".into(),
            "ㄧㄣ" => "音".into(),
            other => other.trim_end_matches(['ˊ', 'ˇ', 'ˋ', '˙']).into(),
        }
    }

    fn handle(state: &mut State, request: &Value) -> Value {
        let id = request["id"].clone();
        let context = request["context"].as_str().unwrap_or("").to_owned();
        let op = request["op"].as_str().unwrap_or("");
        let field = state.fields.entry(context.clone()).or_insert(Field { preedit: String::new(), english: false });
        let mut handled = false;
        let mut commit = String::new();
        match op {
            "key" => {
                let key = request["key"].as_i64().unwrap_or(0);
                let modifiers = request["modifiers"].as_i64().unwrap_or(0);
                let release = request["release"].as_bool().unwrap_or(false);
                let chord = modifiers & (0x0400_0000 | 0x0800_0000 | 0x1000_0000) != 0;
                let composing = !field.preedit.is_empty();
                if key == 0x0100_0020 {
                    if release && !composing {
                        field.english = !field.english;
                    }
                    handled = true;
                } else if release {
                    handled = composing;
                } else if field.english || chord {
                    handled = false;
                } else if composing && (key == 0x20 || matches!(key as u8, b'3' | b'4' | b'6' | b'7')) {
                    if key != 0x20 {
                        field.preedit.push_str(symbol(key, false).unwrap_or(""));
                    }
                    commit = convert(&field.preedit);
                    field.preedit.clear();
                    handled = true;
                } else if composing && (key == 0x0100_0004 || key == 0x0100_0005) {
                    commit = std::mem::take(&mut field.preedit);
                    handled = true;
                } else if composing && key == 0x0100_0003 {
                    field.preedit.pop();
                    handled = true;
                } else if composing && key == 0x0100_0000 {
                    field.preedit.clear();
                    handled = true;
                } else if let Some(symbol) = symbol(key, modifiers & 0x0200_0000 != 0) {
                    field.preedit.push_str(symbol);
                    handled = true;
                } else {
                    handled = composing;
                }
                if state.delay_ms > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(state.delay_ms));
                }
            }
            "blur" => commit = std::mem::take(&mut field.preedit),
            "reset" => field.preedit.clear(),
            "mode" => {
                field.english = request["english"].as_bool().unwrap_or(field.english);
                handled = true;
            }
            "secure" if request["secure"].as_bool() == Some(true) => field.preedit.clear(),
            _ => {}
        }
        let field = &state.fields[&context];
        let reply = json!({
            "v": 1, "id": id, "context": context, "ok": true, "handled": handled, "commit": commit,
            "preedit": field.preedit, "cursor": field.preedit.encode_utf16().count(), "candidates": false,
            "english": field.english, "composing": !field.preedit.is_empty(),
        });
        if let Some(log) = state.log.as_mut() {
            let key =
                request.get("key").map(|key| format!(" key=0x{:x}", key.as_i64().unwrap_or(0))).unwrap_or_default();
            let release = if request["release"].as_bool() == Some(true) { " up" } else { "" };
            let _ = writeln!(
                log,
                "{} op={op}{key}{release} secure={} handled={handled} preedit={} commit={}",
                context.get(..9).unwrap_or(&context),
                request["secure"],
                field.preedit,
                commit
            );
        }
        reply
    }

    fn serve(pipe: HANDLE, state: Arc<Mutex<State>>) {
        let mut pending = Vec::new();
        let mut buffer = [0u8; 4096];
        loop {
            let mut read = 0u32;
            // SAFETY: a connected pipe and a local buffer.
            if unsafe { ReadFile(pipe, Some(&mut buffer), Some(&mut read), None) }.is_err() || read == 0 {
                break;
            }
            pending.extend_from_slice(&buffer[..read as usize]);
            while let Some(end) = pending.iter().position(|&byte| byte == b'\n') {
                let line: Vec<u8> = pending.drain(..=end).collect();
                let reply = match serde_json::from_slice::<Value>(&line[..line.len() - 1]) {
                    Ok(request) => handle(&mut state.lock().unwrap(), &request),
                    Err(_) => json!({"v": 1, "ok": false, "error": "bad json"}),
                };
                let mut bytes = serde_json::to_vec(&reply).unwrap();
                bytes.push(b'\n');
                let mut written = 0u32;
                // SAFETY: as above.
                if unsafe { WriteFile(pipe, Some(&bytes), Some(&mut written), None) }.is_err() {
                    return;
                }
            }
        }
        // SAFETY: the handle is ours.
        unsafe {
            let _ = FlushFileBuffers(pipe);
            let _ = DisconnectNamedPipe(pipe);
            let _ = CloseHandle(pipe);
        }
        if let Some(log) = state.lock().unwrap().log.as_mut() {
            let _ = writeln!(log, "disconnected");
        }
    }

    fn default_pipe() -> String {
        // The same name the text service derives (its broker module), from whoami's SID.
        let output = std::process::Command::new("whoami").arg("/user").arg("/fo").arg("csv").arg("/nh").output();
        let sid = output
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .and_then(|text| text.trim().rsplit(',').next().map(|sid| sid.trim_matches('"').to_owned()))
            .unwrap_or_default();
        format!("\\\\.\\pipe\\ZhuyinType-ime-v1-{sid}")
    }

    pub(super) fn run() {
        let mut args = std::env::args().skip(1);
        let (mut pipe_name, mut log, mut delay_ms) = (default_pipe(), None, 0);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--pipe" => pipe_name = args.next().unwrap_or(pipe_name),
                "--log" => log = args.next(),
                "--delay-ms" => delay_ms = args.next().and_then(|value| value.parse().ok()).unwrap_or(0),
                _ => {}
            }
        }
        let log = log.map(|path| OpenOptions::new().create(true).append(true).open(path).expect("open the log"));
        let state = Arc::new(Mutex::new(State { fields: HashMap::new(), log, delay_ms }));
        if let Some(log) = state.lock().unwrap().log.as_mut() {
            let _ = writeln!(log, "listening on {pipe_name}");
        }
        // Everyone, AppContainers (all and restricted) and low integrity: a test pipe, not the product's.
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        // SAFETY: a constant SDDL string; the descriptor lives for the whole process.
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                w!("D:(A;;GA;;;WD)(A;;GA;;;AC)(A;;GA;;;S-1-15-2-2)S:(ML;;NW;;;LW)"),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
        }
        .expect("security descriptor");
        let attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: false.into(),
        };
        let name = HSTRING::from(pipe_name);
        loop {
            // SAFETY: valid name and attributes.
            let pipe = unsafe {
                CreateNamedPipeW(
                    &name,
                    PIPE_ACCESS_DUPLEX,
                    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                    PIPE_UNLIMITED_INSTANCES,
                    65536,
                    65536,
                    0,
                    Some(&attributes),
                )
            };
            if pipe.is_invalid() {
                eprintln!("CreateNamedPipeW failed");
                std::thread::sleep(std::time::Duration::from_secs(1));
                continue;
            }
            // SAFETY: a fresh pipe instance; an already connected client is fine.
            let _ = unsafe { ConnectNamedPipe(pipe, None) };
            let state = Arc::clone(&state);
            let pipe = pipe.0 as usize;
            std::thread::spawn(move || serve(HANDLE(pipe as *mut _), state));
        }
        #[allow(unreachable_code)]
        // SAFETY: allocated by the conversion above.
        unsafe {
            LocalFree(Some(HLOCAL(descriptor.0)));
        }
    }
}
