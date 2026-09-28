//! Configurações do sistema para Linux: proxy automático, autostart e PATH.
//!
//! No Linux, o proxy automático é aplicado via lançador do Discord (.desktop e wrapper),
//! com suporte integrado às configurações de proxy de desktop do KDE Plasma (kwriteconfig)
//! e GNOME (gsettings). O autostart é gerenciado pelo systemd --user e XDG Autostart.

use anyhow::{bail, Context, Result};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
};

fn pasta_config() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .or_else(|_| std::env::var("HOME").map(|h| format!("{h}/.config")))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(base)
}

fn pasta_dados_local() -> PathBuf {
    let base = std::env::var("XDG_DATA_HOME")
        .or_else(|_| std::env::var("HOME").map(|h| format!("{h}/.local/share")))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(base)
}

fn pasta_bin_local() -> PathBuf {
    let base = std::env::var("HOME").map(|h| format!("{h}/.local/bin")).unwrap_or_else(|_| ".".into());
    PathBuf::from(base)
}

fn caminho_servico_systemd() -> PathBuf {
    pasta_config().join("systemd").join("user").join("fol-discord.service")
}

fn caminho_autostart_desktop() -> PathBuf {
    pasta_config().join("autostart").join("fol-discord.desktop")
}

fn caminho_discord_desktop() -> PathBuf {
    pasta_dados_local().join("applications").join("discord.desktop")
}

fn caminho_discord_backup() -> PathBuf {
    pasta_dados_local().join("applications").join("discord.desktop.fol_backup")
}

fn caminho_vesktop_desktop() -> PathBuf {
    pasta_dados_local().join("applications").join("vesktop.desktop")
}

fn caminho_vesktop_backup() -> PathBuf {
    pasta_dados_local().join("applications").join("vesktop.desktop.fol_backup")
}

fn caminho_wrapper_discord() -> PathBuf {
    pasta_bin_local().join("discord")
}

fn caminho_wrapper_vesktop() -> PathBuf {
    pasta_bin_local().join("vesktop")
}

fn caminho_pac_marcador() -> PathBuf {
    crate::pasta_dados().join("pac_url")
}

// --- Proxy Automático (PAC) ------------------------------------------------

pub fn ativar_pac(url: &str) -> Result<()> {
    let _ = fs::create_dir_all(crate::pasta_dados());
    let _ = fs::write(caminho_pac_marcador(), url);

    // 1. Configurar Discord oficial
    let _ = configurar_desktop("discord", "/usr/share/applications/discord.desktop", &caminho_discord_desktop(), &caminho_discord_backup(), "/usr/bin/discord", url);
    let _ = configurar_wrapper(&caminho_wrapper_discord(), "/usr/bin/discord", url);

    // 2. Configurar Vesktop
    let _ = configurar_desktop("vesktop", "/usr/share/applications/vesktop.desktop", &caminho_vesktop_desktop(), &caminho_vesktop_backup(), "/usr/bin/vesktop", url);
    let _ = configurar_wrapper(&caminho_wrapper_vesktop(), "/usr/bin/vesktop", url);

    // 3. Se estiver no KDE Plasma (kwriteconfig6 ou kwriteconfig5)
    configurar_kde_proxy(url);

    // 4. Se gsettings estiver disponível (GNOME / ambientes compatíveis)
    configurar_gnome_proxy(url);

    Ok(())
}

pub fn desativar_pac() -> Result<()> {
    let _ = fs::remove_file(caminho_pac_marcador());

    // 1. Restaurar ou remover discord e vesktop
    restaurar_desktop(&caminho_discord_desktop(), &caminho_discord_backup());
    remover_wrapper(&caminho_wrapper_discord());

    restaurar_desktop(&caminho_vesktop_desktop(), &caminho_vesktop_backup());
    remover_wrapper(&caminho_wrapper_vesktop());

    // 2. Desativar KDE proxy
    desativar_kde_proxy();

    // 3. Desativar GNOME proxy
    desativar_gnome_proxy();

    Ok(())
}

