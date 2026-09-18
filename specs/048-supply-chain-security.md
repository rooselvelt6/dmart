# SPEC-048: Supply Chain Security — SBOM + firma (cosign) + provenance SLSA

## Contexto
- **Problema a resolver**: los artefactos (imagen `ghcr.io/ucigtm/dmart-server`, binario
  `dmart-server`, bundle WASM `dist/**`) se publican sin inventario de dependencias (SBOM)
  ni firma verificable, por lo que un tercero no puede comprobar procedencia ni integridad.
  Hoy CI construye la imagen con `push: false` y `release.yml` no firma ni genera SBOM.
- **Usuario objetivo**: ingeniería/seguridad y auditoría externa (Fase 4.2).
- **Métrica de éxito (KPI)**: 100 % de releases con SBOM (CycloneDX) publicado, imagen
  firmada con cosign (keyless/OIDC) y attestation de provenance SLSA build level 2+;
  `cosign verify` y `cosign verify-attestation` exitosos.

## Acceptance Criteria (Gherkin)

```gherkin
Feature: Cadena de suministro verificable
  As a security engineer
  I want every released artifact signed with a published SBOM
  So that provenance and integrity can be verified without trusting the build host

  Scenario: Tag de release construye, firma y publica
    Given un push de tag "vX.Y.Z"
    When corre el workflow "release-image"
    Then se publica la imagen ghcr.io/ucigtm/dmart-server:vX.Y.Z
    And se sube el SBOM CycloneDX como asset y attestation
    And cosign firma la imagen por keyless (Fulcio/Rekor)
    And se genera provenance SLSA build level 3 para la imagen

  Scenario: Binario y WASM con provenance SLSA nivel 2
    Given el job "release" de release.yml
    When publica target/release/dmart-server y dist/**
    Then se genera SBOM CycloneDX del binario con cargo-cyclonedx
    And actions/attest-build-provenance emite attestation verificable
    And el SBOM queda como asset de la GitHub Release

  Scenario: Verificación local de la imagen
    Given una imagen publicada
    When se ejecuta scripts/verify-image.sh <ref>
    Then cosign verify valida la firma keyless contra el OIDC esperado
    And cosign verify-attestation valida el SBOM adjunto
    And el script falla con código != 0 si algo no verifica
```

## API Contracts
No expone endpoints HTTP. Contratos de artefactos:

| Artefacto | Publicación | Verificación |
|-----------|-------------|--------------|
| Imagen `ghcr.io/ucigtm/dmart-server:<tag>` | `docker push` + `cosign sign` | `cosign verify` / `cosign verify-attestation --type cyclonedx` |
| `dmart-server.sbom.cdx.json` | GitHub Release asset | `cosign verify-blob` / hash |
| `target/release/dmart-server`, `dist/**` | GitHub Release + `attest-build-provenance` | `gh attestation verify` |

## Data Models
No aplica (no hay cambios SurrealQL).

### Migraciones requeridas
- Ninguna.

## Edge Cases
| # | Caso | Comportamiento esperado |
|---|------|------------------------|
| 1 | Tag sin permisos OIDC | El job falla explícitamente pidiendo `id-token: write` |
| 2 | Registry privado/no autenticado | `docker/login-action` con `GITHUB_TOKEN`, falla clara |
| 3 | Firma sin red (Rekor no disponible) | `--yes` keyless falla el job (no publicar sin firma) |
| 4 | SBOM vacío | El paso verifica tamaño > 0, si no falla |

## Security Considerations

### Threat Model (STRIDE)
| Threat | Mitigación |
|--------|------------|
| Spoofing | Firma keyless OIDC contra la identidad del workflow |
| Tampering | Digest fijado + SBOM attestation + hash del binario |
| Repudiation | Transparencia en Rekor + attestation de provenance |
| Information Disclosure | El SBOM incluye solo dependencias, sin secretos |
| Denial of Service | N/A |
| Elevation of Privilege | Permisos mínimos por job (`packages: write`, `id-token: write`) |

### Data Classification
- [x] Operational/Metadata
- [ ] PHI / PII / Clinical Data

### Auth/Autz Requirements
- Permisos de workflow: `contents: write`, `packages: write`, `id-token: write`,
  `attestations: write`. Sin credenciales de larga vida (solo `GITHUB_TOKEN`).

## Testing Strategy

### Unit Tests
- N/A (CI/infra). Se valida con `actionlint` y por ejecución real en el release.

### Integration Tests
- [x] Ejecución del workflow en tag real; verificar con `scripts/verify-image.sh`.
- [x] `gitops-sync` sigue validando `helm template` sin regresiones.

### Definition of Done
- [x] Workflow `release-image.yml` en main.
- [x] `release.yml` con SBOM + provenance del binario.
- [x] `scripts/verify-image.sh` documentado en `docs/runbook`.
- [x] CHANGELOG actualizado.
