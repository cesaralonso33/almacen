# Actualizaciones del escritorio

Aquí se publican el instalador NSIS, su firma `.sig` y `latest.json`.

- La API sirve esta carpeta en `https://<dominio>/updates/`.
- No incluya archivos sin firma ni manifiestos de versiones antiguas.
- Los binarios y firmas están ignorados por Git.
- Genere el manifiesto con `scripts/publicar-actualizacion.ps1`.

Estructura típica:

```text
updates/
  latest.json
  Custodia CradSystem_0.2.0_x64-setup.exe
  Custodia CradSystem_0.2.0_x64-setup.exe.sig
```
