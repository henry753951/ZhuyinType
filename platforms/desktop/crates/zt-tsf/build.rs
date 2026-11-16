//! Windows resources of zhuyintype_tsf.dll: the logo (icon 1, also the TSF profile's icon) and the version
//! information Explorer shows.  The .rc file is written here so it carries the workspace version without a
//! preprocessor (llvm-rc on the Linux build host).

use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=logo.ico");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let out = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let version = env::var("CARGO_PKG_VERSION").expect("cargo sets CARGO_PKG_VERSION");
    let mut parts: Vec<u32> = version.split(['.', '-']).take(3).map(|part| part.parse().unwrap_or(0)).collect();
    parts.resize(4, 0);
    let numeric = parts.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
    let icon = manifest.join("logo.ico").display().to_string().replace('\\', "/");
    let rc = format!(
        r#"#pragma code_page(65001)
1 ICON "{icon}"
1 VERSIONINFO
FILEVERSION {numeric}
PRODUCTVERSION {numeric}
FILEOS 0x40004
FILETYPE 0x2
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040404b0"
    BEGIN
      VALUE "CompanyName", "ZhuyinType"
      VALUE "FileDescription", "ZhuyinType 注音輸入法（TSF 文字服務）"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "zhuyintype-tsf"
      VALUE "LegalCopyright", "AGPL-3.0-only"
      VALUE "OriginalFilename", "zhuyintype-tsf.dll"
      VALUE "ProductName", "ZhuyinType"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x0404, 1200
  END
END
"#
    );
    let path = out.join("zt-tsf.rc");
    fs::write(&path, rc).expect("write zt-tsf.rc");
    embed_resource::compile_for_cdylib(&path, embed_resource::NONE)
        .manifest_required()
        .expect("compile zt-tsf.rc (llvm-rc)");
}
