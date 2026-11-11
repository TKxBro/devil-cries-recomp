use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::time::Instant;

pub enum BuildMsg {
    Progress(String, f32),
    Log(String),
    Success(PathBuf),
    Failed(String),
}

pub struct BuildRunner {
    pub rx: Receiver<BuildMsg>,
}

impl BuildRunner {
    pub fn start(input: PathBuf, out_dir: PathBuf, jobs: usize) -> Self {
        let (tx, rx) = channel();

        std::thread::spawn(move || {
            let start_time = Instant::now();
            let _ = tx.send(BuildMsg::Progress("Verificando arquivo de entrada...".to_string(), 0.1));
            let _ = tx.send(BuildMsg::Log(format!("Origem: {}", input.display())));
            let _ = tx.send(BuildMsg::Log(format!("Destino: {}", out_dir.display())));

            if !input.exists() {
                let _ = tx.send(BuildMsg::Failed("Arquivo ou diretório de entrada não encontrado.".into()));
                return;
            }

            // Create target dir
            if let Err(e) = std::fs::create_dir_all(&out_dir) {
                let _ = tx.send(BuildMsg::Failed(format!("Erro ao criar pasta de destino: {e}")));
                return;
            }

            let _ = tx.send(BuildMsg::Progress("Iniciando recompilador Kirikae (Devil Cries Recomp)...".to_string(), 0.25));

            let cli = find_cli();
            let _ = tx.send(BuildMsg::Log(format!("Usando backend: {}", cli.display())));

            let mut cmd = Command::new(&cli);
            cmd.arg("build")
                .arg(&input)
                .arg("-o")
                .arg(&out_dir)
                .arg("--name")
                .arg("base")
                .arg("-j")
                .arg(jobs.to_string());

            cmd.env("KIRIKAE_FFMPEG_H264", "1");

            cmd.stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(BuildMsg::Failed(format!("Falha ao iniciar processo de build: {e}")));
                    return;
                }
            };

            let _ = tx.send(BuildMsg::Progress("Recompilando código nativo e traduzindo shaders...".to_string(), 0.5));

            if let Some(stdout) = child.stdout.take() {
                let tx_c = tx.clone();
                std::thread::spawn(move || {
                    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                        let _ = tx_c.send(BuildMsg::Log(line));
                    }
                });
            }

            if let Some(stderr) = child.stderr.take() {
                let tx_c = tx.clone();
                std::thread::spawn(move || {
                    for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                        let _ = tx_c.send(BuildMsg::Log(format!("[LOG] {line}")));
                    }
                });
            }

            match child.wait() {
                Ok(status) if status.success() => {
                    let exe = out_dir.join("base");
                    let elapsed = start_time.elapsed().as_secs();
                    let _ = tx.send(BuildMsg::Progress("Recompilação concluída com sucesso!".to_string(), 1.0));
                    let _ = tx.send(BuildMsg::Log(format!("Jogo compilado em {elapsed}s: {}", exe.display())));
                    let _ = tx.send(BuildMsg::Success(exe));
                }
                Ok(status) => {
                    let _ = tx.send(BuildMsg::Failed(format!("Processo encerrou com código de erro {status}")));
                }
                Err(e) => {
                    let _ = tx.send(BuildMsg::Failed(format!("Erro ao aguardar processo: {e}")));
                }
            }
        });

        Self { rx }
    }
}

fn find_cli() -> PathBuf {
    let current = std::env::current_exe().unwrap_or_default();
    let beside = current.parent().map(|p| p.join("kirikae")).unwrap_or_default();
    if beside.is_file() {
        return beside;
    }
    // Check target/release or fallback
    let fallback = PathBuf::from("./target/release/kirikae");
    if fallback.is_file() {
        return fallback;
    }
    PathBuf::from("kirikae")
}
