fn main() {
  // tauri-build 只声明 tauri.conf.json/capabilities 为 rerun-if-changed，
  // 不监听图标文件；不声明的话改图标后 build.rs 不重跑，旧图标会一直
  // 嵌进 exe（Windows 资源在 resource.lib 里）。
  println!("cargo:rerun-if-changed=icons/icon.ico");
  tauri_build::build()
}
