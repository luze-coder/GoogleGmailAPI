# Gmail API Schema Discovery Worker - Work Plan

## 1. Introducción

### Objetivo

Desarrollar un worker ejecutable desde línea de comandos (CLI) cuya responsabilidad sea consumir uno o más endpoints de Gmail API, almacenar sus respuestas y detectar automáticamente cambios en la estructura (schema) de dichas respuestas.

La aplicación actuará como un repositorio histórico de schemas, permitiendo identificar cuándo un proveedor externo introduce modificaciones en los contratos de sus APIs sin previo aviso.

### Alcance

El worker deberá:

- Consumir endpoints configurados de Gmail API.
- Almacenar las respuestas obtenidas.
- Generar automáticamente un schema a partir de cada respuesta JSON.
- Calcular un hash representativo del schema generado.
- Comparar dicho hash contra la última versión registrada para el endpoint correspondiente.
- Crear una nueva versión únicamente cuando exista un cambio estructural.
- Mantener un historial auditable de schemas por endpoint.

### Fuera de Alcance

La aplicación no realizará:

- Procesamiento de negocio.
- Transformación de datos.
- Validación funcional de los payloads.
- Sincronización de información hacia otros sistemas.

Su única responsabilidad será capturar, almacenar y versionar schemas.

### Stack Propuesto

- Lenguaje: Node.js + TypeScript
- Cliente HTTP: Fetch
- Base de datos: PostgreSQL
- Cliente PostgreSQL: pg
- CLI: Commander.js
- Logging: console.log
- Hashing: crypto (nativo de Node.js)

### Persistencia

Se almacenarán:

- Endpoints monitoreados.
- Respuestas obtenidas desde Gmail API.
- Schemas inferidos.
- Hashes de schemas.
- Versiones históricas por endpoint.

---

# 2. API Externa

## Fuente de Datos

Google Gmail API.

## Endpoints

El worker deberá ser capaz de consumir cualquier endpoint configurado de Gmail API.

Ejemplos:

```http
GET /gmail/v1/users/me/messages

GET /gmail/v1/users/me/messages/{messageId}

GET /gmail/v1/users/me/labels

GET /gmail/v1/users/me/profile

GET /gmail/v1/users/me/history
```

Los endpoints no estarán definidos de forma fija dentro de la aplicación.

Serán registrados en la base de datos y procesados dinámicamente por el worker.

## Autenticación

La comunicación con Gmail API se realizará mediante OAuth 2.0.

```http
Authorization: Bearer <access_token>
```

## Flujo de Consumo

Para cada endpoint configurado:

```text
Endpoint Configurado
        |
        v
Request Gmail API
        |
        v
Respuesta JSON
        |
        v
Persistencia
        |
        v
Inferencia de Schema
        |
        v
Generación de Hash
        |
        v
Comparación de Versiones
```

---

# 3. Descubrimiento y Versionado de Schema

## Generación de Schema

Cada respuesta recibida desde Gmail API será recorrida recursivamente para generar una representación estructural de su contenido.

Ejemplo:

Payload recibido:

```json
{
  "id": "123",
  "threadId": "456",
  "labelIds": ["INBOX"],
  "unread": true
}
```

Schema inferido:

```json
{
  "id": "string",
  "threadId": "string",
  "labelIds": [
    "string"
  ],
  "unread": "boolean"
}
```

El schema representa exclusivamente la estructura del JSON y no sus valores.

## Generación de Hash

Una vez inferido el schema, se generará un hash SHA-256 que será utilizado como mecanismo de comparación.

Ejemplo:

Schema:

```json
{
  "id": "string",
  "threadId": "string"
}
```

Hash generado:

```text
a12b34c5f0f9d8...
```

Este hash representa la versión estructural del endpoint en ese momento.

## Comparación

Cada endpoint mantendrá su propio historial de versiones.

Al obtener una nueva respuesta:

1. Se genera el schema.
2. Se calcula el hash.
3. Se obtiene el último hash registrado para ese endpoint.
4. Se comparan ambos valores.

### Sin Cambios

```text
hash_actual == hash_registrado
```

Resultado:

