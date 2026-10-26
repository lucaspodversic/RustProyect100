//! Binario `hbench`: `hbench http://localhost:8080/ -n 1000 -c 50`.

use benchmark_http::{correr, Config, Url};
use clap::Parser;
use std::process::ExitCode;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "hbench", version, about = "Benchmark HTTP concurrente (tipo hey/wrk)")]
struct Args {
    /// URL http:// a medir.
    url: String,
    /// Cantidad total de requests.
    #[arg(short = 'n', long, default_value_t = 200)]
    total: u64,
    /// Workers concurrentes (cada uno con su conexión keep-alive).
    #[arg(short, long, default_value_t = 10)]
    concurrencia: usize,
    /// Timeout por request, en segundos.
    #[arg(short, long, default_value_t = 10)]
    timeout: u64,
}

#[tokio::main]
async fn main() -> ExitCode {
    let a = Args::parse();
    let url = match Url::parsear(&a.url) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    println!("Midiendo {} con {} requests y {} workers...", a.url, a.total, a.concurrencia);
    let inf = correr(Config { url, total: a.total, concurrencia: a.concurrencia, timeout: Duration::from_secs(a.timeout) }).await;
    println!("{}", inf.texto());
    if inf.errores == inf.total { ExitCode::from(1) } else { ExitCode::SUCCESS }
}
