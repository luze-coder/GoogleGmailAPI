# Gmail API Worker

Worker ejecutable por CLI desarrollado en Rust para procesar tareas de una cola, consumir la Gmail API, persistir respuestas JSON en SQLite y detectar cambios estructurales mediante versionado de schemas.

## Funcionalidades

- Toma exclusiva de tareas pendientes mediante locking transaccional.
- Autenticación OAuth 2.0 con `client_id`, `client_secret` y `refresh_token`.
- Consumo de `GET /gmail/v1/users/me/messages`.
- Persistencia del payload completo en SQLite.
- Inferencia de schema y cálculo de hash SHA-256 determinístico.
- Creación de versiones y snapshots cuando cambia la estructura.
- Registro de eventos operativos y trazabilidad por tarea.
- Estados de tarea: `PENDING`, `PROCESSING`, `COMPLETED` y `FAILED`.

## Requisitos

- Rust y Cargo.
- Una cuenta de Google con credenciales OAuth 2.0 para Gmail API.
- SQLite3 opcional para inspeccionar la base de datos directamente.

La base de datos utiliza SQLite embebido mediante `rusqlite`, por lo que SQLite3 no es necesario para compilar ni ejecutar el worker.

## Configuración

1. Configura un proyecto en Google Cloud y habilita Gmail API.
2. Obtén las credenciales OAuth 2.0 y un refresh token.
3. Copia la plantilla de variables de entorno:

```powershell
Copy-Item .env.example .env
```

4. Completa `.env`:

```env
GMAIL_CLIENT_ID=tu-client-id.apps.googleusercontent.com
GMAIL_CLIENT_SECRET=tu-client-secret
GMAIL_REFRESH_TOKEN=tu-refresh-token
```

La guía completa está en [Documentacion/02-GMAIL_API_SETUP.md](Documentacion/02-GMAIL_API_SETUP.md).

## Ejecución rápida

Desde la raíz del proyecto:

```powershell
cargo build
cargo run -- --init-db
cargo run -- --queue-example
cargo run -- --list-tasks
cargo run -- --process-one
cargo run -- --list-tasks
```

`--process-one` toma la primera tarea pendiente del worker `gmail`, consulta Gmail API, guarda el payload, calcula el schema, registra los eventos y actualiza el estado final.

La base de datos se crea en `db/gmail_worker.db`.

## Comandos disponibles

| Comando | Descripción |
|---|---|
| `cargo run -- --init-db` | Crea o actualiza las tablas de SQLite. |
| `cargo run -- --queue-example` | Inserta una tarea de ejemplo para Gmail API. |
| `cargo run -- --list-tasks` | Lista las tareas y sus estados. |
| `cargo run -- --process-one` | Procesa una tarea pendiente. |
| `cargo run -- --help` | Muestra la ayuda del worker. |

Si no se especifica un comando, el programa utiliza `--process-one`.

## Flujo de procesamiento

```text
task_queue (PENDING)
        |
        v
Toma exclusiva de la tarea
        |
        v
Refresh del access token OAuth
        |
        v
GET /gmail/v1/users/me/messages
        |
        v
Guardar payload en assets
        |
        v
Inferir schema y calcular SHA-256
        |
        +-- hash igual ------> continuar
        |
        +-- hash diferente --> crear version y snapshot
        |
        v
Registrar eventos y completar tarea
```

Cuando ocurre un error, la tarea pasa a `FAILED` y se registra un evento `TaskFailed`.

## Base de datos

El esquema se encuentra en [db/schema.sql](db/schema.sql) e incluye:

- `status`: catálogo de estados.
- `task_queue`: tareas pendientes y estado de procesamiento.
- `assets`: payloads completos recibidos desde Gmail API.
- `schema_versions`: historial de hashes por endpoint.
- `schema_snapshots`: representación JSON de cada schema.
- `worker_events`: auditoría de la ejecución.

Para inspeccionar la base de datos con SQLite3:

```powershell
sqlite3 db/gmail_worker.db
```

```sql
SELECT * FROM task_queue ORDER BY id DESC;
SELECT * FROM assets ORDER BY id DESC;
SELECT * FROM schema_versions ORDER BY id DESC;
SELECT * FROM schema_snapshots ORDER BY id DESC;
SELECT * FROM worker_events ORDER BY id DESC;
```

## Tests

Ejecuta la suite completa con:

```powershell
cargo test
```

Los tests cubren, entre otros aspectos, la toma de tareas, credenciales OAuth, inferencia y estabilidad del hash, inserción de tareas y registro de eventos de error.

## Estructura del proyecto

```text
GoogleGmailAPI/
├── Cargo.toml
├── contrato-proyecto.md
├── workplan.md
├── README.md
├── db/
│   └── schema.sql
├── src/
│   ├── main.rs
│   └── db.rs
├── examples/
│   └── task_queue_flow.rs
├── Documentacion/
│   ├── 01-BDD.md
│   ├── 02-GMAIL_API_SETUP.md
│   ├── 03-ImplementacionFinal.md
│   └── 04-Evidencias.md
└── images/
```

## Documentación

- [Contrato del proyecto](contrato-proyecto.md)
- [Workplan](workplan.md)
- [Base de datos](Documentacion/01-BDD.md)
- [Configuración de Gmail API](Documentacion/02-GMAIL_API_SETUP.md)
- [Implementación final](Documentacion/03-ImplementacionFinal.md)
- [Evidencias de implementación](Documentacion/04-Evidencias.md)
