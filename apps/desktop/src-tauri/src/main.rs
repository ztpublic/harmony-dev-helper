use hdc_bridge_rs::{run_bridge, DEFAULT_WS_ADDR};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().build())
        .setup(|_app| {
            tauri::async_runtime::spawn(async {
                if let Err(error) = run_bridge(DEFAULT_WS_ADDR).await {
                    eprintln!("{error}");
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
