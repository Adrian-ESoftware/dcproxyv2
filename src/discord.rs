//! Localiza e reinicia o Discord no Linux.

use anyhow::Result;
use std::{path::PathBuf, process::Command, time::Duration};

use crate::{processos::Processo, sessao::Identidade};

const IMAGEM: &str = "discord";

/// Lançador do Discord no Linux.
pub fn lancador() -> Option<PathBuf> {
    let caminhos = [
        "/usr/bin/discord",
        "/usr/bin/Discord",
        "/opt/discord/Discord",
        "/usr/local/bin/discord",
    ];
    for c in caminhos {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(caminho) = std::env::var("PATH") {
        for p in std::env::split_paths(&caminho) {
            let d = p.join("discord");
            if d.is_file() {
                return Some(d);
            }
        }
    }
    None
}

pub fn esta_rodando() -> bool {
    crate::processos::esta_rodando(IMAGEM)
}

/// O processo principal do Discord no ar, com a hora em que nasceu.
///
/// O Discord roda vários processos no Linux (principal, GPU, renderizadores, etc.).
/// O principal é o único cujo pai não é outro processo do Discord.
pub fn principal() -> Option<Identidade> {
    principal_entre(
        &crate::processos::processos_por_nome(IMAGEM),
        crate::processos::criado_em,
    )
}

fn principal_entre(
    processos: &[Processo],
    criado_em: impl Fn(u32) -> Option<u64>,
) -> Option<Identidade> {
    let nascidos: Vec<(Processo, Option<u64>)> =
        processos.iter().map(|p| (*p, criado_em(p.pid))).collect();

    let e_pai_de_verdade = |pai: u32, filho_nasceu: Option<u64>| {
        nascidos.iter().any(|(q, q_nasceu)| {
            q.pid == pai
                && match (q_nasceu, filho_nasceu) {
                    (Some(pai_nasceu), Some(filho_nasceu)) => *pai_nasceu <= filho_nasceu,
                    _ => true,
                }
        })
    };

    let mais_antigo = |(p, nasceu): &&(Processo, Option<u64>)| (nasceu.is_none(), nasceu.unwrap_or(0), p.pid);

    let raiz = nascidos
        .iter()
        .filter(|(p, nasceu)| !e_pai_de_verdade(p.pai, *nasceu))
        .min_by_key(mais_antigo)
        .or_else(|| nascidos.iter().min_by_key(mais_antigo))?;

    Some(Identidade {
        pid: raiz.0.pid,
        criado_em: raiz.1.unwrap_or(0),
    })
}

fn encerrar() {
    crate::processos::encerrar_por_nome(IMAGEM);
    std::thread::sleep(Duration::from_millis(500));
}

/// Fecha e reabre o Discord.
pub fn reiniciar() -> Result<bool> {
    let Some(lancador) = lancador() else {
        return Ok(false);
    };
    let estava_aberto = esta_rodando();
    if estava_aberto {
        encerrar();
    }

    Command::new(lancador)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;

    Ok(true)
}

/// Só encerra, sem reabrir. Usado na desinstalação.
pub fn encerrar_se_aberto() -> bool {
    if esta_rodando() {
        encerrar();
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pid: u32, pai: u32) -> Processo {
        Processo { pid, pai }
    }

    fn arvore_comum() -> Vec<Processo> {
        vec![
            p(4000, 3990),
            p(4100, 4000),
            p(4200, 4000),
            p(4300, 4000),
            p(4400, 4000),
            p(4500, 4000),
            p(4600, 4000),
        ]
    }

    #[test]
    fn o_principal_e_quem_nao_tem_pai_discord() {
        let principal = principal_entre(&arvore_comum(), |pid| Some(u64::from(pid) * 10));
        assert_eq!(
            principal,
            Some(Identidade {
                pid: 4000,
                criado_em: 40_000
            })
        );
    }

    #[test]
    fn filho_novo_nao_muda_o_principal() {
        let mut arvore = arvore_comum();
        let antes = principal_entre(&arvore, |_| Some(1));

        arvore.retain(|p| p.pid != 4300);
        arvore.push(p(4700, 4000));
        assert_eq!(principal_entre(&arvore, |_| Some(1)), antes);
    }

    #[test]
    fn sem_discord_nao_ha_principal() {
        assert_eq!(principal_entre(&[], |_| Some(1)), None);
    }

    #[test]
    fn sem_hora_de_criacao_o_pid_ainda_identifica() {
        let principal = principal_entre(&arvore_comum(), |_| None);
        assert_eq!(principal.map(|i| i.pid), Some(4000));
    }

    #[test]
    fn pid_do_lancador_reaproveitado_por_um_filho_nao_esconde_o_principal() {
        let arvore = vec![p(4000, 3990), p(3990, 4000), p(4200, 4000), p(4300, 4000)];
        let hora = |pid: u32| Some(if pid == 4000 { 100 } else { 105 + u64::from(pid) });
        assert_eq!(
            principal_entre(&arvore, hora),
            Some(Identidade {
                pid: 4000,
                criado_em: 100
            })
        );

        let sem_hora = principal_entre(&arvore, |_| None);
        assert!(sem_hora.is_some(), "Discord de pé nunca vira None");
        assert_eq!(principal_entre(&arvore, |_| None), sem_hora);
    }

    #[test]
    fn raiz_sem_hora_nao_passa_na_frente_da_raiz_com_hora() {
        let arvore = vec![p(4000, 1), p(9000, 2)];
        let hora = |pid: u32| (pid == 4000).then_some(500);
        assert_eq!(principal_entre(&arvore, hora).map(|i| i.pid), Some(4000));
    }

    #[test]
    fn com_o_principal_morto_os_filhos_ainda_dao_uma_identidade_estavel() {
        let orfaos = vec![p(4100, 4000), p(4200, 4000), p(4300, 4000)];
        let hora = |pid: u32| Some(u64::from(pid));
        assert_eq!(principal_entre(&orfaos, hora).map(|i| i.pid), Some(4100));
        assert_eq!(principal_entre(&orfaos, hora).map(|i| i.pid), Some(4100));
    }
}
