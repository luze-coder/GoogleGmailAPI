# Base de datos

En esta documentación se presenta el modelo de datos, el flujo de datos y el paso a paso realizado para configurar la base de datos junto con sus tablas.

## Modelo de datos

La solución utiliza SQLite como mecanismo de persistencia para almacenar las tareas pendientes, los payloads capturados desde Gmail API, las versiones de schema detectadas y los eventos operativos generados durante la ejecución del worker.

## Entidades principales

### `status`

Tabla de referencia que define los estados posibles de una tarea.

| Campo | Tipo | Descripción |
|---|---|---|
| `id` | INTEGER | Identificador único del estado |
| `name` | TEXT | Nombre del estado |

#### Valores permitidos

- `PENDING`
- `PROCESSING`
- `COMPLETED`
- `FAILED`

Esta tabla permite garantizar la consistencia referencial sobre el estado de las tareas.

### `task_queue`

Almacena las tareas que deben ser procesadas por los workers.

| Campo | Tipo | Descripción |
|---|---|---|
| `id` | INTEGER | Identificador de la tarea |
| `worker_type` | TEXT | Tipo de worker responsable |
| `endpoint` | TEXT | Endpoint que se debe consumir |
| `parameters_json` | TEXT | Parámetros opcionales de ejecución |
| `status` | INTEGER | Estado actual de la tarea |
| `locked_by` | TEXT | Worker que tomó la tarea |
| `locked_at` | DATETIME | Fecha y hora de asignación |
| `created_at` | DATETIME | Fecha y hora de creación |
| `completed_at` | DATETIME | Fecha y hora de finalización |

#### Responsabilidades

- Coordinar la ejecución de los workers.
- Evitar el procesamiento duplicado.
- Registrar el estado de ejecución.
- Mantener la trazabilidad del procesamiento.

### `assets`

Almacena los payloads originales obtenidos desde Gmail API.

| Campo | Tipo | Descripción |
|---|---|---|
| `id` | INTEGER | Identificador del asset |
| `task_id` | INTEGER | Tarea que originó la captura |
| `payload_json` | TEXT | Respuesta completa obtenida |
| `captured_at` | DATETIME | Fecha y hora de captura |

#### Responsabilidades

- Mantener evidencia histórica.
- Conservar el payload original.
- Permitir análisis posteriores.
- Asociar cada captura con una ejecución específica.

### `schema_versions`

Registra las versiones de schema detectadas para un endpoint determinado.

| Campo | Tipo | Descripción |
|---|---|---|
| `id` | INTEGER | Identificador de la versión |
| `endpoint` | TEXT | Endpoint monitoreado |
| `version_number` | INTEGER | Número de versión |
| `schema_hash` | TEXT | Hash único del schema |
| `created_at` | DATETIME | Fecha y hora de creación |

#### Responsabilidades

- Detectar cambios estructurales.
- Mantener el historial de evolución del schema.
- Evitar versiones duplicadas.

### `schema_snapshots`

Almacena la representación completa del schema correspondiente a una versión.

| Campo | Tipo | Descripción |
|---|---|---|
| `id` | INTEGER | Identificador del snapshot |
| `schema_version_id` | INTEGER | Versión de schema asociada |
| `schema_json` | TEXT | Representación completa del schema |
| `created_at` | DATETIME | Fecha y hora de creación |

#### Responsabilidades

- Conservar evidencia histórica.
- Facilitar la comparación entre versiones.
- Permitir la auditoría de cambios.

### `worker_events`

Registra los eventos generados durante la ejecución del worker.

| Campo | Tipo | Descripción |
|---|---|---|
| `id` | INTEGER | Identificador del evento |
| `task_id` | INTEGER | Tarea asociada |
| `event_type` | TEXT | Tipo de evento |
| `details` | TEXT | Información adicional |
| `created_at` | DATETIME | Fecha y hora del evento |

#### Tipos de evento

- `TaskAssigned`
- `AssetCaptured`
- `SchemaVersionCreated`
- `TaskCompleted`
- `TaskFailed`

#### Responsabilidades

- Auditar la ejecución.
- Facilitar el monitoreo operativo.
- Proveer trazabilidad completa.

## Diagrama entidad-relación


![Diagrama entidad-relación](../images/GoogleGmail_ModeloDeDatos.png)

## Flujo de datos

El flujo general de procesamiento comienza con una tarea en estado `PENDING`. El worker toma la tarea, consume Gmail API, guarda el payload obtenido y analiza su estructura para detectar posibles cambios en el schema.

```text
task_queue (PENDING)
          │
          ▼
      Tomar lock
          │
          ▼
    TaskAssigned
          │
          ▼
  Consumir Gmail API
          │
          ▼
     Guardar asset
          │
          ▼
    AssetCaptured
          │
          ▼
     Inferir schema
          │
          ▼
     Calcular hash
          │
     ┌────┴────┐
     │         │
   Igual    Distinto
     │         │
     │    Crear versión
     │         │
     │    Crear snapshot
     │         │
     └────┬────┘
          │
          ▼
    TaskCompleted
```

## Paso a paso para crear la base de datos

### 1. Instalación de SQLite3

Se instaló SQLite3 mediante Chocolatey desde PowerShell en Windows.

```powershell
choco install sqlite
```

La instalación se verificó mediante el siguiente comando:

```powershell
sqlite3 --version
```

### 2. Creación de la estructura del proyecto

Se creó una carpeta destinada a almacenar los recursos relacionados con la base de datos.

```text
proyecto/
├── db/
│   └── schema.sql
├── src/
└── Cargo.toml
```

### 3. Definición del modelo de datos

Se creó el siguiente archivo:

```text
db/schema.sql
```

Este archivo contiene la definición completa de la base de datos, incluyendo:

- Tablas.
- Claves primarias.
- Claves foráneas.
- Restricciones de integridad.
- Índices.
- Datos iniciales de referencia.

El objetivo de este archivo es permitir que la base de datos pueda recrearse automáticamente en cualquier entorno.

### 4. Generación de la base de datos

Una vez finalizado el archivo `schema.sql`, se creó físicamente la base de datos mediante el siguiente comando:

```bash
sqlite3 db/gmail_worker.db < db/schema.sql
```

Este comando realiza las siguientes acciones:

1. Genera el archivo `gmail_worker.db`.
2. Ejecuta las instrucciones definidas en `schema.sql`.
3. Crea todas las tablas.
4. Crea los índices.
5. Inserta los estados iniciales.

### 5. Verificación de la base de datos

Finalmente, se verificó la creación correcta de la base de datos mediante:

```bash
sqlite3 db/gmail_worker.db
```

Una vez dentro de la consola de SQLite, se ejecutó:

```sql
.tables
```

El resultado esperado es:

```text
assets
schema_snapshots
schema_versions
status
task_queue
worker_events
```

## Resultado

Luego de completar estos pasos, quedó creada y operativa la base de datos SQLite del proyecto. Su estructura puede reconstruirse en cualquier entorno mediante el archivo `schema.sql`, mientras que el archivo `gmail_worker.db` contiene la base de datos física generada localmente.