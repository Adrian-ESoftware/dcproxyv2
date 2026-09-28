#!/bin/sh
# fol-discord - instalador automático para Linux (Binário pré-compilado)
#
# Não precisa de Rust, Cargo ou compilador.
# Não precisa de permissão de root ou sudo.

set -e

DIR_ORIGEM="$(cd "$(dirname "$0")" 2>/dev/null && pwd || echo ".")"
BINARIO=""

# 1. Procura binário pré-compilado local
if [ -f "$DIR_ORIGEM/bin/fol-discord" ]; then
    BINARIO="$DIR_ORIGEM/bin/fol-discord"
elif [ -f "$DIR_ORIGEM/fol-discord" ]; then
    BINARIO="$DIR_ORIGEM/fol-discord"
elif [ -f "$DIR_ORIGEM/target/release/fol-discord" ]; then
    BINARIO="$DIR_ORIGEM/target/release/fol-discord"
fi

echo ""
echo "=== FOL-discord — Instalador para Linux ==="
echo ""

# 2. Se não encontrar binário localmente, baixa diretamente do GitHub
if [ -z "$BINARIO" ] || [ ! -f "$BINARIO" ]; then
    echo "[1/2] Baixando binário pré-compilado do GitHub..."
    DESTINO_TEMP="/tmp/fol-discord"
    URL_BINARIO="https://raw.githubusercontent.com/Adrian-ESoftware/dcproxyv2/main/bin/fol-discord"

    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$URL_BINARIO" -o "$DESTINO_TEMP"
    elif command -v wget >/dev/null 2>&1; then
        wget -q "$URL_BINARIO" -O "$DESTINO_TEMP"
    else
        echo "Erro: curl ou wget não encontrados para baixar o binário." >&2
        exit 1
    fi

    chmod +x "$DESTINO_TEMP"
    BINARIO="$DESTINO_TEMP"
else
    echo "[1/2] Binário pronto encontrado: $BINARIO"
    chmod +x "$BINARIO" 2>/dev/null || true
fi

echo "[2/2] Instalando e configurando o serviço..."
"$BINARIO" instalar "$@"

echo ""
echo "✓ Instalação concluída com sucesso!"
echo "Você pode conferir o status com o comando:"
echo "  fol-discord status"
echo ""
