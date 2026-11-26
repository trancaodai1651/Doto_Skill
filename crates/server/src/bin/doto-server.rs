//! `doto-server`: chạy máy chủ Doto Skill không giao diện (dev, CI, E2E, máy chủ headless).
//! In ra URL đăng nhập desktop dùng một lần cho listener Control.

use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "doto-server", version, about = "Doto Skill — máy chủ web/API cục bộ")]
struct Args {
    /// Thư mục dữ liệu (mặc định theo OS hoặc DOTO_DATA_DIR)
    #[arg(long, env = "DOTO_DATA_DIR")]
    data_dir: Option<PathBuf>,
    /// Cổng listener Control (chỉ 127.0.0.1). 0 = ngẫu nhiên.
    #[arg(long, default_value_t = 0)]
    control_port: u16,
    /// Thư mục giao diện web đã build (apps/web/dist)
    #[arg(long, env = "DOTO_WEB_ROOT")]
    web_root: Option<PathBuf>,
    /// Ghi URL đăng nhập desktop vào file này (thay vì chỉ in ra stdout) — dùng cho E2E
    #[arg(long)]
    token_file: Option<PathBuf>,
    /// Bật public server theo cấu hình đã lưu khi khởi động
    #[arg(long, default_value_t = true)]
    autostart: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,doto=debug".into())).init();
    let args = Args::parse();
    let web_root = args.web_root.or_else(|| {
        let p = PathBuf::from("apps/web/dist");
        p.join("index.html").is_file().then_some(p)
    });
    let st = doto_server::AppState::init(doto_server::Options { data_dir: args.data_dir, web_root, ..Default::default() }).await?;
    let addr = st.start_control(args.control_port).await?;
    let token = st.issue_desktop_token();
    let url = format!("http://{addr}/#desktop-token={token}");
    println!("Doto Skill {} — Control: http://{addr}", doto_server::VERSION);
    println!("Mở cửa sổ quản trị (dùng một lần): {url}");
    if let Some(f) = args.token_file {
        std::fs::write(f, &url)?;
    }
    if args.autostart {
        st.autostart().await;
    }
    tokio::signal::ctrl_c().await?;
    println!("Đang tắt…");
    st.shutdown().await;
    Ok(())
}
