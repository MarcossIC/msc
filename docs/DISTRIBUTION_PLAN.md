# Plan de Distribución - MSC CLI

**Versión:** 1.0
**Fecha:** 2025-12-26
**Autor:** Investigación automatizada basada en mejores prácticas 2025

---

## Tabla de Contenido

1. [Resumen Ejecutivo](#resumen-ejecutivo)
2. [Análisis del Proyecto Actual](#análisis-del-proyecto-actual)
3. [Opciones de Distribución Investigadas](#opciones-de-distribución-investigadas)
4. [Estrategia Recomendada](#estrategia-recomendada)
5. [Plan de Implementación por Fases](#plan-de-implementación-por-fases)
6. [Cronograma y Dependencias](#cronograma-y-dependencias)
7. [Referencias y Recursos](#referencias-y-recursos)

---

## Resumen Ejecutivo

MSC CLI es una herramienta de línea de comandos multi-propósito escrita en Rust que combina capacidades de administración del sistema, gestión de medios, descarga web y monitoreo en tiempo real. El proyecto está listo para distribución profesional y requiere:

- **Distribución automatizada** multiplataforma (Windows, Linux, macOS)
- **Instaladores nativos** que gestionen el PATH automáticamente
- **Sistema de auto-actualización** para actualizaciones sin fricciones
- **Integración con gestores de paquetes** (winget, Homebrew, etc.)

### Recomendación Principal

Implementar **cargo-dist** como solución principal de distribución por:
- ✅ Automatización completa vía GitHub Actions
- ✅ Soporte multiplataforma nativo
- ✅ Instaladores MSI para Windows con gestión de PATH
- ✅ Scripts de instalación para Unix/macOS
- ✅ Integración directa con GitHub Releases
- ✅ Configuración mínima, resultados profesionales

---

## Análisis del Proyecto Actual

### Características del Proyecto

**Nombre:** MSC CLI
**Versión actual:** 0.1.0
**Licencia:** MIT
**Repositorio:** GitHub (a configurar)
**Lenguaje:** Rust 2021

### Arquitectura

```
MSC CLI
├── Información del Sistema (sys info/monitor)
│   ├── CPU, GPU, RAM, Motherboard
│   ├── Red, Almacenamiento, Batería
│   └── Dashboard TUI en tiempo real
├── Gestión de Medios (vget/vedit)
│   ├── Descarga de videos (1000+ plataformas)
│   ├── Edición con FFmpeg
│   └── Extracción de cookies de navegadores
├── Descarga Web (wget)
│   ├── Mirror de sitios completos
│   ├── Post-procesamiento offline
│   └── Soporte CDP para Chrome
├── Limpieza de Sistema (clean)
│   ├── Archivos temporales
│   ├── Caché de proyectos
│   └── Validación de seguridad
└── Sistema de Alias Global
    ├── Creación de shortcuts
    └── Ejecutables shim (~369KB c/u)
```

### Estado Actual de Compilación

**Perfil de Release configurado:**
```toml
[profile.release]
strip = true           # Eliminar símbolos de depuración
lto = true            # Optimización en tiempo de enlace
codegen-units = 1     # Optimización máxima
panic = "abort"       # Binario más pequeño
```

**Dependencias principales:**
- 40+ crates de producción
- Soporta características opcionales (nvml, rocm)
- Abstracciones multiplataforma (Windows/Unix)
- Interfaz TUI con ratatui
- Cliente HTTP async con tokio/reqwest

### Complejidad y Tamaño

- **Archivos fuente:** 80+ archivos Rust
- **Líneas de código:** ~15,000+ líneas (estimado)
- **Binario release:** ~15-30 MB (estimado, depende de plataforma)
- **Shim ejecutables:** 369 KB c/u

### Casos de Uso

1. **Administradores de sistemas** - Monitoreo y limpieza
2. **Desarrolladores** - Gestión de workspaces y alias
3. **Creadores de contenido** - Descarga y edición de video
4. **Usuarios generales** - Herramienta de productividad

---

## Opciones de Distribución Investigadas

### Opción 1: cargo-dist (⭐ RECOMENDADA)

**Descripción:** Herramienta moderna de empaquetado para aplicaciones Rust mantenida por Axo Dev.

**Versión actual:** 0.30.2 (2025)

#### Ventajas
✅ **Automatización completa**
- Integración con GitHub Actions lista para usar
- Pipeline completo: plan → build → host → publish → announce
- Compilación multiplataforma automática

✅ **Instaladores nativos**
- **Windows:** MSI con WiX v3
- **macOS:** Scripts homebrew-style
- **Linux:** Scripts shell + tarballs

✅ **Gestión de PATH**
- MSI modifica variables de entorno automáticamente
- Scripts Unix agregan binarios a directorios estándar

✅ **Integración GitHub Releases**
- Publicación automática al crear tags (v1.0.0)
- Assets organizados por plataforma
- Checksums y firmas opcionales

✅ **Configuración mínima**
```bash
cargo install cargo-dist
cargo dist init
```

#### Limitaciones
⚠️ WiX v4 no soportado aún (usa WiX v3)
⚠️ Requiere Windows para construir MSI (GitHub Actions lo tiene pre-instalado)

#### Referencias
- [cargo-dist en crates.io](https://crates.io/crates/cargo-dist)
- [Documentación de instaladores MSI](https://opensource.axo.dev/cargo-dist/book/installers/msi.html)
- [Guía oficial cargo-dist](https://github.com/axodotdev/cargo-dist)

---

### Opción 2: cargo-wix

**Descripción:** Subcomando de Cargo para crear instaladores MSI usando WiX Toolset directamente.

#### Ventajas
✅ Control total sobre el instalador MSI
✅ Personalización completa del archivo `main.wxs`
✅ Sin dependencia de servicios externos

#### Limitaciones
⚠️ Solo Windows (requiere WiX Toolset instalado)
⚠️ Configuración manual del XML WiX
⚠️ No incluye automatización CI/CD
⚠️ Requiere gestión manual de PATH:

```xml
<Environment Id="PATH" Name="PATH"
    Value="[INSTALLFOLDER]"
    Permanent="no"
    Part="last"
    Action="set"
    System="yes" />
```

#### Cuándo usar
- Control absoluto sobre cada aspecto del instalador
- Requisitos de empaquetado muy específicos
- Solo distribución Windows

#### Referencias
- [cargo-wix en GitHub](https://github.com/volks73/cargo-wix)

---

### Opción 3: Inno Setup

**Descripción:** Creador de instaladores .exe clásicos para Windows.

#### Ventajas
✅ Instaladores .exe familiares para usuarios finales
✅ Interfaz gráfica amigable durante instalación
✅ Control completo sobre proceso de instalación

#### Limitaciones
⚠️ Solo Windows
⚠️ Requiere compilar binario primero (cargo build --release)
⚠️ Script .iss separado para mantener
⚠️ Proceso manual (no automatizado)

#### Configuración PATH
```ini
[Setup]
ChangesEnvironment=yes

[Registry]
Root: HKLM; Subkey: "SYSTEM\CurrentControlSet\Control\Session Manager\Environment"; \
    ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; \
    Check: NeedsAddPath('{app}')
```

#### Cuándo usar
- Usuarios finales que prefieren instaladores .exe tradicionales
- Necesitas wizard de instalación personalizado
- Complemento a cargo-dist para variedad de opciones

---

### Opción 4: Gestores de Paquetes Nativos

#### Windows Package Manager (winget)

**Proceso de submisión:**
1. Fork de [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs)
2. Crear manifiesto de paquete
3. Validar con `winget validate`
4. Pull request al repositorio oficial
5. Validación automatizada
6. Revisión manual por moderadores
7. Aprobación e inclusión en catálogo

**Requisitos:**
- Instalador debe ser MSIX, MSI, APPX o .exe
- ✅ MSI generado por cargo-dist es compatible

**Ventajas:**
- Descubrimiento por usuarios de Windows
- Instalación/actualización centralizada
- Integración con Windows Terminal

**Tiempo:** Aprobación puede tomar varios días

#### Referencias
- [Documentación de winget](https://learn.microsoft.com/en-us/windows/package-manager/winget/)
- [Repositorio winget-pkgs](https://github.com/microsoft/winget-pkgs)

#### Homebrew (macOS/Linux)

**Proceso:**
1. Crear "formula" (archivo Ruby)
2. Pull request a homebrew-core
3. Revisión de la comunidad

**Alternativa más rápida:**
- Crear tu propio "tap" (repositorio de fórmulas)
- Los usuarios agregan: `brew tap tuusuario/msc`
- Instalación: `brew install msc`

#### Cargo (crates.io)

**Consideraciones:**
- `cargo install msc` - Funciona pero compila desde fuente
- No gestiona PATH automáticamente
- Tiempo de instalación muy largo
- Útil para desarrolladores Rust principalmente

---

## Estrategia Recomendada

### Enfoque de 3 Niveles

#### Nivel 1: Distribución Base (PRIORITARIO)
**Herramienta:** cargo-dist + GitHub Releases

**Cubre:**
- Windows (MSI)
- macOS (Homebrew-style installer)
- Linux (Script shell + tarball)

**Razón:** Configuración única, builds automáticos, instaladores nativos

---

#### Nivel 2: Gestores de Paquetes (MEDIANO PLAZO)
**Integraciones:**
1. **winget** (Windows) - Alcance amplio
2. **Homebrew tap** (macOS/Linux) - Control total
3. **AUR** (Arch Linux) - Comunidad activa

**Razón:** Mayor descubrimiento, instalaciones más fáciles

---

#### Nivel 3: Auto-actualización (COMPLEMENTARIO)
**Herramienta:** self_update crate v0.42+

**Implementa:** `msc update`

**Razón:** Usuarios pueden actualizarse sin reinstalar

---

### Arquitectura de Distribución Propuesta

```
┌─────────────────────────────────────────────────────────────┐
│                     GitHub Repository                        │
│                                                              │
│  ┌────────────────────────────────────────────────────┐    │
│  │  Código fuente + Cargo.toml + .github/workflows/  │    │
│  └────────────────────────────────────────────────────┘    │
│                            │                                │
│                            ▼                                │
│  ┌────────────────────────────────────────────────────┐    │
│  │         GitHub Actions (cargo-dist)                │    │
│  │  - Compila para Windows/Linux/macOS                │    │
│  │  - Genera instaladores MSI                         │    │
│  │  - Crea scripts de instalación                     │    │
│  │  - Publica en GitHub Releases                      │    │
│  └────────────────────────────────────────────────────┘    │
│                            │                                │
└────────────────────────────┼────────────────────────────────┘
                             ▼
         ┌───────────────────────────────────────┐
         │      GitHub Releases (v1.0.0)         │
         │  - msc-v1.0.0-x86_64-pc-windows.msi  │
         │  - msc-v1.0.0-x86_64-apple-darwin.tar │
         │  - msc-v1.0.0-x86_64-linux-gnu.tar   │
         │  - checksums.txt                      │
         └───────────────────────────────────────┘
                             │
        ┌────────────────────┼────────────────────┐
        ▼                    ▼                    ▼
   ┌─────────┐         ┌─────────┐         ┌─────────┐
   │ Windows │         │  macOS  │         │  Linux  │
   │  Users  │         │  Users  │         │  Users  │
   └────┬────┘         └────┬────┘         └────┬────┘
        │                   │                    │
        ▼                   ▼                    ▼
   ┌─────────┐         ┌─────────┐         ┌─────────┐
   │ .msi    │         │ .tar.gz │         │ .tar.gz │
   │Installer│         │ Script  │         │ Script  │
   └────┬────┘         └────┬────┘         └────┬────┘
        │                   │                    │
        ▼                   ▼                    ▼
   Modifica PATH      Agrega a PATH       Agrega a PATH
        │                   │                    │
        └───────────────────┴────────────────────┘
                            │
                            ▼
              ┌─────────────────────────┐
              │   msc --version         │
              │   msc sys monitor       │
              │   msc update            │
              └─────────────────────────┘
```

---

## Plan de Implementación por Fases

### 🔷 FASE 1: Preparación del Proyecto

**Objetivo:** Preparar el repositorio y configuración base para distribución

#### Tareas

##### 1.1. Limpieza y Organización del Repositorio

**Acciones:**
- [ ] Commit de todos los cambios pendientes (23 archivos modificados)
- [ ] Añadir archivos nuevos al repositorio (9 archivos en docs/, src/core/wget/)
- [ ] Revisar y actualizar .gitignore
- [ ] Limpiar archivos temporales (CURRENT_FN.md si no es necesario)

**Comandos:**
```bash
git add .
git commit -m "feat: prepare project for distribution"
git push origin main
```

---

##### 1.2. Configuración de Cargo.toml para Distribución

**Acciones:**
- [ ] Actualizar metadatos del proyecto
- [ ] Configurar información de publicación
- [ ] Verificar versión semántica

**Cambios en Cargo.toml:**
```toml
[package]
name = "msc"
version = "0.1.0"
edition = "2021"
authors = ["Marco <tu-email@ejemplo.com>"]
license = "MIT"
description = "Multi-purpose CLI tool for system monitoring, media management, and productivity"
repository = "https://github.com/tu-usuario/msc"
homepage = "https://github.com/tu-usuario/msc"
documentation = "https://github.com/tu-usuario/msc/blob/main/README.md"
readme = "README.md"
keywords = ["cli", "system-monitor", "video-downloader", "productivity", "tools"]
categories = ["command-line-utilities", "multimedia", "development-tools"]
exclude = [
    "tests/*",
    "docs/*",
    ".github/*",
    "target/*",
]

# El resto de configuración permanece igual...
```

**Verificación:**
```bash
cargo check
cargo build --release
cargo test
```

---

##### 1.3. Crear/Actualizar README.md Profesional

**Secciones requeridas:**
- [ ] Descripción del proyecto
- [ ] Características principales
- [ ] Instalación (placeholder para después)
- [ ] Ejemplos de uso
- [ ] Documentación de comandos
- [ ] Requisitos del sistema
- [ ] Licencia
- [ ] Contribuciones

**Template:**
```markdown
# MSC CLI

Multi-purpose command-line interface tool for system administration, media management, and productivity.

## Features

- 🖥️ **System Monitoring** - Real-time TUI dashboard with CPU, GPU, memory, network metrics
- 📹 **Video Downloading** - Download from 1000+ platforms (YouTube, Vimeo, TikTok, etc.)
- 🌐 **Website Archiving** - Mirror websites for offline viewing
- 🧹 **System Cleanup** - Safe temporary file removal with age-based filtering
- ⚡ **Global Aliases** - Create command shortcuts accessible anywhere
- 📊 **Hardware Information** - Detailed system specifications

## Installation

### Windows
Download the MSI installer from [releases](https://github.com/tu-usuario/msc/releases)

### macOS / Linux
```bash
curl -sSL https://github.com/tu-usuario/msc/releases/latest/download/install.sh | sh
```

## Quick Start

[Continuar con ejemplos...]
```

---

##### 1.4. Verificar Licencia y Documentación Legal

**Acciones:**
- [ ] Confirmar archivo LICENSE (MIT) está presente
- [ ] Añadir copyright notices donde sea apropiado
- [ ] Documentar dependencias de terceros si requieren atribución

---

##### 1.5. Configurar GitHub Repository

**Acciones en GitHub:**
- [ ] Crear repositorio público `msc` (si no existe)
- [ ] Configurar descripción y topics
- [ ] Añadir `.github/ISSUE_TEMPLATE/` para reportes de bugs
- [ ] Añadir `.github/PULL_REQUEST_TEMPLATE.md`
- [ ] Configurar GitHub Pages para documentación (opcional)

**Topics sugeridos:**
`rust` `cli` `system-monitor` `video-downloader` `productivity` `windows` `linux` `macos`

---

### Entregables de Fase 1
✅ Repositorio limpio y organizado
✅ Cargo.toml completamente configurado
✅ README.md profesional
✅ Licencia clarificada
✅ Repositorio GitHub configurado

**Criterio de completitud:** Proyecto puede ser clonado y compilado limpiamente sin errores

---

### 🔷 FASE 2: Implementación de cargo-dist

**Objetivo:** Configurar cargo-dist y automatizar builds multiplataforma

#### Tareas

##### 2.1. Instalación de cargo-dist

**Comando:**
```bash
cargo install cargo-dist
```

**Verificación:**
```bash
cargo dist --version
# Debería mostrar: cargo-dist 0.30.2 (o superior)
```

---

##### 2.2. Inicialización de cargo-dist

**Comando:**
```bash
cargo dist init
```

**Interacciones esperadas:**
El comando hará preguntas interactivas:

1. **¿Generar GitHub Actions?** → **SÍ**
2. **¿Qué instaladores generar?**
   - ✅ MSI (Windows)
   - ✅ Shell script (Unix)
   - ✅ Homebrew (macOS)
3. **¿Targets de compilación?**
   - ✅ x86_64-pc-windows-msvc
   - ✅ x86_64-apple-darwin
   - ✅ aarch64-apple-darwin (Apple Silicon)
   - ✅ x86_64-unknown-linux-gnu
   - ✅ aarch64-unknown-linux-gnu (ARM Linux)

**Cambios generados:**
```
.github/
  └── workflows/
      └── release.yml          # Workflow de GitHub Actions

Cargo.toml                     # Añade [workspace.metadata.dist]
```

---

##### 2.3. Configuración de cargo-dist en Cargo.toml

**Revisar la sección añadida:**
```toml
# El `profile` que dist usará para construir todo
[profile.dist]
inherits = "release"
lto = "thin"

[workspace.metadata.dist]
# Los instaladores a generar para cada app
installers = ["msi", "shell", "homebrew"]

# Targets de compilación
targets = [
    "x86_64-pc-windows-msvc",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu"
]

# El archivo que se debe incluir en cada App (ruta relativa a Cargo.toml)
# Puede personalizar este listado para añadir/excluir archivos
include = ["README.md", "LICENSE"]

# Versión CI para usar en GitHub Actions
ci = ["github"]

# Detecta si está en un workspace
workspace = false
```

**Personalización adicional (opcional):**
```toml
[workspace.metadata.dist]
# ... configuración anterior ...

# Personalizar nombres de instaladores
dist-name = "msc"

# Añadir descripción para instalador
description = "Multi-purpose CLI tool for system monitoring and productivity"

# Personalizar script de instalación
install-path = "CARGO_HOME"  # o "~/bin" o custom

# Añadir firmas (requiere configuración de llaves GPG)
# checksum = "sha256"
```

---

##### 2.4. Configurar GitHub Actions Workflow

**Archivo:** `.github/workflows/release.yml` (generado automáticamente)

**Revisar configuración:**
```yaml
name: Release

on:
  push:
    tags:
      - "v*.*.*"  # Triggers en tags como v1.0.0
  workflow_dispatch:  # Permite ejecución manual

jobs:
  # cargo-dist genera automáticamente los jobs necesarios:
  # - plan: Planea qué construir
  # - build-*: Construye para cada plataforma
  # - host: Sube artifacts a GitHub Releases
  # - publish: Publica anuncios/instaladores
```

**Personalización (opcional):**

Añadir step de testing antes del release:
```yaml
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --all-features

  # ... resto de jobs generados por cargo-dist ...
```

---

##### 2.5. Testing Local de cargo-dist

**Generar preview de lo que se construirá:**
```bash
cargo dist plan
```

**Salida esperada:**
```
📦 Planned artifacts:
   - msc-v0.1.0-x86_64-pc-windows-msvc.msi
   - msc-v0.1.0-x86_64-apple-darwin.tar.gz
   - msc-v0.1.0-aarch64-apple-darwin.tar.gz
   - msc-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
   - msc-v0.1.0-aarch64-unknown-linux-gnu.tar.gz
   - install.sh (universal installer script)
   - checksums.txt
```

**Build local (solo para plataforma actual):**
```bash
cargo dist build
```

**Ubicación de outputs:**
```
target/distrib/
  └── msc-v0.1.0-<tu-plataforma>.*
```

**Validación:**
- [ ] Instalador se genera sin errores
- [ ] Tamaño del binario es razonable (~15-30 MB)
- [ ] Instalador puede ejecutarse localmente

---

##### 2.6. Commit y Push de Configuración

**Comandos:**
```bash
git add .
git commit -m "feat: configure cargo-dist for automated releases"
git push origin main
```

---

### Entregables de Fase 2
✅ cargo-dist instalado y configurado
✅ Workflow de GitHub Actions generado
✅ Configuración de targets multiplataforma
✅ Preview local funcional
✅ Configuración commiteada al repositorio

**Criterio de completitud:** `cargo dist plan` ejecuta sin errores y muestra todos los artifacts esperados

---

### 🔷 FASE 3: Primer Release con cargo-dist

**Objetivo:** Crear y publicar el primer release oficial usando cargo-dist

#### Tareas

##### 3.1. Preparación Pre-Release

**Checklist:**
- [ ] Todas las features funcionan correctamente
- [ ] Tests pasan: `cargo test --all-features`
- [ ] Compilación release limpia: `cargo build --release`
- [ ] README actualizado con instrucciones de instalación
- [ ] CHANGELOG.md creado (opcional pero recomendado)
- [ ] Versión en Cargo.toml refleja el release (ej: 0.1.0 o 1.0.0)

**Crear CHANGELOG.md:**
```markdown
# Changelog

## [0.1.0] - 2025-XX-XX

### Added
- System monitoring with real-time TUI dashboard
- Video downloading from 1000+ platforms
- Website archiving with offline viewing
- System cleanup with safety validations
- Global alias system
- Hardware information display
- Multiple browser cookie extraction support

### Features
- Cross-platform support (Windows, Linux, macOS)
- GPU monitoring (NVIDIA and AMD)
- Interactive prompts for user-friendly experience
```

---

##### 3.2. Crear Git Tag para Release

**Determinar versión semántica:**
- `v0.1.0` - Primer beta público
- `v1.0.0` - Primer release estable (si estás listo)

**Comandos:**
```bash
# Asegurar que main está actualizado
git checkout main
git pull origin main

# Crear tag anotado con mensaje
git tag -a v0.1.0 -m "Release v0.1.0: Initial public release"

# Verificar tag
git tag -l
git show v0.1.0

# Push tag (esto trigerea GitHub Actions)
git push origin v0.1.0
```

**Importante:** El push del tag automáticamente iniciará el workflow de release.yml

---

##### 3.3. Monitoreo del GitHub Actions Workflow

**Acciones:**
1. Ir a `https://github.com/tu-usuario/msc/actions`
2. Encontrar el workflow "Release" ejecutándose
3. Monitorear cada job:
   - ✅ `plan` - Planificación de artifacts
   - ✅ `build-windows` - Compilación para Windows
   - ✅ `build-macos` - Compilación para macOS
   - ✅ `build-linux` - Compilación para Linux
   - ✅ `host` - Subida a GitHub Releases
   - ✅ `publish` - Publicación de instaladores

**Tiempo estimado:** 10-20 minutos dependiendo de la complejidad

**En caso de errores:**
- Revisar logs específicos del job fallido
- Problemas comunes:
  - Permisos de GitHub token (verificar Settings → Actions → General)
  - Dependencias faltantes en runners
  - Errores de compilación específicos de plataforma

---

##### 3.4. Verificación del Release

**Navegar a:**
```
https://github.com/tu-usuario/msc/releases/tag/v0.1.0
```

**Verificar que estén presentes:**
- [ ] `msc-v0.1.0-x86_64-pc-windows-msvc.msi` (~15-30 MB)
- [ ] `msc-v0.1.0-x86_64-apple-darwin.tar.gz`
- [ ] `msc-v0.1.0-aarch64-apple-darwin.tar.gz`
- [ ] `msc-v0.1.0-x86_64-unknown-linux-gnu.tar.gz`
- [ ] `msc-v0.1.0-aarch64-unknown-linux-gnu.tar.gz`
- [ ] `install.sh` (script de instalación universal)
- [ ] `checksums.txt` (SHA256 de todos los binarios)

**Notas de release (editar en GitHub):**
```markdown
# MSC CLI v0.1.0 - Initial Release

Multi-purpose command-line interface tool for system monitoring, media management, and productivity.

## 🎉 Highlights

- Real-time system monitoring dashboard
- Video downloading from 1000+ platforms
- Website archiving for offline viewing
- Safe system cleanup utilities
- Global command aliases

## 📦 Installation

### Windows
Download and run `msc-v0.1.0-x86_64-pc-windows-msvc.msi`

### macOS / Linux
```bash
curl -sSL https://github.com/tu-usuario/msc/releases/download/v0.1.0/install.sh | sh
```

## 🔧 Supported Platforms

- Windows (x64)
- macOS (Intel & Apple Silicon)
- Linux (x64 & ARM64)

## 📚 Documentation

See [README](https://github.com/tu-usuario/msc/blob/main/README.md) for detailed usage instructions.

---

**Full Changelog**: https://github.com/tu-usuario/msc/commits/v0.1.0
```

---

##### 3.5. Testing de Instalación

**Windows (desde máquina limpia o VM):**
```powershell
# Descargar MSI
Invoke-WebRequest -Uri "https://github.com/tu-usuario/msc/releases/download/v0.1.0/msc-v0.1.0-x86_64-pc-windows-msvc.msi" -OutFile "msc-installer.msi"

# Instalar (doble click o)
msiexec /i msc-installer.msi

# Abrir nueva terminal
msc --version
msc sys info
```

**Verificar:**
- [ ] Instalador ejecuta sin errores
- [ ] Aparece en "Programas y características"
- [ ] `msc` disponible en PATH (nueva terminal)
- [ ] Comandos funcionan correctamente

**macOS:**
```bash
# Usando script de instalación
curl -sSL https://github.com/tu-usuario/msc/releases/download/v0.1.0/install.sh | sh

# Verificar
msc --version
```

**Linux:**
```bash
# Instalación manual
wget https://github.com/tu-usuario/msc/releases/download/v0.1.0/msc-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
tar -xzf msc-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
sudo mv msc /usr/local/bin/
msc --version
```

---

### Entregables de Fase 3
✅ Tag v0.1.0 creado y pusheado
✅ GitHub Actions ejecutado exitosamente
✅ Release publicado en GitHub
✅ Instaladores funcionales en todas las plataformas
✅ Instalación verificada en al menos 2 plataformas

**Criterio de completitud:** Usuarios pueden descargar e instalar MSC desde GitHub Releases sin intervención manual

---

### 🔷 FASE 4: Implementación de Auto-actualización

**Objetivo:** Añadir comando `msc update` para auto-actualización desde GitHub Releases

#### Tareas

##### 4.1. Añadir Dependencia self_update

**Editar Cargo.toml:**
```toml
[dependencies]
# ... dependencias existentes ...
self_update = { version = "0.42", features = ["compression-flate2", "rustls"] }
```

**Notas sobre features:**
- `compression-flate2` - Descomprimir archives .tar.gz
- `rustls` - TLS puro Rust (no requiere OpenSSL)

**Compilar para verificar:**
```bash
cargo build
```

---

##### 4.2. Crear Módulo de Actualización

**Crear archivo:** `src/commands/update.rs`

```rust
use anyhow::Result;
use self_update::{cargo_crate_version, Status};

/// Actualiza msc a la última versión desde GitHub Releases
pub fn execute() -> Result<()> {
    println!("🔍 Verificando actualizaciones...");

    let target = self_update::get_target();
    let current_version = cargo_crate_version!();

    println!("Versión actual: {}", current_version);
    println!("Plataforma: {}", target);

    let status = self_update::backends::github::Update::configure()
        .repo_owner("tu-usuario")         // ⚠️ CAMBIAR
        .repo_name("msc")                 // ⚠️ CAMBIAR
        .bin_name("msc")
        .target(&target)
        .show_download_progress(true)
        .show_output(true)
        .no_confirm(false)                // Pedir confirmación
        .current_version(current_version)
        .build()?
        .update()?;

    match status {
        Status::UpToDate(version) => {
            println!("✅ Ya estás en la última versión: {}", version);
        }
        Status::Updated(version) => {
            println!("🎉 ¡Actualizado exitosamente a versión {}!", version);
            println!("\n💡 Reinicia tu terminal si es necesario.");
        }
    }

    Ok(())
}
```

---

##### 4.3. Añadir Comando Update al CLI

**Editar `src/commands/mod.rs`:**
```rust
pub mod update;
// ... otros módulos ...
```

**Editar `src/main.rs`:**

Añadir subcomando a la estructura CLI:
```rust
#[derive(Parser)]
#[command(name = "msc")]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    // ... comandos existentes ...

    /// Update msc to the latest version
    Update,
}
```

Añadir case al match:
```rust
match cli.command {
    // ... otros casos ...

    Commands::Update => {
        if let Err(e) = commands::update::execute() {
            eprintln!("❌ Error al actualizar: {}", e);
            process::exit(1);
        }
    }
}
```

---

##### 4.4. Manejo de Permisos en Windows

**Problema:** Si MSC está instalado en `Program Files`, el update requiere permisos de administrador.

**Solución 1 (Recomendada):** Detectar y pedir elevación

**Editar `src/commands/update.rs`:**
```rust
#[cfg(windows)]
fn require_admin() -> Result<()> {
    use crate::platform::elevation;

    if !elevation::is_elevated() {
        println!("⚠️  La actualización requiere permisos de administrador.");
        println!("Por favor, ejecuta este comando como administrador:");
        println!("  - Click derecho en terminal → 'Ejecutar como administrador'");
        println!("  - O usa: runas /user:Administrator \"msc update\"");
        anyhow::bail!("Se requieren permisos de administrador");
    }
    Ok(())
}

pub fn execute() -> Result<()> {
    #[cfg(windows)]
    require_admin()?;

    // ... resto del código ...
}
```

**Solución 2:** Instalar en directorio de usuario

Modificar cargo-dist para instalar en `%LOCALAPPDATA%` en lugar de `Program Files`:
```toml
[workspace.metadata.dist]
# ...
install-path = ["$LOCALAPPDATA/msc", "$HOME/.local/bin"]
```

Esto permite updates sin admin pero reduce visibilidad del programa.

---

##### 4.5. Testing de Auto-actualización

**Preparar test:**
1. Instalar versión v0.1.0
2. Crear versión v0.1.1 con cambio menor
3. Publicar v0.1.1 a GitHub Releases
4. Ejecutar `msc update` desde v0.1.0

**Crear versión v0.1.1:**
```bash
# Cambiar versión en Cargo.toml
# [package]
# version = "0.1.1"

git add Cargo.toml
git commit -m "chore: bump version to 0.1.1"
git push origin main
git tag -a v0.1.1 -m "Release v0.1.1: Add self-update functionality"
git push origin v0.1.1
```

**Ejecutar update:**
```bash
# Desde instalación de v0.1.0
msc update
```

**Salida esperada:**
```
🔍 Verificando actualizaciones...
Versión actual: 0.1.0
Plataforma: x86_64-pc-windows-msvc
Nueva versión disponible: 0.1.1
¿Deseas actualizar? [y/N]: y
⬇️  Descargando...
[████████████████████] 100%
✅ Actualizado exitosamente a versión 0.1.1!
💡 Reinicia tu terminal si es necesario.
```

**Verificar:**
```bash
msc --version
# Debería mostrar: msc 0.1.1
```

---

##### 4.6. Documentar Auto-actualización

**Actualizar README.md:**
```markdown
## Updating

MSC includes a built-in self-update feature:

```bash
msc update
```

This will check for the latest version and update automatically.

**Windows Note:** You may need to run your terminal as Administrator to update.

Alternatively, download the latest installer from [releases](https://github.com/tu-usuario/msc/releases).
```

---

### Entregables de Fase 4
✅ Dependencia self_update añadida
✅ Comando `msc update` implementado
✅ Manejo de permisos en Windows
✅ Testing exitoso de actualización
✅ Documentación actualizada

**Criterio de completitud:** `msc update` actualiza correctamente desde v0.1.0 → v0.1.1 en al menos 2 plataformas

---

### 🔷 FASE 5: Integración con Gestores de Paquetes

**Objetivo:** Publicar MSC en winget, Homebrew y AUR para facilitar instalación

#### Tareas

##### 5.1. Publicación en Windows Package Manager (winget)

**Paso 1: Fork del repositorio winget-pkgs**

```bash
# Ir a https://github.com/microsoft/winget-pkgs
# Click en "Fork"
git clone https://github.com/TU-USUARIO/winget-pkgs.git
cd winget-pkgs
```

---

**Paso 2: Crear manifiesto de paquete**

**Estructura de directorios:**
```
manifests/
  └── t/
      └── TuUsuario/
          └── MSC/
              └── 0.1.0/
                  ├── TuUsuario.MSC.installer.yaml
                  ├── TuUsuario.MSC.locale.en-US.yaml
                  └── TuUsuario.MSC.yaml
```

**Archivo: TuUsuario.MSC.yaml (manifiesto principal)**
```yaml
PackageIdentifier: TuUsuario.MSC
PackageVersion: 0.1.0
DefaultLocale: en-US
ManifestType: version
ManifestVersion: 1.6.0
```

**Archivo: TuUsuario.MSC.installer.yaml**
```yaml
PackageIdentifier: TuUsuario.MSC
PackageVersion: 0.1.0
Platform:
  - Windows.Desktop
MinimumOSVersion: 10.0.0.0
InstallerType: wix
Scope: machine
InstallModes:
  - interactive
  - silent
  - silentWithProgress
UpgradeBehavior: install
Installers:
  - Architecture: x64
    InstallerUrl: https://github.com/tu-usuario/msc/releases/download/v0.1.0/msc-v0.1.0-x86_64-pc-windows-msvc.msi
    InstallerSha256: [SHA256_HASH_DEL_MSI]  # Obtener de checksums.txt
    ProductCode: '{PRODUCT-CODE-DEL-MSI}'   # Ver abajo cómo obtenerlo
ManifestType: installer
ManifestVersion: 1.6.0
```

**Obtener SHA256:**
```bash
# Descargar checksums.txt del release
curl -L https://github.com/tu-usuario/msc/releases/download/v0.1.0/checksums.txt
# Copiar el hash correspondiente al MSI
```

**Obtener ProductCode del MSI:**
```powershell
# Windows PowerShell
$installer = "C:\path\to\msc-v0.1.0-x86_64-pc-windows-msvc.msi"
Get-AppLockerFileInformation -Path $installer | Select-Object -ExpandProperty Publisher | Select-Object -ExpandProperty BinaryName
```

O usar herramienta:
```bash
# Instalar lessmsi
choco install lessmsi

# Extraer información
lessmsi l "msc-v0.1.0-x86_64-pc-windows-msvc.msi" | grep ProductCode
```

**Archivo: TuUsuario.MSC.locale.en-US.yaml**
```yaml
PackageIdentifier: TuUsuario.MSC
PackageVersion: 0.1.0
PackageLocale: en-US
Publisher: TuUsuario
PublisherUrl: https://github.com/tu-usuario
PublisherSupportUrl: https://github.com/tu-usuario/msc/issues
PackageName: MSC
PackageUrl: https://github.com/tu-usuario/msc
License: MIT
LicenseUrl: https://github.com/tu-usuario/msc/blob/main/LICENSE
ShortDescription: Multi-purpose CLI tool for system monitoring and productivity
Description: |-
  MSC is a comprehensive command-line interface tool that combines system monitoring,
  media management, website archiving, and productivity utilities in a single application.

  Features:
  - Real-time system monitoring with TUI dashboard
  - Video downloading from 1000+ platforms
  - Website archiving for offline viewing
  - Safe system cleanup utilities
  - Global command aliases
  - Hardware information display
Moniker: msc
Tags:
  - cli
  - system-monitor
  - video-downloader
  - productivity
  - rust
  - system-information
ManifestType: defaultLocale
ManifestVersion: 1.6.0
```

---

**Paso 3: Validar manifiesto**

```bash
# Instalar winget (si no está instalado)
# Ya viene con Windows 11 y Windows 10 moderno

# Validar manifiesto
winget validate --manifest manifests/t/TuUsuario/MSC/0.1.0/
```

**Salida esperada:**
```
Manifest validation succeeded.
```

---

**Paso 4: Crear Pull Request**

```bash
# Crear branch
git checkout -b add-msc-0.1.0

# Añadir manifiestos
git add manifests/t/TuUsuario/MSC/
git commit -m "New package: MSC version 0.1.0"
git push origin add-msc-0.1.0

# Ir a GitHub y crear PR desde tu fork al repositorio oficial
```

**Título del PR:**
```
New package: TuUsuario.MSC version 0.1.0
```

**Descripción:**
```markdown
# MSC v0.1.0

Multi-purpose CLI tool for system monitoring and productivity.

## Testing

- [x] Manifest validated with `winget validate`
- [x] Installer tested on Windows 10/11
- [x] Silent install works correctly
- [x] Uninstall works correctly

## Links

- Repository: https://github.com/tu-usuario/msc
- Release: https://github.com/tu-usuario/msc/releases/tag/v0.1.0
```

---

**Paso 5: Esperar aprobación**

- ✅ Validación automatizada (5-10 minutos)
- ✅ Revisión de moderador (1-7 días)
- ✅ Merge y publicación

**Una vez aprobado, los usuarios pueden instalar con:**
```powershell
winget install TuUsuario.MSC
```

---

##### 5.2. Crear Homebrew Tap (macOS/Linux)

**Opción más rápida:** Crear tu propio "tap" en lugar de enviar a homebrew-core

**Paso 1: Crear repositorio homebrew-msc**

```bash
# En GitHub, crear nuevo repositorio: homebrew-msc
git clone https://github.com/tu-usuario/homebrew-msc.git
cd homebrew-msc
```

---

**Paso 2: Crear Formula**

**Archivo: Formula/msc.rb**
```ruby
class Msc < Formula
  desc "Multi-purpose CLI tool for system monitoring and productivity"
  homepage "https://github.com/tu-usuario/msc"
  version "0.1.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/tu-usuario/msc/releases/download/v0.1.0/msc-v0.1.0-aarch64-apple-darwin.tar.gz"
      sha256 "[SHA256_HASH_ARM]"  # De checksums.txt
    else
      url "https://github.com/tu-usuario/msc/releases/download/v0.1.0/msc-v0.1.0-x86_64-apple-darwin.tar.gz"
      sha256 "[SHA256_HASH_INTEL]"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/tu-usuario/msc/releases/download/v0.1.0/msc-v0.1.0-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "[SHA256_HASH_LINUX_ARM]"
    else
      url "https://github.com/tu-usuario/msc/releases/download/v0.1.0/msc-v0.1.0-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "[SHA256_HASH_LINUX_X64]"
    end
  end

  def install
    bin.install "msc"

    # Opcional: instalar completions si los tienes
    # bash_completion.install "completions/msc.bash" => "msc"
    # zsh_completion.install "completions/_msc"
    # fish_completion.install "completions/msc.fish"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/msc --version")
  end
end
```

---

**Paso 3: Publicar tap**

```bash
git add Formula/msc.rb
git commit -m "Add msc formula v0.1.0"
git push origin main
```

---

**Paso 4: Documentar instalación**

**Actualizar README.md principal:**
```markdown
## Installation

### macOS / Linux (Homebrew)

```bash
brew tap tu-usuario/msc
brew install msc
```
```

**Usuarios pueden ahora instalar con:**
```bash
brew tap tu-usuario/msc
brew install msc
```

**Actualizar:**
```bash
brew update
brew upgrade msc
```

---

##### 5.3. Publicación en Arch User Repository (AUR) - Opcional

**Solo para usuarios avanzados de Linux**

**Paso 1: Crear PKGBUILD**

```bash
# Crear directorio local
mkdir msc-bin
cd msc-bin
```

**Archivo: PKGBUILD**
```bash
# Maintainer: Tu Nombre <tu-email@ejemplo.com>
pkgname=msc-bin
pkgver=0.1.0
pkgrel=1
pkgdesc="Multi-purpose CLI tool for system monitoring and productivity"
arch=('x86_64' 'aarch64')
url="https://github.com/tu-usuario/msc"
license=('MIT')
provides=('msc')
conflicts=('msc')

source_x86_64=("https://github.com/tu-usuario/msc/releases/download/v${pkgver}/msc-v${pkgver}-x86_64-unknown-linux-gnu.tar.gz")
source_aarch64=("https://github.com/tu-usuario/msc/releases/download/v${pkgver}/msc-v${pkgver}-aarch64-unknown-linux-gnu.tar.gz")

sha256sums_x86_64=('[SHA256_HASH]')
sha256sums_aarch64=('[SHA256_HASH_ARM]')

package() {
    install -Dm755 msc "$pkgdir/usr/bin/msc"

    # Opcional: licencia
    # install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
```

**Paso 2: Publicar a AUR**

```bash
# Crear repositorio git
git clone ssh://aur@aur.archlinux.org/msc-bin.git
cd msc-bin
cp ../PKGBUILD .
makepkg --printsrcinfo > .SRCINFO

git add PKGBUILD .SRCINFO
git commit -m "Initial commit: msc-bin 0.1.0"
git push
```

**Usuarios Arch pueden instalar con:**
```bash
yay -S msc-bin
# o
paru -S msc-bin
```

---

### Entregables de Fase 5
✅ Manifiesto winget creado y PR enviado
✅ Homebrew tap creado y publicado
✅ (Opcional) Paquete AUR publicado
✅ Documentación actualizada con todos los métodos de instalación

**Criterio de completitud:** Usuarios pueden instalar MSC usando al menos 2 gestores de paquetes diferentes

---

### 🔷 FASE 6: Mejoras Post-Lanzamiento

**Objetivo:** Pulir la experiencia de distribución y añadir características avanzadas

#### Tareas

##### 6.1. Generación de Completions de Shell

**Clap puede generar completions automáticamente**

**Editar `src/main.rs`:**

Añadir subcomando hidden:
```rust
use clap::CommandFactory;
use clap_complete::{generate, shells::Shell};

#[derive(Subcommand)]
enum Commands {
    // ... comandos existentes ...

    /// Generate shell completions (hidden from help)
    #[command(hide = true)]
    Completions {
        #[arg(value_enum)]
        shell: Shell,
    },
}

// En el match de commands:
Commands::Completions { shell } => {
    generate(
        shell,
        &mut Cli::command(),
        "msc",
        &mut std::io::stdout()
    );
}
```

**Añadir dependencia:**
```toml
[dependencies]
clap_complete = "4.5"
```

**Generar completions:**
```bash
msc completions bash > msc.bash
msc completions zsh > _msc
msc completions fish > msc.fish
msc completions powershell > _msc.ps1
```

**Incluir en instaladores:**
- Añadir a Homebrew formula
- Incluir en cargo-dist archives

---

##### 6.2. Añadir Verificación de Firmas

**Para seguridad adicional, firmar releases**

**Opción 1: GPG Signatures**
```bash
# Generar clave GPG
gpg --full-generate-key

# Exportar clave pública
gpg --armor --export tu-email@ejemplo.com > public-key.asc

# Configurar GitHub Actions para firmar
# Añadir GPG_PRIVATE_KEY a secrets
```

**Configurar en cargo-dist:**
```toml
[workspace.metadata.dist]
# ...
checksum = "sha256"
```

**Opción 2: Cosign (recomendado para 2025)**
```yaml
# En .github/workflows/release.yml
- name: Install cosign
  uses: sigstore/cosign-installer@v3

- name: Sign artifacts
  run: |
    cosign sign-blob \
      --key env://COSIGN_KEY \
      msc-*.tar.gz \
      --output-signature=signature.sig
```

---

##### 6.3. Crear Website de Documentación

**Opción 1: GitHub Pages con mdBook**

```bash
cargo install mdbook
mdbook init docs
cd docs
mdbook build
```

**Estructura:**
```
docs/
  └── src/
      ├── SUMMARY.md
      ├── installation.md
      ├── quickstart.md
      ├── commands/
      │   ├── sys.md
      │   ├── vget.md
      │   ├── wget.md
      │   └── clean.md
      └── faq.md
```

**Publicar con GitHub Pages:**
```yaml
# .github/workflows/docs.yml
name: Deploy Docs

on:
  push:
    branches: [main]

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install mdBook
        run: cargo install mdbook
      - name: Build docs
        run: mdbook build docs
      - name: Deploy to GitHub Pages
        uses: peaceiris/actions-gh-pages@v3
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          publish_dir: ./docs/book
```

**URL:** `https://tu-usuario.github.io/msc/`

---

##### 6.4. Analytics y Telemetría (Opcional)

**Considerar añadir telemetría opt-in para entender uso**

**Nunca recopilar:**
- Datos personales
- Paths del usuario
- Contenido de archivos

**Sí recopilar (con consentimiento):**
- Comandos usados (sin argumentos)
- Plataforma/versión de OS
- Versión de MSC
- Crashes/errores

**Implementación:**
```rust
// Pregunta en primer uso
if !config.telemetry_configured {
    let enable = dialoguer::Confirm::new()
        .with_prompt("¿Permitir telemetría anónima para mejorar MSC?")
        .default(false)
        .interact()?;

    config.telemetry_enabled = enable;
    config.telemetry_configured = true;
    config.save()?;
}
```

---

##### 6.5. Badges y Métricas en README

**Añadir badges profesionales:**

```markdown
# MSC CLI

[![Crates.io](https://img.shields.io/crates/v/msc.svg)](https://crates.io/crates/msc)
[![Downloads](https://img.shields.io/github/downloads/tu-usuario/msc/total.svg)](https://github.com/tu-usuario/msc/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![CI](https://github.com/tu-usuario/msc/workflows/CI/badge.svg)](https://github.com/tu-usuario/msc/actions)

Multi-purpose command-line interface tool for system monitoring, media management, and productivity.

[Installation](#installation) | [Documentation](https://tu-usuario.github.io/msc/) | [Changelog](CHANGELOG.md)
```

---

##### 6.6. Configurar Dependabot

**Para mantener dependencias actualizadas**

**Archivo: `.github/dependabot.yml`**
```yaml
version: 2
updates:
  - package-ecosystem: "cargo"
    directory: "/"
    schedule:
      interval: "weekly"
    open-pull-requests-limit: 10

  - package-ecosystem: "github-actions"
    directory: "/"
    schedule:
      interval: "weekly"
```

---

### Entregables de Fase 6
✅ Shell completions generados e incluidos
✅ (Opcional) Firmas de releases implementadas
✅ Website de documentación publicado
✅ README con badges profesionales
✅ Dependabot configurado

**Criterio de completitud:** Proyecto tiene apariencia profesional y mantenimiento automatizado

---

## Cronograma y Dependencias

### Cronograma Sugerido

| Fase | Duración Estimada | Dependencias |
|------|-------------------|--------------|
| Fase 1: Preparación | 1-2 días | Ninguna |
| Fase 2: cargo-dist Setup | 2-3 horas | Fase 1 completa |
| Fase 3: Primer Release | 1 día | Fase 2 completa |
| Fase 4: Auto-actualización | 4-6 horas | Fase 3 completa |
| Fase 5: Gestores de Paquetes | 3-5 días (aprobaciones) | Fase 3 completa |
| Fase 6: Mejoras | 2-3 días | Fase 3 completa |

**Total:** ~2 semanas (con aprobaciones de winget/homebrew)

---

### Diagrama de Dependencias

```
Fase 1 (Preparación)
        ↓
Fase 2 (cargo-dist)
        ↓
Fase 3 (Primer Release)
        ├─────────────┬─────────────┐
        ↓             ↓             ↓
Fase 4 (Update)  Fase 5 (Pkgs)  Fase 6 (Mejoras)
        ↓             ↓             ↓
     v0.1.1      winget/brew    Docs/Badges
```

---

### Orden de Prioridades

**Crítico (Bloquea distribución):**
1. ✅ Fase 1 - Preparación
2. ✅ Fase 2 - cargo-dist
3. ✅ Fase 3 - Primer Release

**Importante (Mejora experiencia):**
4. ✅ Fase 4 - Auto-actualización
5. ✅ Fase 5 - Gestores de paquetes

**Opcional (Pulido profesional):**
6. ⚪ Fase 6 - Mejoras

---

## Referencias y Recursos

### Documentación Oficial

- [cargo-dist Book](https://opensource.axo.dev/cargo-dist/book/)
- [cargo-dist en crates.io](https://crates.io/crates/cargo-dist)
- [self_update crate](https://docs.rs/self_update)
- [Rust CLI Book - Packaging](https://rust-cli.github.io/book/tutorial/packaging.html)
- [Windows Package Manager Docs](https://learn.microsoft.com/en-us/windows/package-manager/winget/)
- [winget-pkgs Repository](https://github.com/microsoft/winget-pkgs)
- [Homebrew Formula Cookbook](https://docs.brew.sh/Formula-Cookbook)

### Herramientas

- [cargo-dist GitHub](https://github.com/axodotdev/cargo-dist)
- [cargo-wix GitHub](https://github.com/volks73/cargo-wix)
- [self_update GitHub](https://github.com/jaemk/self_update)

### Ejemplos de Proyectos

Estudiar cómo otros proyectos Rust hacen distribución:

- **ripgrep** - https://github.com/BurntSushi/ripgrep (cargo-dist, winget)
- **bat** - https://github.com/sharkdp/bat (cargo-dist, homebrew)
- **fd** - https://github.com/sharkdp/fd (todos los gestores)
- **starship** - https://github.com/starship/starship (distribución completa)

### Comunidad y Soporte

- [Discord de Axo.dev](https://discord.gg/axo) - Para ayuda con cargo-dist
- [Rust Users Forum](https://users.rust-lang.org/)
- [r/rust en Reddit](https://reddit.com/r/rust)

---

## Checklist Final de Pre-Release

Antes de crear el primer release oficial (v1.0.0), verificar:

### Código
- [ ] Todos los tests pasan
- [ ] No hay warnings de compilación
- [ ] Código documentado adecuadamente
- [ ] No hay TODOs críticos pendientes

### Documentación
- [ ] README.md completo y actualizado
- [ ] CHANGELOG.md con historial
- [ ] LICENSE presente
- [ ] Comentarios de código claros

### Configuración
- [ ] Cargo.toml con metadata completa
- [ ] .gitignore apropiado
- [ ] cargo-dist configurado
- [ ] GitHub Actions funcional

### Seguridad
- [ ] Dependencies actualizadas (`cargo update`)
- [ ] `cargo audit` sin vulnerabilidades
- [ ] No hay secrets hardcodeados
- [ ] Path validation implementada

### Testing
- [ ] Probado en Windows
- [ ] Probado en macOS
- [ ] Probado en Linux
- [ ] Instaladores funcionan en cada plataforma

### Legal
- [ ] Licencias de dependencias revisadas
- [ ] Atribuciones necesarias incluidas
- [ ] Términos de uso claros

---

## Notas Finales

### Mejores Prácticas

1. **Semantic Versioning:** Seguir estrictamente semver.org
   - MAJOR: Cambios incompatibles
   - MINOR: Nuevas features compatibles
   - PATCH: Bug fixes compatibles

2. **Changelog:** Mantener actualizado con cada release
   - Usa formato Keep a Changelog

3. **Comunicación:** Anunciar releases en:
   - GitHub Discussions
   - Twitter/Mastodon (si tienes cuenta)
   - Reddit r/rust
   - Rust Users Forum

4. **Feedback:** Responder a issues y PRs prontamente
   - Triage semanal de issues
   - Labels claros (bug, enhancement, help-wanted)

### Mantenimiento Continuo

**Semanalmente:**
- Revisar issues nuevos
- Merge de Dependabot PRs

**Por Release:**
- Actualizar CHANGELOG
- Testing en 3 plataformas
- Verificar instaladores

**Anualmente:**
- Auditoría de seguridad completa
- Revisión de roadmap
- Actualización de dependencias mayores

---

## Conclusión

Este plan proporciona una ruta clara desde el estado actual de MSC hasta una aplicación distribuida profesionalmente con:

✅ Instalación automatizada multiplataforma
✅ Gestión de PATH sin intervención manual
✅ Auto-actualización integrada
✅ Presencia en gestores de paquetes principales
✅ Proceso de release completamente automatizado

**Siguiente paso:** Comenzar con Fase 1 - Preparación del Proyecto.

---

**Documento creado:** 2025-12-26
**Basado en:** Investigación de mejores prácticas 2025, documentación oficial de cargo-dist v0.30.2, self_update v0.42, y análisis del proyecto MSC CLI actual.