pub fn pac_ativo(url: &str) -> bool {
    if let Ok(conteudo) = fs::read_to_string(caminho_pac_marcador()) {
        if conteudo.trim() == url.trim() {
            return true;
        }
    }

    for desk in [&caminho_discord_desktop(), &caminho_vesktop_desktop()] {
        if let Ok(conteudo) = fs::read_to_string(desk) {
            if conteudo.contains(&format!("--proxy-pac-url={url}")) {
                return true;
            }
        }
    }

    false
}

fn configurar_desktop(
    nome_app: &str,
    caminho_origem: &str,
    destino: &Path,
    backup: &Path,
    binario_padrao: &str,
    url: &str,
) -> Result<()> {
    let pasta_apps = destino.parent().context("caminho de applications inválido")?;
    fs::create_dir_all(pasta_apps)?;

    let origem_sistema = Path::new(caminho_origem);
    let conteudo_base = if destino.exists() {
        let atual = fs::read_to_string(destino)?;
        if !atual.contains("# Managed by FOL-discord") && !backup.exists() {
            let _ = fs::copy(destino, backup);
        }
        atual
    } else if origem_sistema.exists() {
        fs::read_to_string(origem_sistema)?
    } else {
        format!(
            "[Desktop Entry]\n\
             Name={nome_app}\n\
             StartupWMClass={nome_app}\n\
             Comment=Cliente Discord\n\
             GenericName=Internet Messenger\n\
             Exec={binario_padrao} %U\n\
             Icon={nome_app}\n\
             Type=Application\n\
             Categories=Network;InstantMessaging;\n"
        )
    };

    let flag = format!("--enable-features=WebRTCPipeWireCapturer --proxy-pac-url={url}");
    let mut linhas_modificadas = Vec::new();
    let mut tem_marcador = false;

    for linha in conteudo_base.lines() {
        if linha.trim() == "# Managed by FOL-discord" {
            tem_marcador = true;
            linhas_modificadas.push(linha.to_string());
            continue;
        }

        if linha.starts_with("Exec=") {
            let mut partes: Vec<&str> = linha.split_whitespace().collect();
            partes.retain(|p| !p.starts_with("--proxy-pac-url=") && !p.starts_with("--enable-features="));
            if partes.is_empty() {
                linhas_modificadas.push(format!("Exec={binario_padrao} {flag}"));
            } else {
                let exec = partes[0];
                let resto = &partes[1..];
                if resto.is_empty() {
                    linhas_modificadas.push(format!("{exec} {flag}"));
                } else {
                    linhas_modificadas.push(format!("{exec} {flag} {}", resto.join(" ")));
                }
            }
        } else {
            linhas_modificadas.push(linha.to_string());
        }
    }

    if !tem_marcador {
        linhas_modificadas.insert(0, "# Managed by FOL-discord".to_string());
    }

    let mut novo_conteudo = linhas_modificadas.join("\n");
    novo_conteudo.push('\n');
    fs::write(destino, novo_conteudo)?;

    let _ = Command::new("update-desktop-database").arg(pasta_apps).output();
    Ok(())
}

fn restaurar_desktop(destino: &Path, backup: &Path) {
    if backup.exists() {
        let _ = fs::copy(backup, destino);
        let _ = fs::remove_file(backup);
    } else if destino.exists() {
        if let Ok(conteudo) = fs::read_to_string(destino) {
            if conteudo.contains("# Managed by FOL-discord") {
                let _ = fs::remove_file(destino);
            }
        }
    }

    if let Some(pasta_apps) = destino.parent() {
        let _ = Command::new("update-desktop-database").arg(pasta_apps).output();
    }
}

fn configurar_wrapper(destino: &Path, real_bin: &str, url: &str) -> Result<()> {
    if let Some(p) = destino.parent() {
        fs::create_dir_all(p)?;
    }

    let conteudo = format!(
        "#!/bin/sh\n\
         # Managed by FOL-discord\n\
         exec \"{real_bin}\" --enable-features=WebRTCPipeWireCapturer --proxy-pac-url=\"{url}\" \"$@\"\n"
    );

    fs::write(destino, conteudo)?;
    let mut perms = fs::metadata(destino)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(destino, perms)?;

    Ok(())
}

