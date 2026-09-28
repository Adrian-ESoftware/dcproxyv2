//! fol-discord — corrige o problema do Discord no Brasil (Linux).
//!
//! O Discord decide a região da sua sessão pelo IP que enxerga na abertura.
//! Em vários provedores brasileiros essa decisão sai errada e a transmissão de
//! tela para de funcionar. Este programa faz o mesmo que ligar uma VPN para
//! abrir o Discord e desligá-la assim que ele entrou: enquanto a sessão está
//! nascendo, o tráfego do Discord que decide a região sai por um IP
//! estrangeiro; depois, tudo volta a sair direto, com o ping de sempre, e a
//! região fica gravada na sessão. A voz, a câmera e a tela são UDP e nunca
//! passam por aqui — nem o TCP dos servidores de voz, que não decide região
//! nenhuma, sai do país.

mod discord;
mod linux;
mod pac;
mod pool;
mod processos;
mod routing;
mod sessao;
mod socks;

use linux as os;

use anyhow::{Context, Result};
use std::{ffi::OsStr, path::PathBuf, process::Command, time::Duration};

const PORTA_SOCKS: u16 = 9250;
const PORTA_PAC: u16 = 9251;
const INTERVALO_MANUTENCAO: Duration = Duration::from_secs(300);

/// Passada do vigia da sessão. Curto o bastante para a janela fechar logo
/// depois do silêncio e para o reinício do Discord ser percebido na hora.
const INTERVALO_VIGIA: Duration = Duration::from_secs(1);

#[derive(Debug, PartialEq, Eq)]
struct OpcoesInstalar {
    reiniciar_discord: bool,
    criar_autostart: bool,
}

fn opcoes_instalar(args: &[String]) -> OpcoesInstalar {
    OpcoesInstalar {
        reiniciar_discord: !args.iter().any(|arg| arg == "--sem-reiniciar"),
        criar_autostart: !args.iter().any(|arg| arg == "--sem-autostart"),
    }
}

fn manter_arquivos(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--manter-arquivos")
}

fn url_pac() -> String {
    format!("http://127.0.0.1:{PORTA_PAC}/proxy.pac")
}

pub fn pasta_dados() -> PathBuf {
    let base = std::env::var("XDG_DATA_HOME")
        .or_else(|_| std::env::var("HOME").map(|h| format!("{h}/.local/share")))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(base).join("fol-discord")
}

pub fn caminho_log() -> PathBuf {
    pasta_dados().join("fol.log")
}

/// Marcador escrito pelo serviço quando a piscina tem proxies utilizáveis.
pub fn caminho_marcador() -> PathBuf {
    pasta_dados().join("pronto")
}

/// Instante da última passada de manutenção da piscina, em milissegundos de época.
pub fn caminho_ultima_validacao() -> PathBuf {
    pasta_dados().join("ultima-validacao-ms")
}

fn piscina_pronta() -> bool {
    caminho_marcador().exists()
}

fn milissegundos_agora() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn registrar_checagem_em(caminho: &std::path::Path, instante: u128) {
    let _ = std::fs::create_dir_all(caminho.parent().unwrap_or(caminho));
    let _ = std::fs::write(caminho, format!("{instante}\n"));
}

fn caminho_instalado() -> PathBuf {
    pasta_dados().join("fol-discord")
}

fn comando_oculto(programa: impl AsRef<OsStr>) -> Command {
    Command::new(programa)
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let comando = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(|s| s.as_str())
        .unwrap_or("ajuda");

    match comando {
        "instalar" => instalar(opcoes_instalar(&args)),
        "desinstalar" => desinstalar(manter_arquivos(&args)),
        "status" => status(),
        "reiniciar-discord" => reiniciar_discord(),
        "rodar" => rodar(),
        _ => {
            ajuda();
            Ok(())
        }
    }
}

fn ajuda() {
    println!(
        "\nfol-discord {}\n\n\
         Uso:\n  \
         fol-discord instalar      liga a correção, reinicia o Discord e sobe com o sistema\n  \
         fol-discord desinstalar   remove tudo, sem deixar rastro\n  \
         fol-discord status        mostra o estado atual\n  \
         fol-discord reiniciar-discord fecha e abre só o Discord\n  \
         fol-discord rodar         roda em primeiro plano (para depurar)\n\n\
         Opções:\n  \
         --sem-reiniciar           não mexe no Discord aberto; a correção vale na\n                            \
         próxima vez que você abrir\n  \
         --sem-autostart           não cria a entrada de inicialização automática\n  \
         --manter-arquivos         limpa a configuração sem apagar a pasta instalada\n",
        env!("CARGO_PKG_VERSION")
    );
}