- Se almacena la respuesta.
- No se genera una nueva versión.

### Con Cambios

```text
hash_actual != hash_registrado
```

Resultado:

- Se crea una nueva versión.
- Se almacena un snapshot completo del nuevo schema.
- Se registra el nuevo hash.
- Se mantiene el historial de versiones anterior.

## Ejemplo

Schema V1:

```json
{
  "id": "string",
  "threadId": "string"
}
```

Hash:

```text
a12b34c5
```

Schema V2:

```json
{
  "id": "string",
  "threadId": "string",
  "labelIds": [
    "string"
  ]
}
```

Hash:

```text
d87f12e2
```

Resultado:

```text
a12b34c5 != d87f12e2
```

Se genera una nueva versión del schema para dicho endpoint.

---

# 4. Modelo de Datos y Base de Datos

## Motor de Base de Datos

PostgreSQL.

---

## Tabla: api_endpoints

Catálogo de endpoints monitoreados.

```sql
CREATE TABLE api_endpoints (
    id BIGSERIAL PRIMARY KEY,
    endpoint_name VARCHAR(100) NOT NULL,
    endpoint_url VARCHAR(500) NOT NULL,
    active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);
```

Ejemplo:

| id | endpoint_name |
|----|---------------|
| 1 | messages |
| 2 | message_detail |
| 3 | labels |
| 4 | profile |

---

## Tabla: api_responses

Almacena el payload completo obtenido desde el endpoint.

```sql
CREATE TABLE api_responses (
    id BIGSERIAL PRIMARY KEY,
    endpoint_id BIGINT NOT NULL,
    fetched_at TIMESTAMP NOT NULL,
    payload JSONB NOT NULL,
    schema_version_id BIGINT NULL
);
```

Responsabilidades:

- Mantener un histórico de respuestas.
- Permitir auditoría y análisis posterior.
- Asociar la respuesta con la versión de schema vigente al momento de su captura.

---

## Tabla: schema_versions

Mantiene el historial de versiones por endpoint.

```sql
CREATE TABLE schema_versions (
    id BIGSERIAL PRIMARY KEY,
    endpoint_id BIGINT NOT NULL,
    version_number INTEGER NOT NULL,
    schema_hash VARCHAR(64) NOT NULL,
    detected_at TIMESTAMP NOT NULL
);
```

Ejemplo:

| endpoint | version |
|-----------|----------|
| messages | 1 |
| messages | 2 |
| messages | 3 |
| labels | 1 |
| profile | 1 |
| profile | 2 |

Cada endpoint posee un ciclo de versionado independiente.

---

## Tabla: schema_snapshots

Almacena la representación completa del schema correspondiente a cada versión.

```sql
CREATE TABLE schema_snapshots (
    id BIGSERIAL PRIMARY KEY,
    schema_version_id BIGINT NOT NULL,
    schema_definition JSONB NOT NULL
);
```

Ejemplo:

```json
{
  "id": "string",
  "threadId": "string",
  "labelIds": [
    "string"
  ]
}
```

Esta tabla permite reconstruir cualquier versión histórica del schema.

---

# Flujo General

```text
CLI
 |
 v
Obtener Endpoints Activos
 |
 v
Consumir Gmail API
 |
 v
Guardar Payload
 |
 v
Inferir Schema
 |
 v
Calcular Hash SHA-256
 |
 v
Buscar Última Versión del Endpoint
 |
 +---- Hash Igual -----------------> Fin
 |
 +---- Hash Diferente -------------> Crear Nueva Versión
                                     |
                                     +-> Guardar Hash
                                     |
                                     +-> Guardar Snapshot
                                     |
                                     +-> Asociar Respuesta
```

---

# Consideraciones de Diseño

- Cada endpoint mantiene un historial de schemas independiente.
- Los payloads originales se almacenan sin modificaciones.
- El versionado se basa exclusivamente en cambios estructurales.
- La comparación utiliza hashes de schema para minimizar procesamiento.
- Toda versión generada debe poder ser auditada y reconstruida desde la base de datos.
- El worker podrá ejecutarse manualmente o mediante un scheduler externo (Cron, Kubernetes Job, Jenkins, etc.).