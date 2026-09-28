# FOL-discord (Linux Edition)

**Devolve a transmissão de tela e a câmera do Discord no Brasil.**  
Sem VPN, sem conta, sem mensalidade, sem root/sudo, sem perder ping.

---

## O que ele faz

O Discord decide a região da sua sessão pelo IP que enxerga **no momento em que abre**, e essa decisão vale para a sessão inteira. Quando ela sai com rota degradada em provedores brasileiros, a transmissão de tela e a câmera falham ou travam.

A solução manual comum era abrir o Discord com VPN e desligar depois. Este programa faz exatamente o mesmo, sozinho e sem VPN:

1. O Discord abre e o tráfego que decide região sai por um IP estrangeiro temporariamente.
2. A sessão nasce com esse IP, e a rota saudável fica gravada nela.
3. Assim que a abertura termina (30s de silêncio), o programa derruba o túnel com o exterior.
4. O Discord reconecta sozinho, direto pelo caminho curto brasileiro (São Paulo / GRU).
5. Áudio, vídeo e transmissão de tela viajam em **UDP** e nunca passam pelo proxy.

---

## Como Instalar no Linux

### 1. Clonar e Instalar
```bash
git clone https://github.com/Adrian-ESoftware/dcproxyv2.git
cd dcproxyv2
./install.sh
```

O script:
* Compila o binário otimizado (se ainda não compilado);
* Instala no diretório do usuário em `~/.local/share/fol-discord/`;
* Adiciona o comando `fol-discord` em `~/.local/bin/`;
* Configura inicialização automática via `systemd --user` e XDG Autostart;
* Configura o proxy automático PAC para o Discord (.desktop, wrapper, KDE Plasma e GNOME);
* **Não requer root/sudo**.

---

## Comandos

No terminal:

```bash
fol-discord status              # Mostra o estado atual
fol-discord reiniciar-discord   # Fecha e reabre o Discord
fol-discord rodar               # Executa em primeiro plano (para depurar/ver logs)
fol-discord desinstalar         # Remove tudo e restaura as configurações
```

---

## Licença

MIT License.