fn remover_wrapper(destino: &Path) {
    if destino.exists() {
        if let Ok(conteudo) = fs::read_to_string(destino) {
            if conteudo.contains("# Managed by FOL-discord") {
                let _ = fs::remove_file(destino);
            }
        }
    }
}

fn configurar_kde_proxy(url: &str) {
    let kwriteconfig = if Command::new("which").arg("kwriteconfig6").output().map(|o| o.status.success()).unwrap_or(false) {
        "kwriteconfig6"
    } else if Command::new("which").arg("kwriteconfig5").output().map(|o| o.status.success()).unwrap_or(false) {
        "kwriteconfig5"
    } else {
        return;
    };

    let _ = Command::new(kwriteconfig)
        .args(["--file", "kioslaverc", "--group", "Proxy Settings", "--key", "ProxyType", "2"])
        .output();
    let _ = Command::new(kwriteconfig)
        .args(["--file", "kioslaverc", "--group", "Proxy Settings", "--key", "Proxy Config Script", url])
        .output();
}

fn desativar_kde_proxy() {
    let kwriteconfig = if Command::new("which").arg("kwriteconfig6").output().map(|o| o.status.success()).unwrap_or(false) {
        "kwriteconfig6"
    } else if Command::new("which").arg("kwriteconfig5").output().map(|o| o.status.success()).unwrap_or(false) {
        "kwriteconfig5"
    } else {
        return;
    };

    let _ = Command::new(kwriteconfig)
        .args(["--file", "kioslaverc", "--group", "Proxy Settings", "--key", "ProxyType", "0"])
        .output();
}

fn configurar_gnome_proxy(url: &str) {
    if Command::new("which").arg("gsettings").output().map(|o| o.status.success()).unwrap_or(false) {
        let _ = Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy", "mode", "auto"])
            .output();
        let _ = Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy", "autoconfig-url", url])
            .output();
    }
}

fn desativar_gnome_proxy() {
    if Command::new("which").arg("gsettings").output().map(|o| o.status.success()).unwrap_or(false) {
        let _ = Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy", "mode", "none"])
            .output();
    }
}

// --- Autostart -------------------------------------------------------------

pub fn ativar_autostart(comando: &str) -> Result<()> {
    // 1. Serviço de usuário do systemd
    let servico_path = caminho_servico_systemd();
    if let Some(p) = servico_path.parent() {
        fs::create_dir_all(p)?;
    }

    let conteudo_servico = format!(
        "[Unit]\n\
         Description=FOL-discord - Correção de handshake do Discord\n\
         After=network.target\n\n\
         [Service]\n\
         Type=simple\n\
         ExecStart={comando}\n\
         Restart=on-failure\n\
         RestartSec=5\n\n\
         [Install]\n\
         WantedBy=default.target\n"
    );
    fs::write(&servico_path, conteudo_servico)?;

    let _ = Command::new("systemctl").args(["--user", "daemon-reload"]).output();
    let _ = Command::new("systemctl").args(["--user", "enable", "fol-discord.service"]).output();

    // 2. XDG Autostart Desktop Entry (garante inicialização em ambientes sem systemd)
    let autostart_path = caminho_autostart_desktop();
    if let Some(p) = autostart_path.parent() {
        fs::create_dir_all(p)?;
    }

    let conteudo_autostart = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=FOL-discord\n\
         Comment=Correção do Discord\n\
         Exec={comando}\n\
         Terminal=false\n\
         Hidden=false\n\
         X-GNOME-Autostart-enabled=true\n"
    );
    fs::write(&autostart_path, conteudo_autostart)?;

    Ok(())
}

pub fn validar_autostart_do_fol(servico: &Path) -> Result<()> {
    let servico_path = caminho_servico_systemd();
    if servico_path.exists() {
        let conteudo = fs::read_to_string(&servico_path)?;
        let esperado = format!("{}", servico.display());
        if !conteudo.contains(&esperado) {
            bail!("o serviço systemd fol-discord.service não pertence ao executável instalado");
        }
    }
    Ok(())
}

