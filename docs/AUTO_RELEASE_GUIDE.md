# Automated Release Guide

Este workflow implementa **creación automática de tags** y releases basándose en la versión de `Cargo.toml`.

## 🚀 Cómo Funciona

### Flujo Automático

```
1. Haces push a main con cambios en Cargo.toml
   ↓
2. Workflow extrae versión (ej: "0.1.0")
   ↓
3. Crea tag automáticamente "v0.1.0"
   ↓
4. Compila para 5 plataformas
   ↓
5. Genera instaladores (MSI, tarballs, etc.)
   ↓
6. Publica GitHub Release automáticamente
```

### Sin Intervención Manual

**No necesitas:**
- ❌ Crear tags manualmente
- ❌ Ejecutar comandos git
- ❌ Configurar nada extra

**Solo necesitas:**
- ✅ Cambiar `version = "0.1.0"` en `Cargo.toml`
- ✅ Hacer `git push`

## 📝 Uso Básico

### Crear un Release

```bash
# 1. Cambia la versión en Cargo.toml
sed -i 's/version = "0.1.0"/version = "0.2.0"/' Cargo.toml

# 2. Commit y push
git add Cargo.toml
git commit -m "chore: bump version to 0.2.0"
git push origin main

# 3. ¡Listo! El workflow hace el resto automáticamente
```

El workflow:
- Detecta el cambio en `Cargo.toml`
- Crea tag `v0.2.0`
- Compila y publica release

### Manejo de Duplicados

Si el tag `v0.2.0` ya existe:
- ⏭️ **Por defecto:** Salta el release (no hace nada)
- 🔄 **Con force:** Crea `v0.2.0+20251226-143022` (versión + timestamp)

Para forzar con timestamp:
1. Ve a Actions → Release → Run workflow
2. Marca "Force release with timestamp"
3. Run

## 🎯 Mejoras Implementadas

Basado en `docs/UPDATE.md`:

### 1. ✅ Auto-creación de Tags
- **Antes:** Tenías que crear tags manualmente
- **Ahora:** Se crean automáticamente de `Cargo.toml`

### 2. ✅ Fuente Única de Verdad
- **Antes:** `cargo-dist` Y `gh release create`
- **Ahora:** Solo `cargo-dist` (elimina duplicación)

### 3. ✅ Permisos Mínimos
- **Antes:** `contents: write` global
- **Ahora:** Solo los jobs que lo necesitan

```yaml
# Global
permissions:
  contents: read

# Solo create-tag y release
permissions:
  contents: write
```

### 4. ✅ Flujo Simplificado

**Antes (5 jobs):**
```
plan → build → build-global → host → announce
```

**Ahora (5 jobs más claros):**
```
create-tag → plan → build → release → announce
```

### 5. ✅ Sin Duplicación de Host

- **Eliminado:** `gh release create` manual
- **Único responsable:** `cargo-dist host`

## 📊 Estructura del Workflow

```yaml
jobs:
  create-tag:       # Crea tag automáticamente
    ↓
  plan:             # Planifica qué construir
    ↓
  build:            # Compila para todas las plataformas
    ↓
  release:          # Publica release (única fuente)
    ↓
  announce:         # Notificación (opcional)
```

## 🔧 Configuración

### Triggers

El workflow se ejecuta cuando:

```yaml
on:
  push:
    branches: [main]
    paths:
      - 'Cargo.toml'    # Cambios de versión
      - 'src/**'        # Cambios de código
```

### Timestamp Format

Si hay duplicados:
```
v0.1.0+20251226-143022
       └─────┬────────┘
             └─ YYYYMMDD-HHMMSS
```

## 🎨 Ejemplos

### Ejemplo 1: Primera Release

```bash
# Cargo.toml tiene version = "0.1.0"
git push origin main

# Resultado:
# ✅ Crea tag: v0.1.0
# ✅ Publica release: v0.1.0
```

