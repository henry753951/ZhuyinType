# ZhuyinType for Linux and Windows (GPUI)

The Linux and Windows front end: one Rust process draws the settings window, the candidate window, the mode toast and
the setup wizard with [GPUI](https://www.gpui.rs/) (through `gpui-kit`), owns the engine, and serves the native input
method adapters. It replaces the Qt client (removed 2026-11).

The **functional layout is the macOS app's**: the same pages, sections, rows, sub-pages, wizard steps, candidate window
variants and links between them, in the same order with the same wording (`platforms/macos/Sources/ZhuyinTypeApp/UI/`).
The visual language is ZhuyinType's own (tokens in `crates/zhuyintype/src/theme.rs`), with the macOS app's motion
(durations, the 140 ms page turn, push/pop of sub-pages) and its hierarchy (grouped rows on cards). The window has no
system frame: the app draws its title bar and window controls, like Discord or Claude.

## Layout

```text
platforms/desktop/
  settings.json             setting descriptors shared by every page and the wizard (moved from platforms/qt)
  IPC.md                    the broker protocol between the host and the native adapters
  crates/zt-core/           no UI: settings, paths, engine session, input controller, shortcuts, usage statistics,
                            correction log, practice sessions, the broker (Unix socket / named pipe)
  crates/zhuyintype/        the GPUI app (binary `zhuyintype`): theme, ui/ components, pages/, candidate/, wizard/
  crates/zt-tsf/            Windows only: the TSF text service DLL and its registration, in Rust (`windows` crate)
  packaging/windows/        NSIS installer (zh-TW UI, one UAC prompt, registers TSF, autostart, uninstaller)
  packaging/linux/          nfpm .deb/.rpm, desktop entries, autostart, Fcitx5 addon config
  scripts/                  remote.sh (Mac → GPU host), build.sh (host: Linux + Windows + packages), screenshot.sh
platforms/linux/            the Fcitx5 addon (C++, talks to the broker; unchanged protocol)
```

## Building: on the GPU host only

Nothing is built on the Mac (the user's disk stays clean). From the Mac:

```sh
platforms/desktop/scripts/remote.sh cargo build                      # Linux debug build on the host
platforms/desktop/scripts/remote.sh cargo test -p zt-core
platforms/desktop/scripts/remote.sh scripts/screenshot.sh ~/zhuyintype/shots/NAME   # every page, light + dark
platforms/desktop/scripts/remote.sh --fetch zhuyintype/shots/NAME /tmp/shots       # copy them back to look at
```

Windows binaries are cross-compiled on the host with `cargo xwin build --target x86_64-pc-windows-msvc` (MSVC ABI,
clang-cl/lld, the Windows SDK fetched by xwin); the installer is made with `makensis` on Linux. Windows behaviour is
tested on the Proxmox VM `zt-windows` (ssh alias; Windows 11 zh-TW).

## Threads and the core ↔ UI boundary

```text
 Fcitx5 addon / TSF DLL ──IPC.md──▶ broker thread (zt-core) ──▶ InputController ──▶ EngineSession (zhuyin-core)
                                          │ UiEvent (async channel)
                                          ▼
                                GPUI main thread: candidate popup, mode toast, settings window
```

- The broker runs on its own thread and never waits for the GPUI thread, so the app's own text fields can use the
  input method (the TSF DLL in our process talks to our broker) without deadlock.
- `zt_core::service::Service` bundles the shared state (`Settings`, `EngineSession`, `InputController`, `Shortcuts`,
  `UsageStats`, `CorrectionLog`) behind `Arc`/`Mutex`; it is cheap to clone. The UI calls it directly for settings,
  personal data, statistics and candidate-window clicks (`Service::popup_action`).
- `Service::start` returns `async_channel::Receiver<UiEvent>`; the UI drains it on GPUI's foreground executor:
  - `UiEvent::Popup(Option<PopupState>)`: show/update/hide the candidate window (`PopupState` = presentation +
    caret rectangle in screen logical pixels + whether the app shows the composition inline);
  - `UiEvent::ModeToast { english, anchor }`; `UiEvent::ShowSettings`; `UiEvent::Boundary(direction)`.
- `zt_core::presentation::Presentation` is the one snapshot every view draws: composition, reading, UTF-16 cursor,
  clauses, the current page's candidate rows, selection, page/page count/page size, single-character and corrections
  flags, notice. The candidate panel element draws it identically in the floating window, the 視窗與提示 preview, the
  介紹 typing demo and the wizard.
- `PracticeSession` is an in-process sandbox (own `InputController`, in-memory personal data, the shared read-only
  engine data) for the 介紹 try-it field and the wizard's practice tasks, like macOS `PracticeField`.
- Keys cross the boundary as `zt_core::keys::Key` (Qt key code + Qt modifier flags + text + Shift side), the values
  the IPC protocol already uses; the UI converts GPUI keystrokes to it.

## Engine data and user files

The engine (`zhuyin-core`, used directly) loads, from the first directory with a lexicon among `ZT_DATA_DIR`, the
configured data directory, `<exe dir>/data` (Windows) or `/usr/share/zhuyintype/data` (Linux), and the checkout's
`data/build` (`zt_core::engine::DataFiles::search_path`):

- `lexicon.bin` (or `lexicon.tsv`), required;
- `lm/lm.bin` (or `lm/{unigram,bigram,backoff}.tsv`), the language model;
- `lm_chat/lm.bin` (or its TSV files), the chat-domain model, when shipped;
- `channel.bin`, the reading channel (「接受常見的其他讀法」), when shipped;
- the notices beside them for the 關於 page (`lm/NOTICE.md`, `lm/articles.tsv.gz`, `channel/NOTICE.md`, …).

`tools/zt data fetch engine && tools/zt data compile` makes them in `data/build`, as for the macOS app. The neural
models (reranker, character model) are Core ML on macOS and not loaded here; their seam is the composer's hooks
(`EngineSession::composer_mut`). Personal data, `stats.json` and `corrections.jsonl` live in `zt_core::paths::data_dir`;
settings and the keys in `zt_core::paths::settings_file`. Practice sessions open an engine of their own on the same
`EngineData` with personal data in memory; the mapped files are shared.

## UI rules

- Pages compose only `ui/` components and `theme` tokens; no colour, size or duration literal in `pages/`.
- Every page and sub-page matches its macOS counterpart's rows, order, wording and links. References: the Swift
  sources and the renders made by the macOS app (`ZhuyinType --render-ui DIR`, kept outside git in `.reference/`).
- Animations use `theme::motion` and are skipped when the system asks for reduced motion.
- The candidate window and the mode toast never take keyboard focus (Windows: `WS_EX_NOACTIVATE`; X11:
  override-redirect, no input focus).
- Headless runs (screenshots, tests) never open a window on the user's desktop; they run on the host's virtual
  display.
