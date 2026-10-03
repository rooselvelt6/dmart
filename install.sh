#!/bin/bash
# dMart UCI - Instalador rápido
# Uso: curl -fsSL https://raw.githubusercontent.com/rooselvelt6/dmart/main/install.sh | bash

set -euo pipefail

REPO="rooselvelt6/dmart"
BRANCH="main"
INSTALL_DIR="${HOME}/.local/share/dmart"
BIN_DIR="${HOME}/.local/bin"

# Colores
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log() { echo -e "${BLUE}[INFO]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*" >&2; exit 1; }
success() { echo -e "${GREEN}[OK]${NC} $*"; }

# Detectar OS
detect_os() {
    case "$(uname -s)" in
        Linux*)     OS="linux";;
        Darwin*)    OS="macos";;
        *)          error "OS no soportado: $(uname -s)";;
    esac
    case "$(uname -m)" in
        x86_64|amd64) ARCH="x86_64";;
        aarch64|arm64) ARCH="aarch64";;
        *) error "Arquitectura no soportada: $(uname -m)";;
    esac
    log "Detectado: ${OS}/${ARCH}"
}

# Verificar dependencias
check_deps() {
    local missing=()
    for cmd in curl git cargo rustc; do
        command -v "$cmd" >/dev/null 2>&1 || missing+=("$cmd")
    done
    
    if [ ${#missing[@]} -gt 0 ]; then
        warn "Faltan dependencias: ${missing[*]}"
        log "Instalando dependencias..."
        install_deps "${missing[@]}"
    fi
}

install_deps() {
    case "$OS" in
        linux)
            if command -v apt >/dev/null; then
                sudo apt update && sudo apt install -y "$@"
            elif command -v dnf >/dev/null; then
                sudo dnf install -y "$@"
            elif command -v pacman >/dev/null; then
                sudo pacman -S --noconfirm "$@"
            elif command -v zypper >/dev/null; then
                sudo zypper install -y "$@"
            else
                error "Gestor de paquetes no soportado. Instala manualmente: $*"
            fi
            ;;
        macos)
            if command -v brew >/dev/null; then
                brew install "$@"
            else
                error "Homebrew requerido en macOS. Instala desde https://brew.sh"
            fi
            ;;
    esac
}

# Instalar Rust si no existe
ensure_rust() {
    if ! command -v cargo >/dev/null 2>&1; then
        log "Instalando Rust..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "${HOME}/.cargo/env"
    fi
}

# Instalar Trunk (WASM bundler)
ensure_trunk() {
    if ! command -v trunk >/dev/null 2>&1; then
        log "Instalando Trunk (WASM bundler)..."
        cargo install trunk --locked
    fi
}

# Clonar y compilar
build_project() {
    log "Clonando repositorio..."
    rm -rf "${INSTALL_DIR}"
    git clone --depth 1 --branch "${BRANCH}" "https://github.com/${REPO}.git" "${INSTALL_DIR}"
    
    cd "${INSTALL_DIR}"
    
    log "Compilando backend (dmart-server)..."
    cargo build --release -p dmart-server
    
    log "Compilando frontend (WASM)..."
    cd dmart-app
    trunk build --release
    
    log "Compilación completada"
}

# Instalar binarios
install_binaries() {
    log "Instalando binarios en ${BIN_DIR}..."
    mkdir -p "${BIN_DIR}"
    
    # Backend
    cp "${INSTALL_DIR}/target/release/dmart-server" "${BIN_DIR}/dmart-server"
    
    # Frontend (archivos estáticos)
    mkdir -p "${INSTALL_DIR}/dist"
    cp -r dmart-app/dist/* "${INSTALL_DIR}/dist/"
    
    # Script de inicio
    cat > "${BIN_DIR}/dmart" << 'EOF'
#!/bin/bash
# dMart UCI - Launcher
DMART_DIR="${HOME}/.local/share/dmart"
export DMART_DIST_PATH="${DMART_DIR}/dist"
export DMART_DB_PATH="${DMART_DIR}/data/dmart.db"
export DMART_MASTER_KEY="${DMART_MASTER_KEY:-$(openssl rand -hex 32)}"
export JWT_SECRET="${JWT_SECRET:-$(openssl rand -hex 32)}"
export DMART_ADMIN_PASSWORD="${DMART_ADMIN_PASSWORD:-}"
mkdir -p "${DMART_DIR}/data"
exec "${HOME}/.local/bin/dmart-server"
EOF
    chmod +x "${BIN_DIR}/dmart"
    
    success "Binarios instalados en ${BIN_DIR}"
}

# Configurar PATH
setup_path() {
    local shell_rc=""
    case "$SHELL" in
        */zsh) shell_rc="${HOME}/.zshrc";;
        */bash) shell_rc="${HOME}/.bashrc";;
        */fish) shell_rc="${HOME}/.config/fish/config.fish";;
    esac
    
    if [ -n "$shell_rc" ] && [ -f "$shell_rc" ]; then
        if ! grep -q '\.local/bin' "$shell_rc"; then
            echo 'export PATH="${HOME}/.local/bin:${PATH}"' >> "$shell_rc"
            log "PATH actualizado en $shell_rc"
        fi
    fi
}

# Verificar instalación
verify_install() {
    log "Verificando instalación..."
    
    if [ ! -f "${BIN_DIR}/dmart-server" ]; then
        error "dmart-server no encontrado"
    fi
    
    if [ ! -f "${BIN_DIR}/dmart" ]; then
        error "dmart launcher no encontrado"
    fi
    
    # Test rápido
    mkdir -p /tmp/dmart-test
    DMART_DB_PATH=/tmp/dmart-test/test.db \
    DMART_DIST_PATH="${INSTALL_DIR}/dist" \
    DMART_MASTER_KEY="testkey123456789012345678901234" \
    JWT_SECRET="testjwtsecret123456789012345678901234" \
    timeout 5 "${BIN_DIR}/dmart-server" >/dev/null 2>&1 || true
    
    success "Instalación verificada"
}

# Main
main() {
    echo -e "${BLUE}"
    echo "╔══════════════════════════════════════╗"
    echo "║      dMart UCI - Instalador          ║"
    echo "║   Sistema UCI - Cuidados Intensivos  ║"
    echo "╚══════════════════════════════════════╝"
    echo -e "${NC}"
    
    detect_os
    check_deps
    ensure_rust
    ensure_trunk
    build_project
    install_binaries
    setup_path
    verify_install
    
    echo
    success "¡Instalación completada!"
    echo
    log "Para iniciar dMart:"
    echo "  dmart"
    echo
    log "Luego abre: http://localhost:3000/login"
    echo "  Usuario: admin"
    echo "  Contraseña: (se genera aleatoria en logs, o configura DMART_ADMIN_PASSWORD)"
    echo
    warn "Reinicia tu terminal o ejecuta: source ~/.bashrc (o .zshrc)"
}

main "$@"