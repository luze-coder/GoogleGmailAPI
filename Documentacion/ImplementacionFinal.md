## Consumir Gmail API

### Objetivo

Cerrar el ciclo completo `task_queue → obtener tarea → autenticarse → consumir endpoint → obtener payload`, dejándolo probado end-to-end contra la Gmail API real y con manejo de errores adecuado para producción.


### Testeo

**1. Preparar credenciales** (si no lo hiciste ya, seguí [GMAIL_API_SETUP.md](GMAIL_API_SETUP.md)):

```powershell
Copy-Item .env.example .env
# Editar .env y completar GMAIL_CLIENT_ID, GMAIL_CLIENT_SECRET, GMAIL_REFRESH_TOKEN
```

**2. Compilar y correr el flujo básico:**

```powershell
cargo build
cargo run -- --init-db
cargo run -- --queue-example
cargo run -- --list-tasks
```

Deberías ver una tarea con `status = 1` (PENDING) apuntando a `gmail/v1/users/me/messages`.

**3. Procesar la tarea contra la Gmail API real:**

```powershell
cargo run -- --process-one
```

- Si sale `Task <id> completed successfully.`, la autenticación y la consulta funcionaron.
- Si falla, el mensaje de error indica si fue el OAuth (`OAuth token refresh failed...`) o la API de Gmail (`La API de Gmail devolvió ...`).

**4. Verificar persistencia con SQLite** (por ejemplo con la extensión SQLite de VS Code o `sqlite3 db/gmail_worker.db`):

```sql
SELECT * FROM task_queue ORDER BY id DESC LIMIT 1;   -- debe estar status = 3 (COMPLETED)
SELECT * FROM assets ORDER BY id DESC LIMIT 1;        -- payload_json con datos reales de Gmail
SELECT * FROM schema_versions;                        -- al menos 1 versión creada
SELECT * FROM worker_events ORDER BY id;               -- TaskAssigned, AssetCaptured, (SchemaVersionCreated), TaskCompleted
```

**5. Confirmar la idempotencia del versionado de schema:**

```powershell
cargo run -- --queue-example   # siempre agrega una tarea nueva con el mismo endpoint/params
cargo run -- --process-one
```

Volvé a mirar `schema_versions`: si Gmail devuelve la misma estructura, **no** debe agregarse una fila nueva (mismo `schema_hash`), aunque `task_queue` sí tenga una fila nueva por cada `--queue-example`.

**6. Probar el camino de error** (opcional pero recomendado): editá temporalmente `.env` con un `GMAIL_CLIENT_SECRET` inválido, corré `--queue-example` + `--process-one` y confirmá que la tarea queda `status = 4` (FAILED) con un evento `TaskFailed` describiendo el error de OAuth. Restaurá el `.env` real después.

**7. Correr los tests unitarios existentes** (hash de schema, inferencia, etc.):

```powershell
cargo test
```

### Fuera de alcance

Lo siguiente quedó explícitamente fuera del alcance de esta implementación, porque el contrato ([contrato-proyecto.md](contrato-proyecto.md)) y el workplan ([workplan.md](workplan.md)) no lo exigen:

- **Paginación con `nextPageToken`.** El contrato solo pide consumir `GET /gmail/v1/users/me/messages` una vez por tarea. El worker no sigue automáticamente el `nextPageToken`; si Gmail lo devuelve, queda persistido dentro del payload en `assets`, pero no se dispara una segunda request.
- **Otros endpoints de Gmail API.** El contrato menciona un único endpoint (`messages`, listado). No está en alcance consumir `GET /gmail/v1/users/me/messages/{id}` (contenido completo del mensaje), `threads`, `labels` ni ningún otro recurso.
- **Runtime asíncrono (`tokio`).** El contrato pide control de concurrencia *entre workers* (que no procesen la misma tarea dos veces), lo cual ya se resuelve con el locking transaccional en `task_queue`. No exige concurrencia *dentro* de un mismo proceso, por lo que `reqwest::blocking` es suficiente y no se migra a `tokio`/`async`.
- **Retry automático / backoff exponencial.** Si `refresh_access_token()` o `fetch_gmail_messages()` fallan, la tarea se marca `FAILED` con el error registrado en `worker_events`; no hay reintentos automáticos ni espera progresiva ante rate limiting.
- **Interfaz gráfica.** Excluida explícitamente en el contrato (§6): el worker es exclusivamente CLI.
