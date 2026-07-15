# Contrato Base del Proyecto

## 1. Partes

Entre:

- **Solicitante:** Alejandro Rodriguez
- **Responsable del desarrollo:** Lujan Oviedo

Se acuerda el presente contrato para la definición y ejecución del proyecto técnico descrito en este documento.

## 2. Objeto del contrato

Desarrollar un worker ejecutable por CLI en Rust para integración con Gmail API, con capacidad de:

- Consumir tareas pendientes de una cola.
- Ejecutar consumo autenticado de Gmail API (OAuth 2.0).
- Persistir payloads obtenidos.
- Inferir y versionar schemas del payload.
- Detectar cambios estructurales entre versiones.
- Registrar eventos operativos del procesamiento.

## 3. Alcance funcional mínimo

El proyecto debe incluir, como mínimo:

1. Toma exclusiva de tareas pendientes con control de concurrencia.
2. Consumo del endpoint `GET /gmail/v1/users/me/messages`.
3. Persistencia de respuesta completa en base de datos PostgreSQL.
4. Cálculo de hash de schema para detección de cambios.
5. Versionado de schema cuando exista diferencia estructural.
6. Registro de eventos operativos (asignación, captura, versionado, finalización, error).
7. Cierre de tarea con estado final de ejecución.

## 4. Entregables

- Código fuente funcional del worker.
- Script o instrucciones de ejecución por CLI.
- Definición de modelo de datos mínimo: `task_queue`, `assets`, `schema_versions`, `schema_snapshots`, `worker_events`.
- Documento de workplan del proyecto.

## 5. Criterios de aceptación

Se considerará cumplido el contrato cuando:

1. El worker procese una tarea completa de extremo a extremo.
2. Se evidencie almacenamiento del payload capturado.
3. Se registre al menos un evento de ejecución.
4. El sistema detecte correctamente cambio y no cambio de schema.
5. Se mantenga trazabilidad de la tarea procesada.

## 6. Plazo

- **Fecha de inicio:** [20/07/2026]
- **Fecha de entrega:** [04/08/2026]

Las fechas podrán ajustarse por acuerdo entre las partes.

## 7. Exclusiones

No se incluye en este contrato, salvo acuerdo adicional:

- Desarrollo de interfaz gráfica (frontend).
- Despliegue productivo en infraestructura final.
- Soporte posterior a la entrega fuera del período académico o acordado.

## 8. Confidencialidad y credenciales

Las credenciales OAuth y cualquier dato sensible deberán almacenarse fuera del código fuente y no deberán publicarse en repositorios abiertos.
