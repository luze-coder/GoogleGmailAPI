# Work Plan - Gmail API Worker para Captura y Versionado de Schema

## 1. Introducción

Este proyecto implementa un worker ejecutable mediante CLI cuya responsabilidad es procesar tareas pendientes asociadas a Gmail API, obtener información desde la API, persistir la respuesta recibida, detectar cambios estructurales en el schema del payload y registrar eventos operativos asociados al procesamiento.

El worker forma parte de una arquitectura orientada a tareas y eventos, donde múltiples workers pueden coexistir procesando distintas APIs o dominios funcionales. Cada worker consume únicamente las tareas que le corresponden, evitando procesamiento duplicado mediante mecanismos de locking transaccional.

### Responsabilidades

- Consultar tareas pendientes asignadas al worker.
- Tomar la tarea de forma exclusiva.
- Consumir Gmail API.
- Persistir el payload recibido.
- Inferir y calcular el schema del payload.
- Detectar cambios estructurales.
- Versionar schemas cuando corresponda.
- Generar eventos operativos.
- Marcar tareas como resueltas.

### Stack Tecnológico

Lenguaje: Rust
HTTP: reqwest
OAuth: oauth2
JSON: serde
Base de datos: PostgreSQL
Hashes: sha2
Logs: println! o tracing

### Persistencia

La solución utiliza PostgreSQL para almacenar:

- Cola de tareas
- Eventos operativos
- Assets capturados
- Versiones de schema
- Snapshots históricos

---

## 2. Gmail API

### Proveedor

Google Workspace Gmail API

### Base URL

```text
https://gmail.googleapis.com
```

### Endpoint seleccionado

Listado de mensajes de una cuenta Gmail.

```http
GET /gmail/v1/users/me/messages
```

### Request

```http
GET https://gmail.googleapis.com/gmail/v1/users/me/messages
Authorization: Bearer <access_token>
```

### Parámetros soportados

| Parámetro | Tipo | Descripción |
|------------|------|-------------|
| maxResults | integer | Cantidad máxima de mensajes |
| pageToken | string | Token de paginación |
| q | string | Query Gmail |
| labelIds | string[] | Filtro por etiquetas |

Ejemplo:

```http
GET /gmail/v1/users/me/messages?maxResults=100&q=has:attachment
```

### Response esperada

```json
{
  "messages": [
    {
      "id": "18c1ea4b76",
      "threadId": "18c1ea4b76"
    }
  ],
  "nextPageToken": "abc123",
  "resultSizeEstimate": 2450
}
```

### Campos clave

| Campo | Tipo | Uso |
|---------|------|------|
| messages | array | Lista de mensajes |
| messages[].id | string | Identificador único |
| messages[].threadId | string | Conversación asociada |
| nextPageToken | string | Continuación de paginación |
| resultSizeEstimate | integer | Cantidad estimada |

### Autenticación

La integración utiliza OAuth 2.0.

Las credenciales se almacenan fuera del código fuente y se consumen durante la ejecución del worker.

Componentes necesarios:

- Client ID
- Client Secret
- Refresh Token

El worker obtiene un Access Token válido antes de realizar cada solicitud.

---

## 3. Flujo Operativo

El worker no consume Gmail API continuamente.

En cada ejecución:

1. Busca una tarea pendiente.
2. Bloquea la tarea.
3. Consume Gmail API.
4. Guarda la respuesta.
5. Calcula el schema.
6. Detecta cambios.
7. Versiona si corresponde.
8. Genera eventos.
9. Marca la tarea como finalizada.

### Flujo General

```text
Worker CLI
    |
    v
Buscar tarea pendiente
    |
    v
Tomar lock exclusivo
    |
    v
Consumir Gmail API
    |
    v
Guardar payload en assets
    |
    v
Inferir schema
    |
    v
Calcular hash
    |
    +------ igual ------> continuar
    |
    +------ distinto ---> versionar
    |
    v
Registrar evento
    |
    v
Completar tarea
```

---

## 4. Versionado de Schema

Gmail API es un servicio de terceros y la estructura de sus respuestas puede cambiar con el tiempo. Dado que el worker consume y almacena información sin controlar el origen de los datos, resulta necesario mantener un historial de los schemas detectados.

Cada respuesta obtenida es analizada para identificar su estructura. A partir de dicha estructura se genera una representación consistente que permite compararla con versiones previamente registradas.

Cuando se detecta una diferencia estructural, el sistema genera una nueva versión de schema y almacena una copia de referencia (snapshot) para conservar evidencia histórica del cambio.

### Regla de Versionado

- Si la estructura no cambia, no se genera una nueva versión.
- Si la estructura cambia, se registra una nueva versión y su correspondiente snapshot.
---

## 5. Modelo de Datos

La solución utiliza PostgreSQL para almacenar tareas, eventos, respuestas capturadas y versiones de schema.

### task_queue

Almacena las tareas pendientes que serán procesadas por los distintos workers.

Responsabilidades:

- Coordinar la ejecución de workers.
- Evitar procesamiento duplicado.
- Registrar asignación y resolución de tareas.
- Determinar qué endpoint debe consumirse.

---

### assets

Almacena los payloads obtenidos desde Gmail API.

Responsabilidades:

- Persistir la respuesta original.
- Mantener histórico de capturas.
- Asociar cada captura a la tarea que la generó.
- Permitir trazabilidad sobre los datos obtenidos.

---

### schema_versions

Mantiene el historial de versiones detectadas para cada endpoint monitoreado.

Responsabilidades:

- Registrar cambios de schema.
- Asociar cada versión a un hash único.
- Mantener numeración secuencial de versiones.

---

### schema_snapshots

Almacena la representación completa del schema correspondiente a una versión determinada.

Responsabilidades:

- Conservar evidencia histórica.
- Permitir comparar versiones anteriores.
- Facilitar análisis de cambios en la estructura de respuesta.

---

### worker_events

Almacena eventos operativos generados durante la ejecución.

Ejemplos:

- TaskAssigned
- AssetCaptured
- SchemaVersionCreated
- TaskCompleted
- TaskFailed

Responsabilidades:

- Auditar la ejecución del worker.
- Consultar estado de procesamiento.
- Identificar último trabajo ejecutado.
- Notificar eventos producidos por el sistema.
---

## 6. Control de Concurrencia

La arquitectura contempla la existencia de múltiples workers ejecutándose simultáneamente.

Para evitar que dos workers procesen una misma tarea, cada tarea debe ser tomada de manera exclusiva antes de comenzar su ejecución.

Una vez asignada, la tarea queda bloqueada para el resto de los workers hasta que finalice su procesamiento.

Este mecanismo garantiza:

- Procesamiento único por tarea.
- Ausencia de trabajo duplicado.
- Consistencia en la generación de assets.
- Correcta emisión de eventos.
- Trazabilidad del procesamiento realizado por cada worker.
---

## 7. Consultas Operativas

La información almacenada por el sistema permite consultar el estado operativo de los workers y el historial de procesamiento realizado.

Entre las consultas más relevantes se incluyen:

### Seguimiento de eventos

Permite identificar eventos generados durante la ejecución de los workers, tales como:

- Asignación de tareas.
- Captura de assets.
- Detección de nuevas versiones de schema.
- Finalización de tareas.
- Errores de procesamiento.

### Actividad de un worker

Permite conocer qué tareas fueron procesadas por un worker específico y cuáles fueron los eventos generados durante su ejecución.

### Última tarea procesada

Permite identificar la última tarea resuelta por un worker y consultar su estado final.

Estas capacidades facilitan tareas de monitoreo, auditoría y diagnóstico operativo.