fn instalar(opcoes: OpcoesInstalar) -> Result<()> {
    let destino = caminho_instalado();
    std::fs::create_dir_all(pasta_dados()).context("criando a pasta de dados")?;

    let atual = std::env::current_exe()?;
    if atual != destino {
        encerrar_outras_instancias();
        std::fs::copy(&atual, &destino).context("copiando o executável")?;
    }

    if opcoes.criar_autostart {
        os::ativar_autostart(&format!("\"{}\" rodar", destino.display()))
            .context("registrando o autostart")?;
    }
    os::ativar_pac(&url_pac()).context("ligando o proxy automático")?;
    let _ = os::adicionar_ao_path(&pasta_dados().display().to_string());

    let _ = std::fs::remove_file(caminho_marcador());
    let _ = std::fs::remove_file(caminho_ultima_validacao());

    comando_oculto(&destino)
        .arg("rodar")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("subindo o serviço")?;

    print!("Validando proxies");
    let _ = std::io::Write::flush(&mut std::io::stdout());
    let mut pronta = false;
    for _ in 0..15 {
        std::thread::sleep(Duration::from_secs(4));
        print!(".");
        let _ = std::io::Write::flush(&mut std::io::stdout());
        if porta_ocupada(PORTA_SOCKS) && piscina_pronta() {
            pronta = true;
            break;
        }
    }
    println!();
    if !pronta {
        println!("\nNenhum proxy respondeu a tempo. O serviço continua tentando");
        println!("a cada 5 minutos — confira depois com `fol-discord status`.");
    }

    println!("\nInstalado.\n");
    println!("  executável : {}", destino.display());
    println!("  log        : {}", caminho_log().display());
    println!(
        "  autostart  : {}",
        if opcoes.criar_autostart {
            "sim (systemd user service + XDG)"
        } else {
            "desativado"
        }
    );
    println!("  PAC        : {}", url_pac());

    if opcoes.reiniciar_discord {
        match discord::reiniciar() {
            Ok(true) => println!("\nDiscord reiniciado. Já está valendo."),
            Ok(false) => println!(
                "\nDiscord não encontrado — a correção vale na próxima vez que você abrir."
            ),
            Err(e) => {
                println!("\nNão consegui reiniciar o Discord ({e}). Feche e abra ele uma vez.")
            }
        }
    } else {
        println!("\nFeche e abra o Discord uma vez.");
    }

    println!("\nEm um terminal novo, o comando `fol-discord` já funciona sozinho.");
    Ok(())
}

fn desinstalar(manter_arquivos: bool) -> Result<()> {
    os::validar_autostart_do_fol(&caminho_instalado())?;
    os::desativar_pac().context("devolvendo o proxy automático")?;
    os::desativar_autostart(&caminho_instalado()).context("removendo o autostart")?;
    let _ = os::remover_do_path(&pasta_dados().display().to_string());
    encerrar_outras_instancias();

    let estava_aberto = discord::encerrar_se_aberto();
    if !manter_arquivos {
        let _ = std::fs::remove_dir_all(pasta_dados());
    }

    println!(
        "{} O proxy automático do sistema voltou ao que era antes.",
        if manter_arquivos {
            "Configuração removida; os arquivos foram preservados."
        } else {
            "Removido."
        }
    );
    if estava_aberto {
        println!("O Discord foi fechado. Abra de novo e ele já sai pelo seu IP normal.");
    } else {
        println!("Na próxima abertura, o Discord já sai pelo seu IP normal.");
    }
    Ok(())
}

fn status() -> Result<()> {
    println!("\nfol-discord {}\n", env!("CARGO_PKG_VERSION"));
    println!("  instalado  : {}", sim_nao(caminho_instalado().exists()));
    println!("  autostart  : {}", sim_nao(os::autostart_ativo()));
    println!("  PAC ligado : {}", sim_nao(os::pac_ativo(&url_pac())));
    println!("  rodando    : {}", sim_nao(porta_ocupada(PORTA_SOCKS)));
    println!(
        "  no PATH    : {}",
        sim_nao(os::path_ativo(&pasta_dados().display().to_string()))
    );
    println!("  proxies    : {}", sim_nao(piscina_pronta()));
    println!("  log        : {}", caminho_log().display());
    Ok(())
}

fn reiniciar_discord() -> Result<()> {
    match discord::reiniciar()? {
        true => println!("Discord reiniciado."),
        false => println!("Discord não encontrado."),
    }
    Ok(())
}

fn encerrar_outras_instancias() {
    let eu = std::process::id();
    let antigas: Vec<u32> = processos::pids_por_nome("fol-discord")
        .into_iter()
        .filter(|pid| *pid != eu)
        .collect();
    processos::encerrar_todos(&antigas);
}

