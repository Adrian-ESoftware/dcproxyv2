#!/bin/sh
# fol-discord - instalador para Linux
#
# Instala o fol-discord no perfil do usuário (~/.local/share/fol-discord),
# cria o link em ~/.local/bin/fol-discord, configura o autostart (systemd/XDG)
# e o proxy automático para o Discord. Não precisa de permissão de root/sudo.

set -e

DIR_ORIGEM="$(cd "$(dirname "$0")" && pwd)"
BINARIO_RELEASE=""

if [ -f "$DIR_ORIGEM/fol-discord" ]; then
    BINARIO_RELEASE="$DIR_ORIGEM/fol-discord"
elif [ -f "$DIR_ORIGEM/target/release/fol-discord" ]; then
    BINARIO_RELEASE="$DIR_ORIGEM/target/release/fol-discord"
fi

echo ""
echo "=== FOL-discord — Instalador para Linux ==="
echo ""

if [ -z "$BINARIO_RELEASE" ] || [ ! -f "$BINARIO_RELEASE" ]; then
    echo "[1/3] Compilando o fol-discord..."
    if command -v cargo >/dev/null 2>&1; then
        cargo build --release
    elif [ -f "$HOME/.cargo/bin/cargo" ]; then
        "$HOME/.cargo/bin/cargo" build --release
    else
        echo "Erro: cargo não encontrado para compilar." >&2
        exit 1
    fi
else
    echo "[1/3] Binário encontrado: $BINARIO_RELEASE"
fi

echo "[2/3] Executando fol-discord instalar..."
"$BINARIO_RELEASE" instalar "$@"

echo "[3/3] Concluído com sucesso!"
echo ""
echo "Você pode verificar o status a qualquer momento com:"
echo "  fol-discord status"
echo ""