### Ejemplo 2: Nueva Versión

```bash
# Cambias a version = "0.2.0"
git push origin main

# Resultado:
# ✅ Crea tag: v0.2.0
# ✅ Publica release: v0.2.0
```

### Ejemplo 3: Hotfix del Mismo Día

```bash
# version = "0.2.1" pero v0.2.1 ya existe
# Ejecutas workflow manualmente con "force"

# Resultado:
# ✅ Crea tag: v0.2.1+20251226-150000
# ✅ Publica release: v0.2.1+20251226-150000
```

## 🚦 Estados del Workflow

### ✅ Success

```
create-tag → plan → build → release → announce
   ✓         ✓       ✓        ✓         ✓
```

Release publicado correctamente.

### ⏭️ Skipped

```
create-tag (tag exists, not forced)
   ⏭️

plan, build, release, announce
   skipped
```

Tag ya existe, no se hace nada.

### ❌ Failed

Revisa logs en:
```
https://github.com/MarcossIC/msc/actions
```

Errores comunes:
- Permisos de GitHub Actions
- Errores de compilación
- Falta de dependencias en runners

## 📦 Artifacts Generados

Cada release incluye:

```
msc-v0.1.0-x86_64-pc-windows-msvc.msi       # Windows installer
msc-v0.1.0-x86_64-apple-darwin.tar.xz       # macOS Intel
msc-v0.1.0-aarch64-apple-darwin.tar.xz      # macOS ARM
msc-v0.1.0-x86_64-unknown-linux-gnu.tar.xz  # Linux x64
msc-v0.1.0-aarch64-unknown-linux-gnu.tar.xz # Linux ARM
msc-installer.sh                             # Universal installer
sha256.sum                                   # Checksums
```

## 🔐 Seguridad

### Permisos Granulares

Solo 2 jobs tienen write:
1. `create-tag` - Para pushear el tag
2. `release` - Para crear el release

Los demás jobs son read-only.

### Checksums

Todos los binarios incluyen SHA256:
```bash
# Verificar
sha256sum -c msc-v0.1.0-x86_64-pc-windows-msvc.msi.sha256
```

## 🛠️ Troubleshooting

### Tag existe pero quiero republicar

```bash
# Opción 1: Borrar tag y volver a pushear
git tag -d v0.1.0
git push origin :refs/tags/v0.1.0
git push origin main  # Recrea el tag

# Opción 2: Forzar con timestamp
# GitHub Actions → Release → Run workflow → Force ✓
```

### Workflow no se ejecuta

Verifica:
1. El cambio está en `Cargo.toml` o `src/`
2. Pusheaste a `main`
3. GitHub Actions está habilitado (Settings → Actions)

### Build falla

Revisa:
1. `cargo build --release` funciona localmente
2. Todas las dependencias están en `Cargo.toml`
3. Tests pasan: `cargo test`

## 📚 Próximos Pasos

Después del release automático:

1. **Completa integración winget**
   - Sigue `packaging/POST_RELEASE_STEPS.md`
   - Envía PR a microsoft/winget-pkgs

2. **Publica Homebrew tap**
   - Actualiza `packaging/homebrew/msc.rb`
   - Push a repositorio homebrew-msc

3. **Publica a AUR**
   - Actualiza `packaging/aur/PKGBUILD`
   - Push a AUR

## 🎯 Resumen

### Antes
```bash
git tag -a v0.1.0 -m "Release"
git push origin v0.1.0
# Esperas a que compile
# Verificas release
```

### Ahora
```bash
# Cambias version en Cargo.toml
git push origin main
# ¡Listo! 🎉
```

**Todo lo demás es automático.**

---

## Comandos Útiles

```bash
# Ver tags remotos
git ls-remote --tags origin

# Ver último release
gh release view

# Listar workflows
gh workflow list

# Ver runs del workflow
gh run list --workflow=release.yml

# Ver logs de un run
gh run view <run-id> --log
```