fn sim_nao(b: bool) -> &'static str {
    if b {
        "sim"
    } else {
        "não"
    }
}

fn porta_ocupada(porta: u16) -> bool {
    std::net::TcpStream::connect_timeout(
        &format!("127.0.0.1:{porta}").parse().unwrap(),
        Duration::from_millis(300),
    )
    .is_ok()
}

fn vigiar_sessao(sessao: std::sync::Arc<sessao::Sessao>, piscina: pool::Pool) {
    std::thread::spawn(move || loop {
        let agora = std::time::Instant::now();

        match sessao.observar_discord(discord::principal(), agora) {
            Some(sessao::Mudanca::DiscordNovo) => {
                socks::log::linha("Discord novo no ar; a correção vale para esta sessão");
            }
            Some(sessao::Mudanca::DiscordFechou) => {
                socks::log::linha("Discord fechou; a janela reabre para a próxima abertura");
            }
            None => {}
        }

        if sessao.avaliar(agora, piscina.quantidade() > 0) {
            let duracao = sessao.armada_ha(agora).as_secs();
            socks::log::linha(&format!(
                "sessão aberta após {duracao} s; o Discord volta a falar direto"
            ));
        }

        std::thread::sleep(INTERVALO_VIGIA);
    });
}

fn rodar() -> Result<()> {
    let _ = std::fs::create_dir_all(pasta_dados());
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    rt.block_on(async move {
        let piscina = pool::Pool::nova();
        let sessao = std::sync::Arc::new(sessao::Sessao::nova(std::time::Instant::now()));
        vigiar_sessao(sessao.clone(), piscina.clone());

        tokio::spawn({
            let p = piscina.clone();
            async move {
                loop {
                    if p.quantidade() < pool::MINIMO_SAUDAVEIS {
                        socks::log::linha("reabastecendo a piscina de proxies...");
                        match p.reabastecer().await {
                            Ok(n) => {
                                socks::log::linha(&format!("{n} proxies estrangeiros validados"));
                                for u in p.listar().iter().take(3) {
                                    socks::log::linha(&format!(
                                        "  {} ({}) {}ms",
                                        u.endereco,
                                        u.regiao,
                                        u.latencia.as_millis()
                                    ));
                                }
                            }
                            Err(e) => socks::log::linha(&format!("falha ao reabastecer: {e}")),
                        }
                    }

                    if p.quantidade() > 0 {
                        let _ = std::fs::write(caminho_marcador(), b"");
                    } else {
                        let _ = std::fs::remove_file(caminho_marcador());
                    }

                    registrar_checagem_em(&caminho_ultima_validacao(), milissegundos_agora());

                    tokio::select! {
                        _ = tokio::time::sleep(INTERVALO_MANUTENCAO) => {}
                        _ = p.esperar_secar() => {
                            socks::log::linha("a piscina ficou magra; a manutenção acordou antes da hora");
                        }
                    }
                }
            }
        });

        tokio::spawn(async move {
            if let Err(e) = pac::servir(PORTA_PAC, PORTA_SOCKS).await {
                socks::log::linha(&format!("servidor PAC caiu: {e}"));
            }
        });

        socks::servir(PORTA_SOCKS, piscina, sessao).await
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_instala_o_servico() {
        assert_eq!(
            opcoes_instalar(&[
                "instalar".into(),
                "--sem-reiniciar".into(),
                "--sem-autostart".into(),
            ]),
            OpcoesInstalar {
                reiniciar_discord: false,
                criar_autostart: false,
            },
        );
    }

    #[test]
    fn cli_sem_opcoes_ativa_tudo() {
        assert_eq!(
            opcoes_instalar(&["instalar".into()]),
            OpcoesInstalar {
                reiniciar_discord: true,
                criar_autostart: true,
            },
        );
    }

    #[test]
    fn desinstalar_com_manter_arquivos_nao_remove_a_pasta() {
        assert!(manter_arquivos(&[
            "desinstalar".into(),
            "--manter-arquivos".into()
        ]));
        assert!(!manter_arquivos(&["desinstalar".into()]));
    }

    #[test]
    fn a_manutencao_carimba_a_hora() {
        let diretorio = tempfile::tempdir().unwrap();
        let caminho = diretorio.path().join("sub").join("ultima-validacao-ms");

        registrar_checagem_em(&caminho, 1_725_000_123_456);

        assert_eq!(
            std::fs::read_to_string(&caminho).unwrap(),
            "1725000123456\n"
        );
        assert_eq!(
            std::fs::read_to_string(&caminho)
                .unwrap()
                .trim()
                .parse::<u64>()
                .unwrap(),
            1_725_000_123_456
        );
    }
}
