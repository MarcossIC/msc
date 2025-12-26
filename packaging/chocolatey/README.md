# Chocolatey Package for MSC

Este directorio contiene los archivos necesarios para empaquetar MSC para Chocolatey, el gestor de paquetes para Windows.

## Estructura

```
chocolatey/
└── msc/
    ├── msc.nuspec                     # Manifiesto del paquete
    └── tools/
        └── chocolateyinstall.ps1      # Script de instalación
```

## Automatización

El paquete se actualiza automáticamente con cada release a través del workflow de GitHub Actions:

1. Cuando se crea una nueva release, el workflow actualiza:
   - La versión en `msc.nuspec`
   - El checksum SHA256 en `chocolateyinstall.ps1`
   - La URL de las release notes

2. Los cambios se commitean automáticamente al repositorio

## Construcción Manual

Si necesitas construir el paquete manualmente:

```powershell
# Navegar al directorio del paquete
cd packaging/chocolatey/msc

# Construir el paquete .nupkg
choco pack

# Esto generará: msc.<version>.nupkg
```

## Prueba Local

Para probar el paquete localmente antes de publicarlo:

```powershell
# Instalar desde el archivo .nupkg local
choco install msc -s . -y

# Verificar la instalación
msc --version

# Desinstalar para limpiar
choco uninstall msc -y
```

## Publicación a Chocolatey Community

### Prerrequisitos

1. Cuenta en [Chocolatey.org](https://community.chocolatey.org/)
2. API Key de tu cuenta (Configuración → API Keys)

### Proceso de Publicación

```powershell
# Configurar tu API key (solo una vez)
choco apikey --key <TU_API_KEY> --source https://push.chocolatey.org/

# Publicar el paquete
choco push msc.<version>.nupkg --source https://push.chocolatey.org/
```

### Primera Publicación

Para la primera publicación del paquete:

1. El paquete entrará en **moderación automática**
2. Los moderadores revisarán:
   - El contenido del paquete
   - Los scripts de instalación/desinstalación
   - Las URLs de descarga
   - Los checksums

3. Este proceso puede tomar **1-3 días hábiles**

4. Una vez aprobado, futuras actualizaciones se publican **automáticamente** si:
   - Solo cambian versión, URLs y checksums
   - No se modifican los scripts de instalación

### Trusted Package Status

Después de varias versiones exitosas, puedes solicitar el estado de **Trusted Package**, que permite:
- Publicación instantánea sin moderación
- Mayor visibilidad en el repositorio
- Badge de "trusted" en la página del paquete

## Instalación para Usuarios

Una vez publicado, los usuarios podrán instalar MSC con:

```powershell
choco install msc -y
```

## Mantenimiento

### Actualización de Versión

El workflow automatiza esto, pero si necesitas actualizar manualmente:

1. Editar `msc.nuspec`:
   ```xml
   <version>NUEVA_VERSION</version>
   ```

2. El checksum en `chocolateyinstall.ps1` se actualiza automáticamente

### Verificación del Paquete

Antes de publicar, verifica:

```powershell
# Validar el .nuspec
choco pack

# Inspeccionar el contenido
choco info msc -s .
```

## Recursos

- [Chocolatey Package Guidelines](https://docs.chocolatey.org/en-us/create/create-packages)
- [Package Validator](https://docs.chocolatey.org/en-us/community-repository/moderation/package-validator)
- [Chocolatey Community Repository](https://community.chocolatey.org/)

## Troubleshooting

### Error: Checksum mismatch

Si el checksum no coincide:
1. Verificar que la URL apunta a la versión correcta
2. Descargar el archivo MSI manualmente
3. Calcular el SHA256: `Get-FileHash <archivo.msi> -Algorithm SHA256`
4. Actualizar en `chocolateyinstall.ps1`

### Error: Package already exists

Si intentas publicar una versión que ya existe:
1. Incrementa la versión en `Cargo.toml`
2. Crea una nueva release
3. El workflow actualizará automáticamente todos los manifiestos
