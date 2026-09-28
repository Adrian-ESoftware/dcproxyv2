//! Encontra e encerra processos no Linux via /proc e sinais POSIX.

use std::{fs, path::Path, time::Duration};

/// Um processo visto na árvore do Linux: quem ele é e quem o criou.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Processo {
    pub pid: u32,
    pub pai: u32,
}

/// Todos os processos cujo nome casa com `nome`, sem diferenciar maiúsculas/minúsculas.
pub fn processos_por_nome(nome: &str) -> Vec<Processo> {
    let mut achados = Vec::new();
    let Ok(entradas) = fs::read_dir("/proc") else {
        return achados;
    };

    for entrada in entradas.flatten() {
        let Ok(nome_arquivo) = entrada.file_name().into_string() else {
            continue;
        };
        let Ok(pid) = nome_arquivo.parse::<u32>() else {
            continue;
        };

        let caminho_stat = format!("/proc/{pid}/stat");
        let Ok(stat) = fs::read_to_string(&caminho_stat) else {
            continue;
        };

        let Some(idx_abertura) = stat.find('(') else {
            continue;
        };
        let Some(idx_fechamento) = stat.rfind(')') else {
            continue;
        };
        if idx_abertura >= idx_fechamento {
            continue;
        }

        let comm = &stat[idx_abertura + 1..idx_fechamento];
        let nome_limpo = nome.trim_end_matches(".exe");
        if !comm.eq_ignore_ascii_case(nome) && !comm.eq_ignore_ascii_case(nome_limpo) {
            continue;
        }

        let resto = stat[idx_fechamento + 1..].trim();
        let mut partes = resto.split_whitespace();
        // partes[0] é o estado (ex: 'S', 'R')
        // partes[1] é o ppid
        let _estado = partes.next();
        if let Some(ppid_str) = partes.next() {
            if let Ok(pai) = ppid_str.parse::<u32>() {
                achados.push(Processo { pid, pai });
            }
        }
    }

    achados
}

/// Hora em que o processo nasceu, medido em jiffies desde o boot do sistema (campo starttime).
/// Imutável e previne colisão caso um PID seja reaproveitado.
pub fn criado_em(pid: u32) -> Option<u64> {
    let caminho_stat = format!("/proc/{pid}/stat");
    let stat = fs::read_to_string(&caminho_stat).ok()?;
    let idx_fechamento = stat.rfind(')')?;
    let resto = stat[idx_fechamento + 1..].trim();
    let mut partes = resto.split_whitespace();
    // Campo 19 após `)` corresponde ao starttime (campo 22 em /proc/pid/stat)
    partes.nth(19)?.parse::<u64>().ok()
}

/// Encerra cada PID via SIGTERM, aguarda até 5s e aplica SIGKILL caso ainda reste algum processo.
pub fn encerrar_todos(pids: &[u32]) {
    for pid in pids {
        unsafe {
            libc::kill(*pid as i32, libc::SIGTERM);
        }
    }

    let limite = Duration::from_millis(5000);
    let passo = Duration::from_millis(100);
    let mut decorrido = Duration::ZERO;

    while decorrido < limite {
        let ainda_vivos = pids
            .iter()
            .any(|pid| Path::new(&format!("/proc/{pid}")).exists());
        if !ainda_vivos {
            return;
        }
        std::thread::sleep(passo);
        decorrido += passo;
    }

    for pid in pids {
        if Path::new(&format!("/proc/{pid}")).exists() {
            unsafe {
                libc::kill(*pid as i32, libc::SIGKILL);
            }
        }
    }
}

/// Só os PIDs, para quem não se importa com a árvore.
pub fn pids_por_nome(nome: &str) -> Vec<u32> {
    processos_por_nome(nome).into_iter().map(|p| p.pid).collect()
}

/// Atalho para o caso mais comum: encerrar tudo que atende por um nome.
pub fn encerrar_por_nome(nome: &str) {
    encerrar_todos(&pids_por_nome(nome));
}

pub fn esta_rodando(nome: &str) -> bool {
    !pids_por_nome(nome).is_empty()
}