pub fn desativar_autostart(servico: &Path) -> Result<()> {
    validar_autostart_do_fol(servico)?;

    let _ = Command::new("systemctl").args(["--user", "stop", "fol-discord.service"]).output();
    let _ = Command::new("systemctl").args(["--user", "disable", "fol-discord.service"]).output();

    let servico_path = caminho_servico_systemd();
    if servico_path.exists() {
        let _ = fs::remove_file(servico_path);
    }
    let _ = Command::new("systemctl").args(["--user", "daemon-reload"]).output();

    let autostart_path = caminho_autostart_desktop();
    if autostart_path.exists() {
        let _ = fs::remove_file(autostart_path);
    }

    Ok(())
}

pub fn autostart_ativo() -> bool {
    caminho_servico_systemd().exists() || caminho_autostart_desktop().exists()
}

// --- PATH ------------------------------------------------------------------

pub fn adicionar_ao_path(dir: &str) -> Result<()> {
    let bin_local = pasta_bin_local();
    fs::create_dir_all(&bin_local)?;

    let origem = Path::new(dir).join("fol-discord");
    let link = bin_local.join("fol-discord");

    if link.exists() || link.is_symlink() {
        let _ = fs::remove_file(&link);
    }

    #[cfg(unix)]
    std::os::unix::fs::symlink(&origem, &link).context("criando symlink em ~/.local/bin/fol-discord")?;

    Ok(())
}

pub fn remover_do_path(_dir: &str) -> Result<()> {
    let link = pasta_bin_local().join("fol-discord");
    if link.exists() || link.is_symlink() {
        let _ = fs::remove_file(link);
    }
    Ok(())
}

pub fn path_ativo(dir: &str) -> bool {
    let link = pasta_bin_local().join("fol-discord");
    if link.exists() || link.is_symlink() {
        return true;
    }

    if let Ok(path) = std::env::var("PATH") {
        for p in std::env::split_paths(&path) {
            if p == Path::new(dir) {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifica_linha_exec_adicionando_proxy_pac() {
        let original = "[Desktop Entry]\nName=Discord\nExec=/usr/bin/discord --url -- %u\nIcon=discord\n";
        let url = "http://127.0.0.1:9251/proxy.pac";
        let flag = format!("--proxy-pac-url={url}");

        let mut linhas = Vec::new();
        for linha in original.lines() {
            if linha.starts_with("Exec=") {
                let partes: Vec<&str> = linha.split_whitespace().collect();
                let exec = partes[0];
                let resto = &partes[1..];
                linhas.push(format!("{exec} {flag} {}", resto.join(" ")));
            } else {
                linhas.push(linha.to_string());
            }
        }
        let resultado = linhas.join("\n");
        assert!(resultado.contains("Exec=/usr/bin/discord --proxy-pac-url=http://127.0.0.1:9251/proxy.pac --url -- %u"));
    }

    #[test]
    fn remove_flag_antiga_ao_atualizar_url() {
        let atual = "[Desktop Entry]\nExec=/usr/bin/discord --proxy-pac-url=http://127.0.0.1:8888/proxy.pac --url -- %u\n";
        let novo_url = "http://127.0.0.1:9251/proxy.pac";
        let flag = format!("--proxy-pac-url={novo_url}");

        let mut linhas = Vec::new();
        for linha in atual.lines() {
            if linha.starts_with("Exec=") {
                let mut partes: Vec<&str> = linha.split_whitespace().collect();
                partes.retain(|p| !p.starts_with("--proxy-pac-url="));
                let exec = partes[0];
                let resto = &partes[1..];
                linhas.push(format!("{exec} {flag} {}", resto.join(" ")));
            } else {
                linhas.push(linha.to_string());
            }
        }
        let resultado = linhas.join("\n");
        assert!(!resultado.contains(":8888"));
        assert!(resultado.contains("--proxy-pac-url=http://127.0.0.1:9251/proxy.pac"));
    }
}
